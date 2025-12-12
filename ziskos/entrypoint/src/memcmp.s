        .section ".note.GNU-stack","",@progbits
        .text
        .attribute      4, 16
        .attribute      5, "rv64im"
        .globl  memcmp
        .p2align        4
        .type   memcmp,@function
memcmp:
        add	a0,a0,a1
        .insn	4, 0x81362073
        ret
/*
                beqz	a2, .memcmp_eq
    .memcmp_loop:	
                lbu	    a3,0(a0)
                lbu	    a4,0(a1)
                bne	    a3,a4, .memcmp_neq
                addi	a2,a2,-1
                addi	a1,a1,1
                addi	a0,a0,1
                bnez	a2, .memcmp_loop
    .memcmp_eq:	
                li	    a0,0
                ret
    .memcmp_neq:
                sub	    a0,a3,a4
                ret
*/                
        .size memcmp, .-memcmp
        .section .text.hot,"ax",@progbits