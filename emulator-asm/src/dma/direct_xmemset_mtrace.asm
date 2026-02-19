.intel_syntax noprefix
.code64

################################################################################
# memset_mtrace - Optimized memset with memory ops tracing
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
# MTRACE SIZE CONTROL:
#
# xmemset use at maximum 3 qwords to store encode, pre and post, for this reason this
# function doesn't require analyze if need call to realloc.
#
# PARAMETERS (NON System V AMD64 ABI):
#   rdi = dst (u64)                     - Destination address to fill
#   rsi = value (u8 in low byte)        - Byte value to set (0-255)
#   rdx = count (usize)                 - Number of bytes to set
#   r12 = mops_base_addr (u64*)         - Pointer to memory ops trace buffer base
#   r13 = mops_index (usize)            - Current index in mops buffer (input/output)
#
################################################################################

.global direct_dma_xmemset_mtrace
.global dma_xmemset_mtrace
.extern fast_memset

.include "dma_constants.inc"

.section .text

# Standard ABI wrapper that saves/restores callee-saved registers
# and initializes mops tracking state before calling direct implementation 

# rdi = destination
# rsi: byte to write
# rdx: count (bytes)
# rcx = pointer to data trace
# rax = return qword of trace

dma_xmemset_mtrace:

    # Save callee-saved registers
    push    r12         # ~3 cycles - save r12 (used as mops base address)
    push    r13         # ~3 cycles - save r13 (used as mops index)
    
    mov     r12, rcx              # 1 cycle - r12 = mops buffer base address
    xor     r13, r13              # 1 cycle - r13 = 0 (initialize mops index)
    call    direct_dma_xmemset_mtrace  # ~5 cycles + function cost

    mov     rax, r13              # 1 cycle - return mops count in rax
    pop     r13                   # ~3 cycles - restore r13
    pop     r12                   # ~3 cycles - restore r12

    ret                           # ~5 cycles

# Direct entry point for assembly callers (no ABI overhead)
# More efficient when caller manages register preservation

# arguments:
# rdi: destination adress
# rsi: byte to write
# rdx: count (bytes)
# r12 + r13: mops trace

direct_dma_xmemset_mtrace:
   
    # Modified registers (caller must handle): 
    #       r9  = scratch for mops address calculation
    #       rcx = mops index (incremented, output)

    # test count = 0
    test    rdx, rdx
    jz      .L_xmemset_mtrace_count_zero

    # test dst aligned
    test    rdi, 0x7
    jnz     .L_xmemset_mtrace_rdi_unaligned

    # test count multiple of 8
    test    rdx, 0x07
    jnz     .L_memset_mtrace_count_remain

    # FAST BRANCH
    # dst is aligned, count is a multiple of 8 and greater than zero
    # => no pre-reads, only encoding

    # FAST BRANCH - MTRACE (ENCODING)

    # FAST DIRECT ENCODING

    # encode loop count, how count is multiple of 8, direct shift
    mov     r9, rdx
    shl     r9, PRE_AND_LOOP_BYTES_RS

    # encode fill byte
    movzx   eax, sil
    shl     rax, FILL_BYTE_CMP_RES_RS
    add     rax, r9

    # store encoded on mtrace
    mov     [r12 + r13 * 8], rax
    inc     r13

    jmp     fast_memset

.L_memset_mtrace_count_remain:
    # BRANCH 1
    # dst is aligned, but count is NOT a multiple of 8,
    # => one pre-read (post) 
    # NOTE: if count ∈ [1,7] no problem, because you need to do pre-read

    # BRANCH 1 - MTRACE (ENCODING + ALIGNED_READ (POST)

    # encode fill byte
    movzx   eax, sil
    shl     rax, FILL_BYTE_CMP_RES_RS

    # encode post count
    mov     r9, rdx
    and     r9, 0x07
    shl     r9, POST_COUNT_RS

    # encode = fill byte + post_count
    add     rax, r9
    
    # encode += template
    or      rax, ENCODE_MEMSET_ALIGNED_NO_COUNT_M8

    # encode loop count (count % 8 == 0)    
    mov     r9, rdx
    and     r9, ALIGN_MASK
    shl     r9, PRE_AND_LOOP_BYTES_RS
    add     rax, r9

    # store encode to mtrace
    mov     [r12 + r13 * 8], rax

    # unshift loop count, r9 containts count64 
    shr     r9, PRE_AND_LOOP_BYTES_RS + 3

    # BRANCH 1 - specific pre-read part
    # r9 contains count64 to index need to substract 1*8
    lea     rcx, [rdi + r9 * 8 - 8]
    mov     [r12 + r13 * 8 + 8], rcx
    add     r13, 2

    jmp     fast_memset

.L_xmemset_mtrace_rdi_unaligned:
    # BRANCH 2 - worse
    # dst is NOT aligned 
    # => BRANCH 2.1 one pre-read (pre) + no post
    # => BRANCH 2.2 one pre-read (pre) + second post pre-read
    
    # [EC] only PRE but [rdi + rdx] & 0x07 !== 0

    call    fast_dma_encode_memset_with_byte
    mov     [r12 + r13 * 8], rax

    test    rax, PRE_COUNT_MASK
    jz      .L_xmemset_mtrace_rdi_unaligned_no_pre

    mov     r9, rdi
    and     r9, ALIGN_MASK
    mov     rcx, [r9]
    mov     [r12 + r13 * 8 + 8], rcx

    test    rax, POST_COUNT_MASK
    jz      .L_xmemset_mtrace_rdi_unaligned_pre_no_post

    mov     rcx, rax
    shr     rcx, LOOP_COUNT_RS

    # r9 = dst & 0x07 of previous calculation
    # r9 + loop * 8 + 8 for pre part

    mov     rcx, [r9 + 8 + rcx * 8]        
    mov     [r12 + r13 * 8 + 16], rcx
    add     r13, 3    

    jmp     fast_memset

.L_xmemset_mtrace_rdi_unaligned_pre_no_post:
    add     r13, 2

    jmp     fast_memset

.L_xmemset_mtrace_rdi_unaligned_no_pre:
    test    rax, POST_COUNT_MASK
    jz      .L_xmemset_mtrace_rdi_unaligned_no_pre_no_post

    mov     r9, rdi
    and     r9, ALIGN_MASK

    mov     rcx, rax
    shr     rcx, LOOP_COUNT_RS

    # r9 = dst & 0x07 of previous calculation
    # rdi + loop * 8 + 8 for pre part

    mov     rcx, [r9 + 8 + rcx * 8]        
    mov     [r12 + r13 * 8 + 8], rcx
    add     r13, 2

    jmp     fast_memset

.L_xmemset_mtrace_rdi_unaligned_no_pre_no_post:
    inc     r13

    jmp     fast_memset

.L_xmemset_mtrace_count_zero:

    # encode in fast way, for zero-lenght memset

    # encode dst offset
    mov     r9, rdi
    and     r9, 0x07
    shl     r9, DST_OFFSET_RS

    # encode fill byte
    movzx   eax, sil
    shl     rax, FILL_BYTE_CMP_RES_RS
    add     r9, rax

    # encode template of MEMSET_ZERO
    add     r9, ENCODE_MEMSET_ZERO

    # add encode to mtrace
    mov     [r12 + r13 * 8], r9
    inc     r13

    jmp     fast_memset

.L_xmemset_mtrace_done:
    ret    

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
