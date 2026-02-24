use riscv::RiscvInstruction;

pub fn dma_direct_calls(
    riscv_instructions: &Vec<RiscvInstruction>,
    dma_addrs: (u64, u64, u64, u64),
) {
    let (memcpy_addr, memcmp_addr, memset_addr, memmove_addr) = dma_addrs;

    // Detect AUIPC + JALR patterns that jump to DMA functions
    // Pattern: AUIPC rd, imm20 followed by JALR ra, rd, imm12
    // Target address = PC + (imm20 << 12) + imm12
    let mut dma_call_pcs: std::collections::HashSet<u64> = std::collections::HashSet::new();

    // Register numbers for a0, a1, a2 (x10, x11, x12)
    const REG_A0: u32 = 10;
    const REG_A1: u32 = 11;
    const REG_A2: u32 = 12;

    // Types of register loading
    #[derive(Debug, Clone)]
    enum RegLoadType {
        /// Immediate value: li rd, imm or addi rd, x0, imm
        Immediate(i32),
        /// Register + immediate: addi rd, rs, imm or mv rd, rs (imm=0)
        RegPlusImm { rs: u32, imm: i32 },
        /// Load from memory: ld/lw rd, imm(rs)
        MemLoad { rs: u32, imm: i32, size: u8 },
        /// Not found
        NotFound,
    }

    /// Find how a register is loaded before a given instruction index
    fn find_reg_load(
        riscv_instructions: &[RiscvInstruction],
        call_idx: usize,
        target_reg: u32,
    ) -> (usize, RegLoadType) {
        // Search backwards from call_idx
        for offset in 1..=20.min(call_idx) {
            let idx = call_idx - offset;
            let inst = &riscv_instructions[idx];

            // Check if this instruction writes to target_reg
            if inst.rd != target_reg {
                continue;
            }

            // li rd, imm -> actually addi rd, x0, imm or lui+addi
            // addi rd, rs, imm
            if inst.inst == "addi" || inst.inst == "c.addi" {
                if inst.rs1 == 0 {
                    // li rd, imm (addi rd, x0, imm)
                    return (offset, RegLoadType::Immediate(inst.imm));
                } else {
                    // addi rd, rs, imm
                    return (offset, RegLoadType::RegPlusImm { rs: inst.rs1, imm: inst.imm });
                }
            }

            // c.li rd, imm
            if inst.inst == "c.li" {
                return (offset, RegLoadType::Immediate(inst.imm));
            }

            // mv rd, rs -> addi rd, rs, 0 or c.mv
            if inst.inst == "c.mv" {
                return (offset, RegLoadType::RegPlusImm { rs: inst.rs2, imm: 0 });
            }

            // lui rd, imm (upper immediate)
            if inst.inst == "lui" || inst.inst == "c.lui" {
                return (offset, RegLoadType::Immediate(inst.imm));
            }

            // ld rd, imm(rs) - 64-bit load
            if inst.inst == "ld" || inst.inst == "c.ld" || inst.inst == "c.ldsp" {
                return (offset, RegLoadType::MemLoad { rs: inst.rs1, imm: inst.imm, size: 8 });
            }

            // lw rd, imm(rs) - 32-bit load
            if inst.inst == "lw" || inst.inst == "c.lw" || inst.inst == "c.lwsp" {
                return (offset, RegLoadType::MemLoad { rs: inst.rs1, imm: inst.imm, size: 4 });
            }

            // lwu rd, imm(rs) - 32-bit unsigned load
            if inst.inst == "lwu" {
                return (offset, RegLoadType::MemLoad { rs: inst.rs1, imm: inst.imm, size: 4 });
            }

            // add rd, rs1, rs2 (could be mv if rs2=0 or rs1=0)
            if inst.inst == "add" || inst.inst == "c.add" {
                if inst.rs1 == 0 {
                    return (offset, RegLoadType::RegPlusImm { rs: inst.rs2, imm: 0 });
                } else if inst.rs2 == 0 {
                    return (offset, RegLoadType::RegPlusImm { rs: inst.rs1, imm: 0 });
                }
                // General add - treat as reg+imm with imm=0 (approximate)
                return (offset, RegLoadType::RegPlusImm { rs: inst.rs1, imm: 0 });
            }

            // Other instruction that writes to target_reg but we don't recognize
            // Stop searching since the register was overwritten
            return (offset, RegLoadType::NotFound);
        }
        (0, RegLoadType::NotFound)
    }

    fn reg_load_to_string(load: &RegLoadType) -> String {
        match load {
            RegLoadType::Immediate(imm) => format!("imm={}", imm),
            RegLoadType::RegPlusImm { rs, imm } => format!("x{}+{}", rs, imm),
            RegLoadType::MemLoad { rs, imm, size } => format!("{}(x{})[{}B]", imm, rs, size),
            RegLoadType::NotFound => "???".to_string(),
        }
    }

    for i in 0..riscv_instructions.len().saturating_sub(1) {
        let inst = &riscv_instructions[i];
        let next_inst = &riscv_instructions[i + 1];

        // Check for AUIPC + JALR pattern
        if inst.inst == "auipc" && next_inst.inst == "jalr" {
            // AUIPC uses the same register that JALR reads from
            if inst.rd == next_inst.rs1 {
                // Calculate target address: PC + imm_auipc + imm_jalr
                // Note: inst.imm for AUIPC is already shifted (has lower 12 bits as zero)
                let target = (inst.rom_address as i64)
                    .wrapping_add(inst.imm as i64)
                    .wrapping_add(next_inst.imm as i64) as u64;

                // Check if target is a DMA function
                let is_dma = (memcpy_addr != 0 && target == memcpy_addr)
                    || (memcmp_addr != 0 && target == memcmp_addr)
                    || (memset_addr != 0 && target == memset_addr)
                    || (memmove_addr != 0 && target == memmove_addr);

                if is_dma {
                    let func_name = if target == memcpy_addr {
                        "memcpy"
                    } else if target == memcmp_addr {
                        "memcmp"
                    } else if target == memset_addr {
                        "memset"
                    } else {
                        "inputcpy"
                    };

                    // Find how a0, a1, a2 are loaded (search from JALR position = i+1)
                    let jalr_idx = i + 1;
                    let (a0_offset, a0_load) = find_reg_load(riscv_instructions, jalr_idx, REG_A0);
                    let (a1_offset, a1_load) = find_reg_load(riscv_instructions, jalr_idx, REG_A1);
                    let (a2_offset, a2_load) = find_reg_load(riscv_instructions, jalr_idx, REG_A2);

                    // Check if any register load was not found
                    let has_missing = matches!(a0_load, RegLoadType::NotFound)
                        || matches!(a1_load, RegLoadType::NotFound)
                        || matches!(a2_load, RegLoadType::NotFound);

                    println!(
                        "DMA call to {} at PC=0x{:x}:{}",
                        func_name,
                        next_inst.rom_address,
                        if has_missing { " [INCOMPLETE - needs analysis]" } else { "" }
                    );
                    println!("  a0: offset=-{}, {}", a0_offset, reg_load_to_string(&a0_load));
                    println!("  a1: offset=-{}, {}", a1_offset, reg_load_to_string(&a1_load));
                    println!("  a2: offset=-{}, {}", a2_offset, reg_load_to_string(&a2_load));

                    dma_call_pcs.insert(inst.rom_address);
                    dma_call_pcs.insert(next_inst.rom_address);
                }
            }
        }

        // Check for JAL (direct jump) pattern
        if inst.inst == "jal" {
            let target = (inst.rom_address as i64).wrapping_add(inst.imm as i64) as u64;

            let is_dma = (memcpy_addr != 0 && target == memcpy_addr)
                || (memcmp_addr != 0 && target == memcmp_addr)
                || (memset_addr != 0 && target == memset_addr)
                || (memmove_addr != 0 && target == memmove_addr);

            if is_dma {
                let func_name = if target == memcpy_addr {
                    "memcpy"
                } else if target == memcmp_addr {
                    "memcmp"
                } else if target == memset_addr {
                    "memset"
                } else {
                    "inputcpy"
                };

                // Find how a0, a1, a2 are loaded
                let (a0_offset, a0_load) = find_reg_load(riscv_instructions, i, REG_A0);
                let (a1_offset, a1_load) = find_reg_load(riscv_instructions, i, REG_A1);
                let (a2_offset, a2_load) = find_reg_load(riscv_instructions, i, REG_A2);

                // Check if any register load was not found
                let has_missing = matches!(a0_load, RegLoadType::NotFound)
                    || matches!(a1_load, RegLoadType::NotFound)
                    || matches!(a2_load, RegLoadType::NotFound);

                println!(
                    "DMA call to {} at PC=0x{:x} (JAL):{}",
                    func_name,
                    inst.rom_address,
                    if has_missing { " [INCOMPLETE - needs analysis]" } else { "" }
                );
                println!("  a0: offset=-{}, {}", a0_offset, reg_load_to_string(&a0_load));
                println!("  a1: offset=-{}, {}", a1_offset, reg_load_to_string(&a1_load));
                println!("  a2: offset=-{}, {}", a2_offset, reg_load_to_string(&a2_load));

                dma_call_pcs.insert(inst.rom_address);
            }
        }
    }
}
