#ifndef EMULATOR_ASM_C_PROVIDED_HPP
#define EMULATOR_ASM_C_PROVIDED_HPP

#include <stdint.h>

extern int _print_regs();
extern int _print_pc (uint64_t pc, uint64_t c);
extern void _chunk_done();
extern void _realloc_trace (void);
int _wait_for_prec_avail (void);

#endif // EMULATOR_ASM_C_PROVIDED_HPP