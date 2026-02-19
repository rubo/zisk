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
    uint64_t dma_xmemset_mops(uint64_t dst, uint8_t value, uint64_t count, uint64_t* mops_ptr);
}

const char *mops_labels[16] = {"NOP", "CWR1", "RD1", "WR1", "RD2", "WR2", "RD4", "WR4", "RD8", "WR8",
                             "ARD", "AWR", "BR", "BW", "ABR", "ABW"};

// MOPS constants from dma_constants.inc
const uint64_t MOPS_ALIGNED_READ = 0x0000'000C'0000'0000ULL;
const uint64_t MOPS_ALIGNED_BLOCK_WRITE = 0x0000'000F'0000'0000ULL;
const uint64_t MOPS_BLOCK_WORDS_SBITS = 36;
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

void print_mops_trace(uint64_t* mops_ptr, size_t mops_count) {
    std::cout << "MOPS Trace (" << mops_count << " entries):\n";
    for (size_t i = 0; i < mops_count; ++i) {
        uint64_t entry = mops_ptr[i];
        uint64_t opcode = (entry >> 32) & 0x0F;
        uint64_t addr = entry & 0xFFFF'FFFF;
        uint64_t block_words = entry >> MOPS_BLOCK_WORDS_SBITS;
        
        printf("  [%ld] %s (0x%X) addr=0x%08lX", i, mops_labels[opcode], (unsigned)opcode, addr);
        if (opcode == 0x0E || opcode == 0x0F) { // ABR or ABW
            printf(" words=%ld", block_words);
        }
        printf(" (raw=0x%016lX)\n", entry);
    }
}

bool validate_mops_trace(uint64_t dst, uint64_t count, uint64_t* mops_ptr, size_t mops_count) {
    if (count == 0) {
        if (mops_count != 0) {
            printf("❌ FAIL: Expected 0 mops entries for count=0, got %ld\n", mops_count);
            return false;
        }
        return true;
    }

    uint64_t dst_aligned = dst & ALIGN_MASK;
    uint64_t dst_offset = dst & 0x07;
    uint64_t last_byte_addr = dst + count - 1;
    uint64_t last_qword_aligned = last_byte_addr & ALIGN_MASK;
    
    // Calculate how many qwords are affected
    uint64_t qwords_affected = ((count + dst_offset + 7) >> 3);
    
    // Determine if we need pre and post reads
    bool needs_pre_read = (dst_offset != 0);
    bool needs_post_read = ((dst_offset + count) & 0x07) != 0 && (last_qword_aligned > dst_aligned || dst_offset == 0);
    
    // Expected number of mops entries
    size_t expected_entries = 0;
    if (needs_pre_read) expected_entries++;
    if (needs_post_read) expected_entries++;
    expected_entries++; // Always have block write for count > 0
    
    if (mops_count != expected_entries) {
        printf("❌ FAIL: Expected %ld mops entries, got %ld\n", expected_entries, mops_count);
        printf("   dst=0x%lX, count=%ld, dst_offset=%ld\n", dst, count, dst_offset);
        printf("   needs_pre_read=%d, needs_post_read=%d\n", needs_pre_read, needs_post_read);
        print_mops_trace(mops_ptr, mops_count);
        return false;
    }
    
    size_t mops_idx = 0;
    
    // Verify pre-read if expected
    if (needs_pre_read) {
        uint64_t expected = MOPS_ALIGNED_READ + dst_aligned;
        if (mops_ptr[mops_idx] != expected) {
            printf("❌ FAIL: PRE-READ mops[%ld]: expected 0x%016lX, got 0x%016lX\n", 
                   mops_idx, expected, mops_ptr[mops_idx]);
            return false;
        } else 
        mops_idx++;
    }
    
    // Verify post-read if expected
    if (needs_post_read) {
        uint64_t expected = MOPS_ALIGNED_READ + last_qword_aligned;
        if (mops_ptr[mops_idx] != expected) {
            printf("❌ FAIL: POST-READ mops[%ld]: expected 0x%016lX, got 0x%016lX\n", 
                   mops_idx, expected, mops_ptr[mops_idx]);
            return false;
        }
        mops_idx++;
    }
    
    // Verify block write
    uint64_t expected_block_write = MOPS_ALIGNED_BLOCK_WRITE + 
                                   (qwords_affected << MOPS_BLOCK_WORDS_SBITS) + 
                                   dst_aligned;
    if (mops_ptr[mops_idx] != expected_block_write) {
        printf("❌ FAIL: BLOCK-WRITE mops[%ld]: expected 0x%016lX, got 0x%016lX\n", 
               mops_idx, expected_block_write, mops_ptr[mops_idx]);
        printf("   qwords_affected=%ld, dst_aligned=0x%lX\n", qwords_affected, dst_aligned);
        return false;
    }
    
    return true;
}

bool test_fast_memset_single(size_t dst_offset, size_t count, uint8_t value) {
    constexpr size_t BUFFER_SIZE = 2048;
    constexpr size_t TEST_AREA_START = 512;
    constexpr uint8_t PATTERN_START = 0x37;
    
    // Allocate buffer and fill with pattern to detect overwrites
    AlignedBuffer buffer(BUFFER_SIZE);
    buffer.fill_pattern(PATTERN_START);
    
    // Calculate destination address
    uint64_t dst = reinterpret_cast<uint64_t>(buffer.byte_ptr() + TEST_AREA_START) + dst_offset;
    
    // Call the function
    fast_memset(dst, value, count);
    
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
    return true;
}

bool test_memset_mops_single(size_t dst_offset, size_t count, uint8_t value) {
    constexpr size_t BUFFER_SIZE = 2048;
    constexpr size_t TEST_AREA_START = 512;
    constexpr uint8_t PATTERN_START = 0x37;
    
    // Allocate buffer and fill with pattern to detect overwrites
    AlignedBuffer buffer(BUFFER_SIZE);
    buffer.fill_pattern(PATTERN_START);
    
    // Allocate mops trace buffer (16 u64 entries as specified)
    AlignedBuffer mops_buffer(16 * sizeof(uint64_t));
    mops_buffer.fill_value(0);
    
    // Calculate destination address
    uint64_t dst = reinterpret_cast<uint64_t>(buffer.byte_ptr() + TEST_AREA_START) + dst_offset;
    uint64_t* mops_ptr = mops_buffer.aligned_ptr();
    
    // Call the function
    uint64_t mops_count = dma_xmemset_mops(dst, value, count, mops_ptr);
    
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
    
    // Validate mops trace
    if (!validate_mops_trace(dst, count, mops_ptr, mops_count)) {
        printf("❌ TEST FAILED (MOPS): dst=0x%08lX dst_offset=%ld, count=%ld\n", 
               dst, dst_offset, count);
        return false;
    }
    
    return true;
}

void test_all_combinations() {
    constexpr uint8_t TEST_VALUE = 0xAB;
    size_t total_tests = 0;
    size_t passed_tests = 0;
    size_t failed_tests = 0;
    
    std::cout << "Starting comprehensive memset_mops tests...\n";
    std::cout << "Testing dst_offsets 0-7 × lengths 0-1024\n";
    std::cout << "Total tests: " << (8 * 1025) << "\n\n";
    
    for (size_t dst_offset = 0; dst_offset <= 7; ++dst_offset) {
        std::cout << "Testing dst_offset=" << dst_offset << "...\n";
        
        for (size_t count = 0; count <= 1024; ++count) {
            total_tests += 2;
            
            if (test_fast_memset_single(dst_offset, count, TEST_VALUE)) {
                passed_tests++;
            } else {
                failed_tests++;
                std::cout << "❌ FAILED: memset offset=" << dst_offset << " count=" << count << "\n";
                // Don't stop on first failure, continue testing
            }
            if (test_memset_mops_single(dst_offset, count, TEST_VALUE)) {
                passed_tests++;
            } else {
                failed_tests++;
                std::cout << "❌ FAILED: memset_ops offset=" << dst_offset << " count=" << count << "\n";
                // Don't stop on first failure, continue testing
            }
            // Progress indicator every 128 tests
            if (count % 128 == 0 && count > 0) {
                std::cout << "  ... progress: " << count << "/1024" << std::endl;
            }
        }
    }
    
    std::cout << "\n========================================\n";
    std::cout << "TEST SUMMARY\n";
    std::cout << "========================================\n";
    std::cout << "Total tests:  " << total_tests << "\n";
    std::cout << "Passed:       " << passed_tests << " ✅\n";
    std::cout << "Failed:       " << failed_tests << " ❌\n";
    std::cout << "Success rate: " << (100.0 * passed_tests / total_tests) << "%\n";
    std::cout << "========================================\n";
    
    if (failed_tests == 0) {
        std::cout << "\n🎉 ALL TESTS PASSED! 🎉\n";
    } else {
        std::cout << "\n⚠️  SOME TESTS FAILED ⚠️\n";
        exit(1);
    }
}

int main() {
    std::cout << "==============================================\n";
    std::cout << "  DMA MEMSET MOPS COMPREHENSIVE TEST SUITE\n";
    std::cout << "==============================================\n\n";
    
    test_all_combinations();
    
    return 0;
}
