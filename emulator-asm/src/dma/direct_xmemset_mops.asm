.intel_syntax noprefix
.code64

################################################################################
# memset_mops - Optimized memset with memory ops tracing
#
# This function performs two main tasks:
# 1. Records all addresses of memory operations (read and write addresses)
# 2. Performs the actual memset operation filling dst with the byte value
#
# REGISTER USAGE:
# Uses general-purpose registers: rax, rbx, rcx, rdx, rdi, rsi, r8, r9, r12, r13
# Does NOT use XMM registers (caller doesn't need to save them)
# Preserves callee-saved registers (rbx, r12, r13 saved/restored in wrapper)
#
# PARAMETERS (NON System V AMD64 ABI):
#   rdi = dst (u64)                     - Destination address to fill
#   rsi = value (u8 in low byte)        - Byte value to set (0-255)
#   rdx = count (usize)                 - Number of bytes to set
#   r12 = mops_base_addr (u64*)         - Pointer to memory ops trace buffer base
#   r13 = mops_index (usize)            - Current index in mops buffer (input/output)
#
################################################################################

.global direct_dma_xmemset_mops
.global dma_xmemset_mops
.extern fast_dma_encode

.include "dma_constants.inc"


.section .text

# Standard ABI wrapper that saves/restores callee-saved registers
# and initializes mops tracking state before calling direct implementation 
dma_xmemset_mops:

    # Save callee-saved registers
    push    r12         # ~3 cycles - save r12 (used as mops base address)
    push    r13         # ~3 cycles - save r13 (used as mops index)
    push    rbx         # ~3 cycles - save rbx
    
    mov     r12, rcx              # 1 cycle - r12 = mops buffer base address
    xor     r13, r13              # 1 cycle - r13 = 0 (initialize mops index)
    call    direct_dma_xmemset_mops  # ~5 cycles + function cost

    mov     rax, r13              # 1 cycle - return mops count in rax
    pop     rbx                   # ~3 cycles - restore rbx
    pop     r13                   # ~3 cycles - restore r13
    pop     r12                   # ~3 cycles - restore r12

    ret                           # ~5 cycles

# Direct entry point for assembly callers (no ABI overhead)
# More efficient when caller manages register preservation

direct_dma_xmemset_mops:
   
    # Modified registers (caller must handle): 
    #       rax = encoded metadata (output from fast_dma_encode)
    #       rcx = scratch register (loop counter)
    #       rdi = destination pointer (modified by rep stosq/stosb)
    #       rsi = value preserved until expansion
    #       r8  = scratch for value expansion
    #       r9  = scratch for mops address calculation
    #       r13 = mops index (incremented, output)

    # Call fast_dma_encode to calculate encoding
    # Parameters already in correct registers: rdi=dst, rsi=value, rdx=count
    # Result will be returned in rax (encoded metadata)

    call    fast_dma_encode         # ~15-20 cycles - table lookup encoding

    # Check if count is zero
    test    rdx, rdx                # 1 cycle - check if count == 0
    jz      .L_done                 # 2 cycles (unlikely) - nothing to do

.L_pre_dst_to_mops:
    # If pre_count > 0, record aligned dst read address to mops
    test    rax, PRE_COUNT_MASK        # 1 cycle - check if pre_count > 0
    jz      .L_post_dst_to_mops        # 2 cycles (predicted taken)

.L_pre_is_active:
    # Record aligned read of dst qword that will be partially overwritten
    mov     r9, MOPS_ALIGNED_READ      # 1 cycle - r9 = flags for aligned read
    add     r9, rdi                    # 1 cycle - r9 = flags + dst address
    and     r9, ALIGN_MASK             # 1 cycle - align address to 8-byte boundary
    mov     [r12 + r13 * 8], r9        # ~4 cycles - write mops entry (aligned read)
    inc     r13                        # 1 cycle - advance mops index

.L_post_dst_to_mops:

    # If post_count > 0, record aligned (dst+count) read address to mops
    test    rax, POST_COUNT_MASK       # 1 cycle - check if post_count > 0
    jz      .L_src_to_mops             # 2 cycles (predicted taken) - skip if no post bytes

.L_post_is_active:
    # Record aligned read of dst qword at end that will be partially overwritten
    mov     rcx, MOPS_ALIGNED_READ     # 1 cycle - rcx = flags for aligned read
    lea     r9, [rdi + rdx - 1]        # 1 cycle - r9 = dst + count - 1 (last byte address)
    and     r9, ALIGN_MASK             # 1 cycle - align address to 8-byte boundary
    add     r9, rcx                    # 1 cycle - r9 = flags + aligned address
    mov     [r12 + r13 * 8], r9        # ~4 cycles - write mops entry (aligned read)
    inc     r13                        # 1 cycle - advance mops index

.L_src_to_mops:
    # Record block write operation (aligned writes to dst)
    mov     rcx, rax                    # 1 cycle - rcx = encoded metadata
    shr     rcx, LOOP_COUNT_RS          # 1 cycle - extract loop_count (qwords to write)
    shl     rcx, MOPS_BLOCK_WORDS_RS    # 1 cycle - shift to block words field position
  
    mov     r9, rax                        # 1 cycle - r9 = encoded metadata
    and     r9, PRE_WRITES_MASK            # 1 cycle - extract pre_writes count
    shl     r9, PRE_WRITES_TO_MOPS_BLOCK   # 1 cycle - shift pre_writes to correct position
    add     r9, rcx                        # 1 cycle - combine loop_count and pre_writes
    add     r9, rdi                        # 1 cycle - add base dst address

    mov     rcx, MOPS_ALIGNED_BLOCK_WRITE  # 1 cycle - rcx = block write flags
    add     r9, rcx                        # 1 cycle - r9 = flags + address + counts
    and     r9, ALIGN_MASK                 # 1 cycle - align address field

    mov     [r12 + r13 * 8], r9            # ~4 cycles - write mops entry (block write)
    inc     r13                            # 1 cycle - advance mops index
    jmp     .L_mops_done                   # ~2 cycles - skip to memset

.L_mops_done:  

    # Perform actual memset: fill dst with byte value using rep stosq + rep stosb
    # This is the optimal approach on modern x86-64 (no alignment overhead needed)
    
    movzx   rax, sil                   # 1 cycle - rax = byte value (zero-extended)
    mov     r8, 0x0101010101010101     # 1 cycle - r8 = replication pattern
    imul    rax, r8                    # ~3 cycles - rax = byte value replicated 8 times
    
    mov     rcx, rdx                   # 1 cycle - rcx = count (total bytes)
    shr     rcx, 3                     # 1 cycle - rcx = count / 8 (qwords to write)
    rep     stosq                      # ~0.5-1.0 cycles per qword (fast string ops)
    
    mov     rcx, rdx                   # 1 cycle - rcx = count (original)
    and     rcx, 7                     # 1 cycle - rcx = count % 8 (remaining bytes)
    rep     stosb                      # ~1.0-2.0 cycles per byte (remaining 0-7 bytes)

.L_done:
    ret                                # ~5 cycles

# Performance estimate (Modern x86-64, Intel Skylake/AMD Zen+, L1 cache hits):
#
# MEMSET OPERATION WITH MOPS TRACING:
# - fast_dma_encode call:           ~15-20 cycles (function call + table lookup)
# - Pre-read mops entry:            ~8-10 cycles (if pre_count > 0: calc + and + store + inc)
# - Post-read mops entry:           ~10-12 cycles (if post_count > 0: lea + and + add + store + inc)
# - Block write mops entry:         ~12-15 cycles (extract + shift + combine + store + inc)
# - Byte value expansion:           ~5-6 cycles (movzx + mov + imul)
# - Qword fill (rep stosq):         ~0.5-1.0 cycles per qword (ERMSB optimization)
# - Remaining bytes (rep stosb):    ~1.0-2.0 cycles per byte (0-7 bytes)
# - Function overhead:              ~3-5 cycles (branches, return)
#
# TOTAL (typical case, 64 bytes, aligned, no pre/post):
#   ~15 (encode) + ~15 (block mops) + ~6 (expand) + 8*0.75 (fill) + ~4 (overhead)
#   = ~46 cycles (~1.39 GB/s @ 3 GHz)
#
# TOTAL (misaligned case, 64 bytes with pre/post):
#   ~15 (encode) + ~10 (pre) + ~12 (post) + ~15 (block) + ~6 (expand) + 7*0.75 + 4*1.5 (fill) + ~4
#   = ~73 cycles (~0.88 GB/s @ 3 GHz)
#
# TOTAL (large fill, 4096 bytes, aligned):
#   ~15 (encode) + ~15 (mops) + ~6 (expand) + 512*0.5 (fill) + ~4 (overhead)
#   = ~296 cycles (~13.8 GB/s @ 3 GHz, approaching L1D bandwidth)
#
# NOTES:
# - Assumes L1D cache hits for all memory accesses (~4 cycle latency, ~64 GB/s bandwidth)
# - rep stosq/stosb uses Enhanced REP MOVSB/STOSB (ERMSB) on modern CPUs (post-2013)
# - ERMSB enables microcode to use wide stores (16-64 bytes per iteration internally)
# - For fills >256 bytes, performance approaches memory bandwidth limits
# - Actual cycles vary ±20-30% by microarchitecture (Skylake/Zen/Alder Lake)
# - Mops overhead: ~30-50 cycles base + minimal per-byte impact
# - No overlap handling needed for memset (writes only, no read-modify-write hazards)

# Mark stack as non-executable (required by modern linkers)
.section .note.GNU-stack,"",%progbits
