#include <stdarg.h>
#include <stdio.h>
#include "log.hpp"
#include "globals.hpp"

void asm_printf(const char *format, ...)
{
    // Flush any previous error message
    fflush(stderr);

    // Print the prefix first
    printf("[ASM %s] ", log_name);
    
    // Handle the variable arguments
    va_list args;
    va_start(args, format);
    vprintf(format, args);
    va_end(args);

    // Flush the output to ensure this message is printed immediately, in case we are exiting right
    // after this call
    fflush(stdout);
}
