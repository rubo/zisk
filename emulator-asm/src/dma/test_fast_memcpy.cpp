#include <cstdint>
#include <cstring>
#include <iostream>
#include <vector>
#include <iomanip>
#include <cassert>

// External assembly function declaration
extern "C" {
    void fast_memcpy(uint64_t dst, uint64_t src, uint64_t count);
}

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

bool test_fast_memcpy_single(uint64_t dst_offset, uint64_t src_offset, size_t count) {
    // Allocate buffers with extra space for offsets
    AlignedBuffer src_buf(2048);
    AlignedBuffer dst_buf(2048);
    
    // Fill source with pattern starting at 0x10
    src_buf.fill_pattern(0x10);
    
    // Fill destination with different pattern (0xA0)
    dst_buf.fill_pattern(0xA0);
    
    // Calculate actual addresses with offsets
    uint64_t src_addr = reinterpret_cast<uint64_t>(src_buf.byte_ptr() + 64) + src_offset;
    uint64_t dst_addr = reinterpret_cast<uint64_t>(dst_buf.byte_ptr() + 64) + dst_offset;
    
    // Call assembly function
    fast_memcpy(dst_addr, src_addr, count);
    
    // Verify the memcpy was performed correctly
    const uint8_t* src_bytes = reinterpret_cast<const uint8_t*>(src_addr);
    const uint8_t* dst_bytes = reinterpret_cast<const uint8_t*>(dst_addr);
    
    bool ok = true;
    for (size_t i = 0; i < count; ++i) {
        if (dst_bytes[i] != src_bytes[i]) {
            std::cout << "❌ FAIL at byte " << i 
                      << " dst_off=" << dst_offset 
                      << " src_off=" << src_offset
                      << " count=" << count
                      << " expected=0x" << std::hex << (int)src_bytes[i]
                      << " got=0x" << (int)dst_bytes[i] << std::dec << "\n";
            ok = false;
            break;
        }
    }
    
    if (!ok) {
        // Print context around failure
        std::cout << "Source bytes around copy region:\n";
        for (size_t i = 0; i < std::min(count, size_t(32)); ++i) {
            if (i % 16 == 0) std::cout << "  ";
            std::cout << std::hex << std::setw(2) << std::setfill('0') 
                      << (int)src_bytes[i] << " ";
            if (i % 16 == 15) std::cout << "\n";
        }
        std::cout << "\nDestination bytes around copy region:\n";
        for (size_t i = 0; i < std::min(count, size_t(32)); ++i) {
            if (i % 16 == 0) std::cout << "  ";
            std::cout << std::hex << std::setw(2) << std::setfill('0') 
                      << (int)dst_bytes[i] << " ";
            if (i % 16 == 15) std::cout << "\n";
        }
        std::cout << std::dec << "\n";
        
        // Also verify that areas outside the copy region weren't touched
        const uint8_t* dst_base = dst_buf.byte_ptr() + 64;
        size_t offset_in_buf = dst_bytes - dst_base;
        
        // Check before region
        for (size_t i = 0; i < offset_in_buf; ++i) {
            uint8_t expected = static_cast<uint8_t>(0xA0 + i);
            if (dst_base[i] != expected) {
                std::cout << "❌ Buffer corruption BEFORE copy region at " << i << "\n";
                break;
            }
        }
        
        // Check after region
        for (size_t i = offset_in_buf + count; i < 128; ++i) {
            uint8_t expected = static_cast<uint8_t>(0xA0 + i);
            if (dst_base[i] != expected) {
                std::cout << "❌ Buffer corruption AFTER copy region at " << i << "\n";
                break;
            }
        }
    }
    
    return ok;
}

void print_progress(int current, int total) {
    if (current % 100 == 0 || current == total) {
        std::cout << "Progress: " << current << "/" << total 
                  << " (" << (current * 100 / total) << "%)\r" << std::flush;
    }
}

int main() {
    std::cout << "==============================================\n";
    std::cout << "  Testing fast_memcpy assembly implementation\n";
    std::cout << "  (Duff's device based implementation)\n";
    std::cout << "==============================================\n\n";
    
    int total_tests = 0;
    int passed_tests = 0;
    int failed_tests = 0;
    
    // Test parameters
    const int max_count = 1024;
    const int count_step = 1;  // Test every byte count
    const int max_offset = 7;   // Test offsets 0-7
    
    std::cout << "Test configuration:\n";
    std::cout << "  - Destination offsets: 0-" << max_offset << "\n";
    std::cout << "  - Source offsets: 0-" << max_offset << "\n";
    std::cout << "  - Byte counts: 0-" << max_count << " (step=" << count_step << ")\n";
    std::cout << "  - Total tests: " << ((max_offset + 1) * (max_offset + 1) * ((max_count / count_step) + 1)) << "\n\n";
    
    std::cout << "Running tests...\n";
    
    // Test all combinations of dst_offset, src_offset, and count
    for (int dst_off = 0; dst_off <= max_offset; ++dst_off) {
        for (int src_off = 0; src_off <= max_offset; ++src_off) {
            for (int count = 0; count <= max_count; count += count_step) {
                total_tests++;
                print_progress(total_tests, ((max_offset + 1) * (max_offset + 1) * ((max_count / count_step) + 1)));
                    std::cout << "\nTEST: dst_offset=" << dst_off 
                              << ", src_offset=" << src_off 
                              << ", count=" << count << "\n";
                if (test_fast_memcpy_single(dst_off, src_off, count)) {
                    passed_tests++;
                } else {
                    failed_tests++;
                    std::cout << "\n❌ FAILED: dst_offset=" << dst_off 
                              << ", src_offset=" << src_off 
                              << ", count=" << count << "\n";
                    
                    // Stop on first failure for debugging
                    if (failed_tests >= 1) {
                        std::cout << "\nStopping on first failure for debugging.\n";
                        std::cout << "You can debug with: gdb ./test_fast_memcpy\n";
                        std::cout << "  Break at: break test_fast_memcpy_single\n";
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
        return 0;
    } else {
        std::cout << "\n❌ SOME TESTS FAILED ❌\n";
        return 1;
    }
}
