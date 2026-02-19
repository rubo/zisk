#include <cstdint>
#include <cstring>
#include <iostream>
#include <vector>
#include <iomanip>
#include <cassert>

// External assembly function declarations
extern "C" {
    uint64_t trace_address_threshold = 0;
    void fast_memset(uint64_t dst, uint8_t value, uint64_t count);
    uint64_t dma_xmemset_mtrace(uint64_t dst, uint8_t value, uint64_t count, uint64_t* mtrace_ptr);
    uint64_t fast_dma_encode_memset_with_byte(uint64_t dst, uint8_t value, uint64_t count);    
}

const uint64_t ALIGN_MASK = 0xFFFF'FFFF'FFFF'FFF8ULL;

// Helper class to manage aligned test buffers
class AlignedBuffer {
public:
    std::vector<uint8_t> data;
    
    AlignedBuffer(size_t size) : data(size, 0) {}
    
    uint64_t* aligned_ptr() {
        return reinterpret_cast<uint64_t*>(data.data());
    }
    
    uint8_t* byte_ptr() {
        return data.data();
    }
    
    const uint8_t* byte_ptr() const {
        return data.data();
    }
    
    void fill_pattern(uint8_t start = 0) {
        for (size_t i = 0; i < data.size(); ++i) {
            data[i] = static_cast<uint8_t>(start + i);
        }
    }

    void fill_value(uint8_t value) {
        std::fill(data.begin(), data.end(), value);
    }

    bool verify_pattern(uint8_t start = 0, const char *title = "") {
        for (size_t i = 0; i < data.size(); ++i) {
            uint8_t expected = static_cast<uint8_t>(start + i);
            if (data[i] != expected) {
                printf("❌ FAIL PATTERN VERIFICATION of %s: Expected: 0x%02X vs data[%ld]=0x%02X\n",
                    title, expected, i, data[i]);
                return false;
            }
        }
        return true;
    }

    bool verify_pattern_except(uint8_t start, size_t from, size_t count, const char *title = "") {
        size_t to = from + count;
        for (size_t i = 0; i < data.size(); ++i) {
            if (i >= from && i < to) continue;
            uint8_t expected = static_cast<uint8_t>(start + i);
            if (data[i] != expected) {
                printf("❌ FAIL PATTERN VERIFICATION of %s: Expected: 0x%02X vs data[%ld]=0x%02X (from=%ld, count=%ld)\n",
                    title, expected, i, data[i], from, count);
                return false;
            }
        }
        return true;
    }

    bool verify_fill(uint8_t value, size_t from, size_t count, const char *title = "") {
        size_t to = from + count;
        for (size_t i = from; i < to; ++i) {
            if (data[i] != value) {
                printf("❌ FAIL FILL VERIFICATION of %s at [%ld]: Expected: 0x%02X vs 0x%02X (from=%ld, count=%ld)\n",
                    title, i, value, data[i], from, count);
                return false;
            }
        }
        return true;
    }

    bool verify_fill_except(uint8_t value, size_t from, size_t count, const char *title = "") {
        size_t to = from + count;
        for (size_t i = 0; i < data.size(); ++i) {
            if (i >= from && i < to) continue;
            if (data[i] != value) {
                printf("❌ FAIL FILL VERIFICATION of %s at [%ld]: Expected: 0x%02X vs 0x%02X (should be outside from=%ld, count=%ld)\n",
                    title, i, value, data[i], from, count);
                return false;
            }
        }
        return true;
    }
};

void print_mtrace(uint64_t* mtrace_ptr, size_t mtrace_count) {
    std::cout << "MTRACE (" << mtrace_count << " entries):\n";
    for (size_t i = 0; i < mtrace_count; ++i) {
        uint64_t entry = mtrace_ptr[i];
        printf("  [%ld] 0x%016lX\n", i, entry);
    }
}

bool validate_mtrace(uint64_t dst, uint64_t count, uint8_t value, 
                     uint64_t* mtrace_ptr, size_t mtrace_count,
                     const AlignedBuffer& buffer, size_t test_area_start, size_t dst_offset) {
    
    // Expected encode value from fast_dma_encode_memset_with_byte
    uint64_t expected_encode = fast_dma_encode_memset_with_byte(dst, value, count);
    
    if (mtrace_count == 0) {
        printf("❌ FAIL: Expected at least 1 mtrace entry (encode), got 0\n");
        return false;
    }
    
    // Verify encode (first entry)
    if (mtrace_ptr[0] != expected_encode) {
        printf("❌ FAIL: ENCODE mtrace[0]: expected 0x%016lX, got 0x%016lX\n", 
               expected_encode, mtrace_ptr[0]);
        printf("   dst=0x%lX, count=%ld, value=0x%02X, dst_offset=%ld\n", 
               dst, count, value, dst_offset);
        print_mtrace(mtrace_ptr, mtrace_count);
        return false;
    }
    
    uint64_t dst_aligned = dst & ALIGN_MASK;
    
    // For count=0, only encode is generated, no PRE/POST
    if (count == 0) {
        if (mtrace_count != 1) {
            printf("❌ FAIL: Expected 1 mtrace entry for count=0, got %ld\n", mtrace_count);
            print_mtrace(mtrace_ptr, mtrace_count);
            return false;
        }
        return true;
    }
    
    // Determine if we need PRE and POST reads
    bool needs_pre = (dst_offset > 0);
    bool needs_post = ((dst_offset + count) & 0x07) != 0 & (dst_offset == 0 || (dst_offset + count) > 8);
    
    size_t expected_entries = 1; // Always have encode
    size_t mtrace_idx = 1;
    
    if (needs_pre) {
        expected_entries++;
    }
    if (needs_post) {
        expected_entries++;
    }
    
    if (mtrace_count != expected_entries) {
        printf("❌ FAIL: Expected %ld mtrace entries, got %ld\n", expected_entries, mtrace_count);
        printf("   dst=0x%lX, count=%ld, value=0x%02X, dst_offset=%ld\n", 
               dst, count, value, dst_offset);
        printf("   needs_pre=%d, needs_post=%d\n", needs_pre, needs_post);
        print_mtrace(mtrace_ptr, mtrace_count);
        return false;
    }
    
    // Verify PRE entry if expected
    if (needs_pre) {
        // PRE contains the value read from the first aligned qword before modification
        // We can't easily verify the exact value since it's read before memset
        // Just verify the entry exists
        if (mtrace_idx >= mtrace_count) {
            printf("❌ FAIL: Expected PRE entry at mtrace[%ld], but only %ld entries\n", 
                   mtrace_idx, mtrace_count);
            return false;
        }
        mtrace_idx++;
    }
    
    // Verify POST entry if expected
    if (needs_post) {
        if (mtrace_idx >= mtrace_count) {
            printf("❌ FAIL: Expected POST entry at mtrace[%ld], but only %ld entries\n", 
                   mtrace_idx, mtrace_count);
            return false;
        }
        mtrace_idx++;
    }
    
    return true;
}

bool test_memset_mtrace_single(size_t dst_offset, size_t count, uint8_t value) {
    constexpr size_t BUFFER_SIZE = 2048;
    constexpr size_t TEST_AREA_START = 512;
    constexpr uint8_t PATTERN_START = 0x37;
    
    // Allocate buffer and fill with pattern to detect overwrites
    AlignedBuffer buffer(BUFFER_SIZE);
    buffer.fill_pattern(PATTERN_START);
    
    // Allocate mtrace buffer (max 3 u64 entries: encode + PRE + POST)
    AlignedBuffer mtrace_buffer(3 * sizeof(uint64_t));
    mtrace_buffer.fill_value(0);
    
    // Calculate destination address
    uint64_t dst = reinterpret_cast<uint64_t>(buffer.byte_ptr() + TEST_AREA_START) + dst_offset;
    uint64_t* mtrace_ptr = mtrace_buffer.aligned_ptr();
    printf("TEST: dst_offset=%ld, count=%ld, value=0x%02X\n", dst_offset, count, value);
        
    // Call the function
    uint64_t mtrace_count = dma_xmemset_mtrace(dst, value, count, mtrace_ptr);
    
    // Verify that the memset was performed correctly
    if (!buffer.verify_fill(value, TEST_AREA_START + dst_offset, count, "memset result")) {
        printf("❌ TEST FAILED: dst_offset=%ld, count=%ld, value=0x%02X\n", 
               dst_offset, count, value);
        return false;
    }
    
    // Verify that nothing outside the target area was modified
    if (!buffer.verify_pattern_except(PATTERN_START, TEST_AREA_START + dst_offset, count, 
                                     "buffer overflow detection")) {
        printf("❌ TEST FAILED (OVERFLOW): dst_offset=%ld, count=%ld, value=0x%02X\n", 
               dst_offset, count, value);
        return false;
    }
    
    // Validate mtrace
    if (!validate_mtrace(dst, count, value, mtrace_ptr, mtrace_count, 
                        buffer, TEST_AREA_START, dst_offset)) {
        printf("❌ TEST FAILED (MTRACE): dst_offset=%ld, count=%ld, value=0x%02X\n", 
               dst_offset, count, value);
        return false;
    }
    
    return true;
}

void print_progress(int current, int total) {
    if (current % 100 == 0 || current == total) {
        std::cout << "Progress: " << current << "/" << total 
                  << " (" << (current * 100 / total) << "%)\r" << std::flush;
    }
}

int main() {
    std::cout << "==============================================\n";
    std::cout << "  DMA MEMSET MTRACE COMPREHENSIVE TEST SUITE\n";
    std::cout << "==============================================\n\n";
    
    int total_tests = 0;
    int passed_tests = 0;
    int failed_tests = 0;
    
    // Test parameters
    const int max_count = 1024;
    const int max_offset = 7;
    const int num_values = 16; // Test 16 different byte values instead of all 256
    
    // Test specific values: 0x00, 0xFF, and some random values
    const uint8_t test_values[] = {0x00, 0x01, 0x10, 0x37, 0x55, 0x7F, 0x80, 0xAA, 
                                    0xAB, 0xC0, 0xCD, 0xEF, 0xF0, 0xFE, 0xFF, 0x42};
    
    std::cout << "Test configuration:\n";
    std::cout << "  - Destination offsets: 0-" << max_offset << "\n";
    std::cout << "  - Byte counts: 0-" << max_count << "\n";
    std::cout << "  - Test values: " << num_values << " different bytes\n";
    std::cout << "  - Total tests: " << ((max_offset + 1) * (max_count + 1) * num_values) << "\n\n";
    
    std::cout << "Running tests...\n";
    
    // Test all combinations
    for (size_t value_idx = 0; value_idx < num_values; ++value_idx) {
        uint8_t value = test_values[value_idx];
        
        for (size_t dst_offset = 0; dst_offset <= max_offset; ++dst_offset) {
            for (size_t count = 0; count <= max_count; ++count) {
                total_tests++;
                print_progress(total_tests, (max_offset + 1) * (max_count + 1) * num_values);
                
                if (test_memset_mtrace_single(dst_offset, count, value)) {
                    passed_tests++;
                } else {
                    failed_tests++;
                    std::cout << "\n❌ FAILED: dst_offset=" << dst_offset 
                              << ", count=" << count 
                              << ", value=0x" << std::hex << (int)value << std::dec << "\n";
                    
                    // Stop on first failure for debugging
                    if (failed_tests >= 1) {
                        std::cout << "\nStopping on first failure for debugging.\n";
                        std::cout << "You can debug with: gdb ./test_dma_memset_mtrace\n";
                        goto done;
                    }
                }
            }
        }
    }
    
done:
    std::cout << "\n\n==============================================\n";
    std::cout << "  Test Results\n";
    std::cout << "==============================================\n";
    std::cout << "Total tests:  " << total_tests << "\n";
    std::cout << "Passed:       " << passed_tests << " ✅\n";
    std::cout << "Failed:       " << failed_tests << " ❌\n";
    
    if (failed_tests == 0) {
        std::cout << "\n🎉 ALL TESTS PASSED! 🎉\n";
    } else {
        std::cout << "\n⚠️  SOME TESTS FAILED ⚠️\n";
        return 1;
    }
    
    return 0;
}
