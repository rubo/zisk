#include <cstdint>
#include <cstring>
#include <iostream>
#include <vector>
#include <iomanip>
#include <cassert>
#include <algorithm>

// FCALL constants from dma_constants.inc
const uint64_t FCALL_PARAMS_LENGTH = 386;
const uint64_t FCALL_RESULT_LENGTH = 8193;
const uint64_t FCALL_FUNCTION_ID = 0;
const uint64_t FCALL_PARAMS_CAPACITY = FCALL_FUNCTION_ID + 1;
const uint64_t FCALL_PARAMS_SIZE = FCALL_PARAMS_CAPACITY + 1;
const uint64_t FCALL_PARAMS = FCALL_PARAMS_SIZE + 1;
const uint64_t FCALL_RESULT_CAPACITY = FCALL_PARAMS + FCALL_PARAMS_LENGTH;
const uint64_t FCALL_RESULT_SIZE = FCALL_RESULT_CAPACITY + 1;
const uint64_t FCALL_RESULT = FCALL_RESULT_SIZE + 1;            // 391
const uint64_t FCALL_RESULT_GOT = FCALL_RESULT + FCALL_RESULT_LENGTH; // 8584
const uint64_t FCALL_CTX_LENGTH = FCALL_RESULT_GOT + 1;         // 8585

// External assembly function declarations and fcall_ctx
extern "C" {
    uint64_t trace_address_threshold = 0;
    uint64_t fcall_ctx[FCALL_CTX_LENGTH];
    uint64_t dma_inputcpy_mops(uint64_t dst, uint64_t count, uint64_t* mops_ptr);
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
        }
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

void init_fcall_result_data(uint64_t pattern_start) {
    // Initialize FCALL_RESULT with test pattern
    for (size_t i = 0; i < FCALL_RESULT_LENGTH; ++i) {
        uint64_t value = 0;
        for (int b = 0; b < 8; ++b) {
            value |= ((uint64_t)(pattern_start + i * 8 + b)) << (b * 8);
        }
        fcall_ctx[FCALL_RESULT + i] = value;
    }
    
    // Initialize FCALL_RESULT_GOT to 1 (next read from FCALL_RESULT[0])
    fcall_ctx[FCALL_RESULT_GOT] = 1;
}

bool test_inputcpy_single(uint64_t dst_offset, size_t count) {
    if (count % 8 != 0) {
        std::cout << "❌ ERROR: count must be multiple of 8, got " << count << "\n";
        return false;
    }
    
    // Initialize fcall_ctx with test data (pattern starting at 0x10)
    init_fcall_result_data(0x10);
    
    // Save initial FCALL_RESULT_GOT
    uint64_t initial_result_got = fcall_ctx[FCALL_RESULT_GOT];
    
    // Allocate destination buffer
    AlignedBuffer dst_buf(2048);
    dst_buf.fill_pattern(0xA0); // Fill with different pattern
    
    // Calculate destination address with offset
    uint64_t dst_addr = reinterpret_cast<uint64_t>(dst_buf.byte_ptr() + 64) + dst_offset;
    
    // Allocate MOPS trace buffer
    AlignedBuffer mops_buf(256);
    mops_buf.fill_value(0);
    uint64_t* mops_ptr = mops_buf.aligned_ptr();
    
    // Call assembly function
    uint64_t mops_count = dma_inputcpy_mops(dst_addr, count, mops_ptr);
    
    // Verify FCALL_RESULT_GOT was updated correctly
    uint64_t expected_result_got = initial_result_got + (count / 8);
    uint64_t actual_result_got = fcall_ctx[FCALL_RESULT_GOT];
    
    if (actual_result_got != expected_result_got) {
        std::cout << "❌ FAIL: FCALL_RESULT_GOT not updated correctly\n";
        std::cout << "  dst_offset=" << dst_offset << " count=" << count << "\n";        std::cout << "  Expected FCALL_RESULT_GOT=" << expected_result_got 
                  << " Got=" << actual_result_got << "\n";
        return false;
    }
    
    // Verify copied data matches FCALL_RESULT
    const uint8_t* dst_bytes = reinterpret_cast<const uint8_t*>(dst_addr);
    bool data_ok = true;
    
    for (size_t i = 0; i < count; ++i) {
        // Calculate which qword and byte within that qword
        size_t qword_idx = i / 8;
        size_t byte_in_qword = i % 8;
        
        // Get expected byte from FCALL_RESULT (accounting for initial_result_got - 1)
        uint64_t source_qword = fcall_ctx[FCALL_RESULT + (initial_result_got - 1) + qword_idx];
        uint8_t expected = (source_qword >> (byte_in_qword * 8)) & 0xFF;
        
        if (dst_bytes[i] != expected) {
            std::cout << "❌ FAIL: Data mismatch at byte " << i << "\n";
            std::cout << "  dst_offset=" << dst_offset << " count=" << count << "\n";
            std::cout << "  Expected=0x" << std::hex << (int)expected
                      << " Got=0x" << (int)dst_bytes[i] << std::dec << "\n";
            data_ok = false;
            break;
        }
    }
    
    if (!data_ok) {
        std::cout << "\nFirst 32 bytes of destination:\n  ";
        for (size_t i = 0; i < std::min(count, size_t(32)); ++i) {
            if (i > 0 && i % 16 == 0) std::cout << "\n  ";
            std::cout << std::hex << std::setw(2) << std::setfill('0') 
                      << (int)dst_bytes[i] << " ";
        }
        std::cout << std::dec << "\n";
        
        std::cout << "\nExpected (from FCALL_RESULT):\n  ";
        for (size_t i = 0; i < std::min(count, size_t(32)); ++i) {
            if (i > 0 && i % 16 == 0) std::cout << "\n  ";
            size_t qword_idx = i / 8;
            size_t byte_in_qword = i % 8;
            uint64_t source_qword = fcall_ctx[FCALL_RESULT + (initial_result_got - 1) + qword_idx];
            uint8_t expected = (source_qword >> (byte_in_qword * 8)) & 0xFF;
            std::cout << std::hex << std::setw(2) << std::setfill('0') 
                      << (int)expected << " ";
        }
        std::cout << std::dec << "\n";
        
        return false;
    }
    
    // Validate MOPS trace
    if (!validate_mops_trace(dst_addr, count, mops_ptr, mops_count)) {
        std::cout << "❌ FAIL (MOPS): dst=0x" << std::hex << dst_addr << std::dec 
                  << " dst_offset=" << dst_offset << ", count=" << count << "\n";
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
    std::cout << "  Testing dma_inputcpy_mops implementation\n";
    std::cout << "==============================================\n\n";
    
    std::cout << "FCALL_CTX structure:\n";
    std::cout << "  - Total length: " << FCALL_CTX_LENGTH << " qwords (" 
              << (FCALL_CTX_LENGTH * 8) << " bytes)\n";
    std::cout << "  - FCALL_RESULT offset: " << FCALL_RESULT << " (data input)\n";
    std::cout << "  - FCALL_RESULT_GOT offset: " << FCALL_RESULT_GOT << " (read counter)\n\n";
    
    int total_tests = 0;
    int passed_tests = 0;
    int failed_tests = 0;
    
    // Test parameters
    const int max_count = 1024;
    const int count_step = 8;  // Must be multiple of 8
    const int max_offset = 7;   // Test offsets 0-7
    
    std::cout << "Test configuration:\n";
    std::cout << "  - Destination offsets: 0-" << max_offset << "\n";
    std::cout << "  - Byte counts: 0-" << max_count << " (step=" << count_step << ", multiples of 8 only)\n";
    std::cout << "  - Total tests: " << ((max_offset + 1) * ((max_count / count_step) + 1)) << "\n\n";
    
    std::cout << "Running tests...\n";
    
    // Test all combinations of dst_offset and count (must be multiple of 8)
    for (int dst_off = 0; dst_off <= max_offset; ++dst_off) {
        for (int count = 0; count <= max_count; count += count_step) {
            total_tests++;
            print_progress(total_tests, ((max_offset + 1) * ((max_count / count_step) + 1)));
            
            if (test_inputcpy_single(dst_off, count)) {
                passed_tests++;
            } else {
                failed_tests++;
                std::cout << "\n❌ FAILED: dst_offset=" << dst_off 
                          << ", count=" << count << "\n";
                
                // Stop on first failure for debugging
                if (failed_tests >= 1) {
                    std::cout << "\nStopping on first failure for debugging.\n";
                    std::cout << "You can debug with: gdb ./test_dma_inputcpy\n";
                    goto done;
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
        return 0;
    } else {
        std::cout << "\n❌ SOME TESTS FAILED ❌\n";
        return 1;
    }
}
