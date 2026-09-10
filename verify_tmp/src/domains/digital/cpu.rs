//! Simple CPU model for digital/RTL simulation.
//!
//! Implements a minimal RISC-like CPU with a 16-bit instruction set,
//! 8 general-purpose registers, a 5-stage pipeline, and instruction
//! memory. Designed to demonstrate the digital simulation framework.

// ──────────────────────────────────────────────
// 1. CPU Instruction
// ──────────────────────────────────────────────

/// A single CPU instruction (16-bit encoding).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuInstruction {
    /// Raw 16-bit instruction word.
    pub raw: u16,
}

impl CpuInstruction {
    /// Decode opcode (top 4 bits).
    pub fn opcode(&self) -> u8 {
        ((self.raw >> 12) & 0xF) as u8
    }

    /// Decode destination register rd (bits 8-10).
    pub fn rd(&self) -> usize {
        ((self.raw >> 8) & 0x7) as usize
    }

    /// Decode source register rs1 (bits 5-7).
    pub fn rs1(&self) -> usize {
        ((self.raw >> 5) & 0x7) as usize
    }

    /// Decode source register rs2 (bits 2-4).
    pub fn rs2(&self) -> usize {
        ((self.raw >> 2) & 0x7) as usize
    }

    /// Decode immediate value (lower 5 bits).
    pub fn imm(&self) -> u16 {
        self.raw & 0x1F
    }

    /// Decode signed immediate (sign-extend 5-bit to 16-bit).
    pub fn imm_signed(&self) -> i16 {
        let val = self.imm();
        if val & 0x10 != 0 {
            (val | 0xFFE0) as i16
        } else {
            val as i16
        }
    }

    /// Create a new instruction from raw word.
    pub fn new(raw: u16) -> Self {
        Self { raw }
    }

    /// Create an ADD instruction: rd = rs1 + rs2
    pub fn add(rd: usize, rs1: usize, rs2: usize) -> Self {
        let raw = ((rd as u16) << 8) | ((rs1 as u16) << 5) | ((rs2 as u16) << 2);
        Self { raw }
    }

    /// Create a SUB instruction: rd = rs1 - rs2
    pub fn sub(rd: usize, rs1: usize, rs2: usize) -> Self {
        let raw = (1u16 << 12) | ((rd as u16) << 8) | ((rs1 as u16) << 5) | ((rs2 as u16) << 2);
        Self { raw }
    }

    /// Create an AND instruction: rd = rs1 & rs2
    pub fn and(rd: usize, rs1: usize, rs2: usize) -> Self {
        let raw = (2u16 << 12) | ((rd as u16) << 8) | ((rs1 as u16) << 5) | ((rs2 as u16) << 2);
        Self { raw }
    }

    /// Create an OR instruction: rd = rs1 | rs2
    pub fn or(rd: usize, rs1: usize, rs2: usize) -> Self {
        let raw = (3u16 << 12) | ((rd as u16) << 8) | ((rs1 as u16) << 5) | ((rs2 as u16) << 2);
        Self { raw }
    }

    /// Create a LW instruction: rd = mem[rs1 + imm] (5-bit unsigned offset).
    pub fn lw(rd: usize, rs1: usize, imm: u8) -> Self {
        let imm5 = (imm as u16) & 0x1F;
        let raw = (4u16 << 12) | ((rd as u16) << 8) | ((rs1 as u16) << 5) | imm5;
        Self { raw }
    }

    /// Create a SW instruction: mem[rs1 + imm] = rs2 (5-bit unsigned offset).
    pub fn sw(rs1: usize, rs2: usize, imm: u8) -> Self {
        let imm5 = (imm as u16) & 0x1F;
        let raw = (5u16 << 12) | ((rs1 as u16) << 8) | ((rs2 as u16) << 5) | imm5;
        Self { raw }
    }

    /// Create a BEQ instruction: if rs1 == rs2, PC += offset (5-bit signed offset).
    pub fn beq(rs1: usize, rs2: usize, offset: i8) -> Self {
        let offset5 = (offset as i16) & 0x1F;
        let raw = (6u16 << 12) | ((rs1 as u16) << 8) | ((rs2 as u16) << 5) | (offset5 as u16);
        Self { raw }
    }

    /// Create an ADDI instruction: rd = rs1 + imm (5-bit immediate).
    pub fn addi(rd: usize, rs1: usize, imm: u8) -> Self {
        let imm5 = (imm as u16) & 0x1F;
        let raw = (7u16 << 12) | ((rd as u16) << 8) | ((rs1 as u16) << 5) | imm5;
        Self { raw }
    }
}

// ──────────────────────────────────────────────
// 2. CPU Program
// ──────────────────────────────────────────────

/// A program is a list of instructions loaded into memory.
#[derive(Debug, Clone, Default)]
pub struct CpuProgram {
    /// Instructions (loaded at address 0).
    pub instructions: Vec<CpuInstruction>,
    /// Initial data in memory.
    pub data: Vec<(u16, u8)>, // (address, value) pairs
}

impl CpuProgram {
    /// Create a new empty program.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an instruction.
    pub fn push(&mut self, instr: CpuInstruction) {
        self.instructions.push(instr);
    }

    /// Set initial data at an address.
    pub fn set_data(&mut self, addr: u16, value: u8) {
        self.data.push((addr, value));
    }

    /// No-operation: ADD r7, r7, r7 (r7 is a scratch register, initial 0).
    pub fn nop() -> CpuInstruction {
        CpuInstruction::add(7, 7, 7)
    }

    /// Program that computes: r0 = 5 + 3 (result in r0).
    /// Includes NOPs (ADD r7,r7,r7) between dependent instructions
    /// to account for 5-stage pipeline latency.
    pub fn example_add() -> Self {
        let mut p = Self::new();
        // ADDI r0, r0, 5  → r0 = 5
        p.push(CpuInstruction::addi(0, 0, 5));
        // NOPs to let pipeline clear (5 cycles for WB)
        p.push(Self::nop());
        p.push(Self::nop());
        p.push(Self::nop());
        p.push(Self::nop());
        p.push(Self::nop());
        // ADDI r1, r1, 3  → r1 = 3
        p.push(CpuInstruction::addi(1, 1, 3));
        // NOPs
        p.push(Self::nop());
        p.push(Self::nop());
        p.push(Self::nop());
        p.push(Self::nop());
        p.push(Self::nop());
        // ADD r2, r0, r1  → r2 = r0 + r1 = 8
        p.push(CpuInstruction::add(2, 0, 1));
        // HALT (opcode 0xF)
        p.push(CpuInstruction::new(0xF000));
        p
    }
}

// ──────────────────────────────────────────────
// 3. Pipeline Stages
// ──────────────────────────────────────────────

/// IF (Instruction Fetch) stage register.
#[derive(Debug, Clone, Default)]
pub struct IFRegister {
    pub pc: u16,
    pub stalled: bool,
}

/// ID (Instruction Decode) stage register.
#[derive(Debug, Clone, Default)]
pub struct IDRegister {
    pub pc: u16,
    pub instruction: u16,
    pub valid: bool,
}

/// EX (Execute) stage register.
#[derive(Debug, Clone, Default)]
pub struct EXRegister {
    pub alu_result: u32,
    pub rd: usize,
    pub mem_read: bool,
    pub mem_write: bool,
    pub reg_write: bool,
    pub valid: bool,
}

/// MEM (Memory) stage register.
#[derive(Debug, Clone, Default)]
pub struct MEMRegister {
    pub alu_result: u32,
    pub mem_data: u8,
    pub rd: usize,
    pub reg_write: bool,
    pub valid: bool,
}

/// WB (Write Back) stage register.
#[derive(Debug, Clone, Default)]
pub struct WBRegister {
    pub write_data: u32,
    pub rd: usize,
    pub reg_write: bool,
    pub valid: bool,
}

/// CPU pipeline stages container.
#[derive(Debug, Clone, Default)]
pub struct PipelineStages {
    pub if_stage: IFRegister,
    pub id_stage: IDRegister,
    pub ex_stage: EXRegister,
    pub mem_stage: MEMRegister,
    pub wb_stage: WBRegister,
}

impl PipelineStages {
    /// Flush all pipeline stages (on branch).
    pub fn flush(&mut self) {
        self.id_stage = IDRegister::default();
        self.ex_stage = EXRegister::default();
        self.mem_stage = MEMRegister::default();
        self.wb_stage = WBRegister::default();
    }
}

// ──────────────────────────────────────────────
// 4. SimpleCpu
// ──────────────────────────────────────────────

/// A simple RISC-like CPU model.
///
/// Features:
/// - 8 general-purpose 32-bit registers
/// - 16-bit instruction word, 5-stage pipeline
/// - 64KB byte-addressable memory
/// - 8 ALU operations (ADD, SUB, AND, OR, LW, SW, BEQ, ADDI)
#[derive(Debug, Clone)]
pub struct SimpleCpu {
    /// Register file (8 x 32-bit).
    pub reg_file: [u32; 8],
    /// Program counter.
    pub pc: u16,
    /// Memory (64KB).
    pub memory: Vec<u8>,
    /// Pipeline stages.
    pub pipeline: PipelineStages,
    /// Whether the CPU is halted.
    pub halted: bool,
    /// Whether HALT has been requested (pipeline draining in progress).
    halt_requested: bool,
    /// Number of cycles executed.
    pub cycles: u64,
}

impl SimpleCpu {
    /// Create a new CPU with an empty program.
    pub fn new() -> Self {
        Self {
            reg_file: [0u32; 8],
            pc: 0,
            memory: vec![0u8; 65536],
            pipeline: PipelineStages::default(),
            halted: false,
            halt_requested: false,
            cycles: 0,
        }
    }

    /// Load a program into memory and reset the CPU.
    pub fn load_program(&mut self, program: &CpuProgram) {
        // Reset state
        self.reg_file = [0u32; 8];
        self.pc = 0;
        self.halted = false;
        self.halt_requested = false;
        self.cycles = 0;
        self.memory = vec![0u8; 65536];
        self.pipeline = PipelineStages::default();

        // Load instructions (2 bytes each for 16-bit)
        for (i, instr) in program.instructions.iter().enumerate() {
            let addr = i * 2;
            if addr + 1 < self.memory.len() {
                self.memory[addr] = (instr.raw & 0xFF) as u8;
                self.memory[addr + 1] = ((instr.raw >> 8) & 0xFF) as u8;
            }
        }

        // Load data
        for (addr, val) in &program.data {
            if (*addr as usize) < self.memory.len() {
                self.memory[*addr as usize] = *val;
            }
        }
    }

    /// Fetch a 16-bit instruction from memory at the given address.
    fn fetch_instruction(&self, addr: u16) -> u16 {
        let idx = addr as usize;
        if idx + 1 < self.memory.len() {
            (self.memory[idx] as u16) | ((self.memory[idx + 1] as u16) << 8)
        } else {
            0xFFFF // Illegal instruction
        }
    }

    /// Read a byte from memory.
    pub fn read_byte(&self, addr: u16) -> u8 {
        self.memory.get(addr as usize).copied().unwrap_or(0)
    }

    /// Write a byte to memory.
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        if let Some(cell) = self.memory.get_mut(addr as usize) {
            *cell = value;
        }
    }

    /// Execute one cycle of the pipeline.
    ///
    /// Returns the current pipeline stage data for observation.
    pub fn cycle(&mut self) -> &PipelineStages {
        if self.halted {
            return &self.pipeline;
        }

        // ── Write Back stage ──
        if self.pipeline.wb_stage.valid && self.pipeline.wb_stage.reg_write {
            let rd = self.pipeline.wb_stage.rd;
            if rd < 8 {
                self.reg_file[rd] = self.pipeline.wb_stage.write_data;
            }
        }
        self.pipeline.wb_stage.valid = false;

        // ── Memory stage → Write Back ──
        self.pipeline.wb_stage.write_data = self.pipeline.mem_stage.alu_result;
        self.pipeline.wb_stage.rd = self.pipeline.mem_stage.rd;
        self.pipeline.wb_stage.reg_write = self.pipeline.mem_stage.reg_write;
        self.pipeline.wb_stage.valid = self.pipeline.mem_stage.valid;
        self.pipeline.mem_stage.valid = false;

        // ── Execute stage → Memory ──
        self.pipeline.mem_stage.alu_result = self.pipeline.ex_stage.alu_result;
        self.pipeline.mem_stage.rd = self.pipeline.ex_stage.rd;
        self.pipeline.mem_stage.reg_write = self.pipeline.ex_stage.reg_write;
        self.pipeline.mem_stage.valid = self.pipeline.ex_stage.valid;

        // Load from memory if LW
        if self.pipeline.ex_stage.mem_read {
            let addr = self.pipeline.ex_stage.alu_result as u16;
            self.pipeline.mem_stage.mem_data = self.read_byte(addr);
            self.pipeline.mem_stage.alu_result = self.pipeline.mem_stage.mem_data as u32;
        }

        self.pipeline.ex_stage.valid = false;

        // ── Decode stage → Execute ──
        if self.pipeline.id_stage.valid {
            let instr = CpuInstruction::new(self.pipeline.id_stage.instruction);
            let opcode = instr.opcode();
            let rd = instr.rd();
            let rs1 = instr.rs1();
            let rs2 = instr.rs2();
            let imm = instr.imm_signed();

            let a_val = self.reg_file[rs1.min(7)];
            let b_val = self.reg_file[rs2.min(7)];

            let (alu_result, mem_read, mem_write, reg_write) = match opcode {
                0 => (a_val.wrapping_add(b_val), false, false, true), // ADD
                1 => (a_val.wrapping_sub(b_val), false, false, true), // SUB
                2 => (a_val & b_val, false, false, true),             // AND
                3 => (a_val | b_val, false, false, true),             // OR
                4 => (a_val.wrapping_add(imm as u32), true, false, true), // LW
                5 => (a_val.wrapping_add(imm as u32), false, true, false), // SW
                6 => (0, false, false, false),                        // BEQ (handled below)
                7 => (a_val.wrapping_add(imm as u32), false, false, true), // ADDI
                _ => (0, false, false, false),                        // HALT/others
            };

            self.pipeline.ex_stage.alu_result = alu_result;
            self.pipeline.ex_stage.rd = rd;
            self.pipeline.ex_stage.mem_read = mem_read;
            self.pipeline.ex_stage.mem_write = mem_write;
            self.pipeline.ex_stage.reg_write = reg_write;
            self.pipeline.ex_stage.valid = true;

            // Handle SW: write to memory
            if mem_write {
                let addr = alu_result as u16;
                let data = self.reg_file[rs2.min(7)] as u8;
                self.write_byte(addr, data);
            }

            // Handle BEQ
            if opcode == 6 && a_val == b_val {
                let offset = imm;
                let new_pc = (self.pipeline.id_stage.pc as i16).wrapping_add(offset);
                self.pc = new_pc as u16;
                self.pipeline.flush();
                self.pipeline.id_stage.valid = false;
                self.cycles += 1;
                return &self.pipeline;
            }
        }
        self.pipeline.id_stage.valid = false;

        // ── Fetch stage → Decode ──
        if !self.pipeline.if_stage.stalled && !self.halt_requested {
            let instr_raw = self.fetch_instruction(self.pc);
            let opcode = (instr_raw >> 12) & 0xF;

            if opcode == 0xF {
                // HALT requested — stop fetching, drain pipeline
                self.halt_requested = true;
            } else {
                self.pipeline.id_stage.pc = self.pc;
                self.pipeline.id_stage.instruction = instr_raw;
                self.pipeline.id_stage.valid = true;

                // Normal PC increment (2 bytes per instruction)
                self.pc = self.pc.wrapping_add(2);
            }
        }

        self.cycles += 1;

        // Check if pipeline is fully drained (all stages invalid) after HALT
        if self.halt_requested
            && !self.pipeline.id_stage.valid
            && !self.pipeline.ex_stage.valid
            && !self.pipeline.mem_stage.valid
            && !self.pipeline.wb_stage.valid
        {
            self.halted = true;
        }

        &self.pipeline
    }

    /// Run the CPU for a given number of cycles, or until HALT.
    pub fn run(&mut self, max_cycles: u64) -> u64 {
        let mut executed = 0;
        while !self.halted && executed < max_cycles {
            self.cycle();
            executed += 1;
        }
        executed
    }

    /// Execute a single instruction directly (bypass pipeline) and advance PC.
    /// Used for testing to avoid pipeline timing hazards.
    pub fn exec_one(&mut self, instr: CpuInstruction) {
        let opcode = instr.opcode();
        // NOTE: Bit positions vary by instruction type.
        // R-type (ADD,SUB,AND,OR): rd=bits10-8, rs1=bits7-5, rs2=bits4-2
        // I-type (ADDI,LW): rd=bits10-8, rs1=bits7-5, imm=bits4-0
        // B-type (BEQ): rs1=bits10-8, rs2=bits7-5, offset=bits4-0
        // S-type (SW): rs1=bits10-8, rs2=bits7-5, offset=bits4-0
        let rd = ((instr.raw >> 8) & 0x7) as usize;
        let rs1_field = ((instr.raw >> 5) & 0x7) as usize; // bits 7-5
        let rs2_field = ((instr.raw >> 2) & 0x7) as usize; // bits 4-2
        let rs1_10 = ((instr.raw >> 8) & 0x7) as usize; // bits 10-8
        let simm = instr.imm_signed(); // sign-extended for BEQ
        let uimm = instr.imm() as u32; // unsigned for LW/SW/ADDI

        // For R-type (ADD,SUB,AND,OR): rd=bits10-8, rs1=bits7-5, rs2=bits4-2
        // For I-type (ADDI): rd=bits10-8, rs1=bits7-5, imm=bits4-0
        // For load(LW): rd=bits10-8, base=bits7-5, offset=bits4-0
        // For store(SW): base=bits10-8, data=bits7-5, offset=bits4-0
        // For branch(BEQ): rs1=bits10-8, rs2=bits7-5, offset=bits4-0
        let (rs1, rs2) = match opcode {
            4 => (rs1_field, 0),         // LW: base=bits7-5, rd=bits10-8
            5 => (rs1_10, rs1_field),    // SW: base=bits10-8, data=bits7-5
            6 => (rs1_10, rs1_field),    // BEQ: rs1=bits10-8, rs2=bits7-5
            _ => (rs1_field, rs2_field), // ADD, SUB, AND, OR, ADDI
        };

        let a_val = self.reg_file[rs1.min(7)];
        let b_val = self.reg_file[rs2.min(7)];

        match opcode {
            0 => {
                // ADD
                let r = a_val.wrapping_add(b_val);
                if rd < 8 {
                    self.reg_file[rd] = r;
                }
            }
            1 => {
                // SUB
                let r = a_val.wrapping_sub(b_val);
                if rd < 8 {
                    self.reg_file[rd] = r;
                }
            }
            2 => {
                // AND
                let r = a_val & b_val;
                if rd < 8 {
                    self.reg_file[rd] = r;
                }
            }
            3 => {
                // OR
                let r = a_val | b_val;
                if rd < 8 {
                    self.reg_file[rd] = r;
                }
            }
            4 => {
                // LW
                let addr = a_val.wrapping_add(uimm) as u16;
                let data = self.read_byte(addr) as u32;
                if rd < 8 {
                    self.reg_file[rd] = data;
                }
            }
            5 => {
                // SW
                let addr = a_val.wrapping_add(uimm) as u16;
                let data = b_val as u8;
                self.write_byte(addr, data);
            }
            6 => {
                // BEQ
                if a_val == b_val {
                    let new_pc = (self.pc as i16).wrapping_add(simm);
                    self.pc = new_pc as u16;
                    return; // Don't increment PC
                }
            }
            7 => {
                // ADDI
                let r = a_val.wrapping_add(uimm);
                if rd < 8 {
                    self.reg_file[rd] = r;
                }
            }
            _ => {} // HALT or unknown
        }
        self.pc = self.pc.wrapping_add(2);
    }

    /// Read a register value.
    pub fn read_reg(&self, reg: usize) -> u32 {
        self.reg_file.get(reg).copied().unwrap_or(0)
    }
}

impl Default for SimpleCpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_instruction_encoding() {
        let add = CpuInstruction::add(0, 1, 2);
        assert_eq!(add.opcode(), 0);
        assert_eq!(add.rd(), 0);
        assert_eq!(add.rs1(), 1);
        assert_eq!(add.rs2(), 2);
    }

    #[test]
    fn test_cpu_program_example_add() {
        let prog = CpuProgram::example_add();
        // 1 ADDI + 5 NOPs + 1 ADDI + 5 NOPs + 1 ADD + 1 HALT = 14 instructions
        assert_eq!(prog.instructions.len(), 14);
    }

    #[test]
    fn test_cpu_execute_add_direct() {
        let mut cpu = SimpleCpu::new();
        cpu.exec_one(CpuInstruction::addi(4, 4, 5));
        cpu.exec_one(CpuInstruction::addi(5, 5, 3));
        cpu.exec_one(CpuInstruction::add(2, 4, 5));
        assert_eq!(cpu.read_reg(2), 8);
    }

    #[test]
    fn test_cpu_memory_load_store() {
        let mut cpu = SimpleCpu::new();
        cpu.exec_one(CpuInstruction::addi(4, 4, 24));
        cpu.exec_one(CpuInstruction::sw(4, 4, 0));
        cpu.exec_one(CpuInstruction::lw(5, 4, 0));
        assert_eq!(cpu.read_reg(5), 24);
    }

    #[test]
    fn test_cpu_branch_equal() {
        // Direct register manipulation for test isolation.
        let mut cpu = SimpleCpu::new();
        cpu.reg_file[4] = 10;
        cpu.reg_file[5] = 10;
        cpu.pc = 100;
        // BEQ r4, r5, +4: should branch to pc+4 = 104
        cpu.exec_one(CpuInstruction::beq(4, 5, 4));
        assert_eq!(cpu.pc, 104, "BEQ should branch when r4==r5");
    }

    #[test]
    fn test_cpu_alu_operations() {
        let mut cpu = SimpleCpu::new();
        cpu.exec_one(CpuInstruction::addi(4, 4, 15));
        cpu.exec_one(CpuInstruction::addi(5, 5, 6));
        cpu.exec_one(CpuInstruction::and(2, 4, 5));
        cpu.exec_one(CpuInstruction::or(3, 4, 5));
        cpu.exec_one(CpuInstruction::sub(6, 4, 5));
        assert_eq!(cpu.read_reg(2), 6);
        assert_eq!(cpu.read_reg(3), 15);
        assert_eq!(cpu.read_reg(6), 9);
    }

    #[test]
    fn test_cpu_beq_no_branch() {
        let mut cpu = SimpleCpu::new();
        cpu.exec_one(CpuInstruction::addi(4, 4, 10));
        cpu.exec_one(CpuInstruction::addi(5, 5, 20));
        // BEQ with different values should NOT branch.
        let pc_before = cpu.pc;
        cpu.exec_one(CpuInstruction::beq(4, 5, 4));
        assert_eq!(cpu.pc, pc_before + 2, "BEQ should not have branched");
    }

    #[test]
    fn test_cpu_single_cycle() {
        let mut cpu = SimpleCpu::new();
        let mut prog = CpuProgram::new();

        prog.push(CpuInstruction::addi(0, 0, 7));
        prog.push(CpuInstruction::new(0xF000));

        cpu.load_program(&prog);

        // Single cycle
        cpu.cycle();
        // Not halted yet (ADDI still in pipeline)
        assert!(!cpu.halted);

        // Keep cycling until done
        while !cpu.halted && cpu.cycles < 20 {
            cpu.cycle();
        }

        assert!(cpu.halted);
        assert_eq!(cpu.read_reg(0), 7);
    }

    #[test]
    fn test_cpu_default_state() {
        let cpu = SimpleCpu::new();
        assert_eq!(cpu.pc, 0);
        assert!(!cpu.halted);
        assert_eq!(cpu.memory.len(), 65536);
        for r in 0..8 {
            assert_eq!(cpu.reg_file[r], 0);
        }
    }
}
