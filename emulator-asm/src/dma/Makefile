# Makefile for encode_memcpy_inline test

CXX = g++
CXXFLAGS = -std=c++17 -O0 -Wall -Wextra -g -no-pie
# CXXFLAGS = -std=c++17 -O2 -Wall -Wextra -g -no-pie
LDFLAGS = -no-pie
ASMFLAGS = -g --noexecstack -I$(CURDIR)

SRCS_ASM = direct_memcpy_mtrace.asm memcpy_fast.asm fast_dma_encode.asm direct_memcpy_mops.asm direct_xmemset_mops.asm direct_xmemset_mtrace.asm fast_memset.asm fast_memcpy.asm direct_xinput_mops.asm
SRCS_CPP = test_dma.cpp test_dma_memset_mops.cpp test_dma_memset_mtrace.cpp test_fast_memcpy.cpp test_dma_inputcpy_mops.cpp
OBJS_ASM = direct_memcpy_mtrace.o memcpy_fast.o fast_dma_encode.o direct_memcpy_mops.o
OBJS = $(OBJS_ASM) test_dma.o
TARGET = test_dma
TARGET_MEMSET = test_dma_memset_mops
TARGET_MEMSET_MTRACE = test_dma_memset_mtrace
TARGET_FAST_MEMCPY = test_fast_memcpy
TARGET_INPUTCPY = test_dma_inputcpy_mops

# Memset MOPS-specific objects
OBJS_MEMSET_ASM = direct_xmemset_mops.o fast_memset.o
OBJS_MEMSET = $(OBJS_MEMSET_ASM) test_dma_memset_mops.o

# Memset MTRACE-specific objects
OBJS_MEMSET_MTRACE_ASM = direct_xmemset_mtrace.o fast_memset.o fast_dma_encode.o fast_memcpy.o
OBJS_MEMSET_MTRACE = $(OBJS_MEMSET_MTRACE_ASM) test_dma_memset_mtrace.o

# Fast memcpy test objects
OBJS_FAST_MEMCPY = fast_memcpy.o test_fast_memcpy.o

# Inputcpy test objects
OBJS_INPUTCPY = direct_xinput_mops.o fast_memcpy.o test_dma_inputcpy_mops.o

all: $(TARGET) $(TARGET_MEMSET) $(TARGET_MEMSET_MTRACE) $(TARGET_FAST_MEMCPY) $(TARGET_INPUTCPY)

%.o: %.asm
	as $(ASMFLAGS) -o $@ $<

%.o: %.cpp
	$(CXX) $(CXXFLAGS) -c -o $@ $<

$(TARGET): $(OBJS)
	$(CXX) $(LDFLAGS) -o $@ $^

$(TARGET_MEMSET): $(OBJS_MEMSET)
	$(CXX) $(LDFLAGS) -o $@ $^

$(TARGET_MEMSET_MTRACE): $(OBJS_MEMSET_MTRACE)
	$(CXX) $(LDFLAGS) -o $@ $^

$(TARGET_FAST_MEMCPY): $(OBJS_FAST_MEMCPY)
	$(CXX) $(LDFLAGS) -o $@ $^

$(TARGET_INPUTCPY): $(OBJS_INPUTCPY)
	$(CXX) $(LDFLAGS) -o $@ $^

test: $(TARGET)
	./$(TARGET)

test_memset: $(TARGET_MEMSET)
	./$(TARGET_MEMSET)

test_memset_mtrace: $(TARGET_MEMSET_MTRACE)
	./$(TARGET_MEMSET_MTRACE)

run_fast_memcpy: $(TARGET_FAST_MEMCPY)
	./$(TARGET_FAST_MEMCPY)

test_inputcpy: $(TARGET_INPUTCPY)
	./$(TARGET_INPUTCPY)

clean:
	rm -f $(OBJS) $(OBJS_MEMSET) $(OBJS_MEMSET_MTRACE) $(OBJS_FAST_MEMCPY) $(OBJS_INPUTCPY) $(TARGET) $(TARGET_MEMSET) $(TARGET_MEMSET_MTRACE) $(TARGET_FAST_MEMCPY) $(TARGET_INPUTCPY)

.PHONY: all test test_memset test_memset_mtrace run_fast_memcpy test_inputcpy clean
