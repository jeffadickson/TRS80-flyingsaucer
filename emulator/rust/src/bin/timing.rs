//! Checks T-state counts and R-register increments of the Z80 core against an
//! independent reference table typed in from the Zilog Z80 CPU User Manual
//! (UM0080) instruction tables. The tables below are written out literally,
//! opcode by opcode; they are NOT derived from the core's decoding.
//!
//! R reference: the manual states R is incremented after each instruction
//! (opcode) fetch, i.e. once per M1 cycle. Every prefix byte (CB, ED, DD, FD)
//! is fetched by its own M1, so: unprefixed = 1, CB xx = 2, ED xx = 2,
//! DD/FD xx = 2, DD/FD CB d xx = 2 (the final opcode byte of DD CB d op is
//! read by an ordinary memory read, not M1). A repeating block instruction
//! re-fetches ED xx each iteration, so each iteration = 2.
//!
//! Run: cargo run --release --bin timing
use z80emu::z80::{Bus, Z80, FC, FP, FS, FZ};

struct Ram { mem: Vec<u8> }
impl Bus for Ram {
    fn read(&mut self, a: u16) -> u8 { self.mem[a as usize] }
    fn write(&mut self, a: u16, v: u8) { self.mem[a as usize] = v; }
    fn input(&mut self, _p: u16) -> u8 { 0x00 }
    fn output(&mut self, _p: u16, _v: u8) {}
}

const ORG: u16 = 0x1000;

/// Execute one instruction `code` (operand bytes default 0x00) after `setup`.
fn exec(code: &[u8], setup: &dyn Fn(&mut Z80, &mut Ram)) -> (u64, u8, Z80) {
    let mut ram = Ram { mem: vec![0; 65536] };
    ram.mem[ORG as usize..ORG as usize + code.len()].copy_from_slice(code);
    let mut z = Z80::new();
    z.pc = ORG; z.sp = 0xC000; z.set_hl(0x8000); z.set_de(0x9000); z.set_bc(0x0101);
    z.ix = 0x8000; z.iy = 0x8000; z.r = 0x10;
    setup(&mut z, &mut ram);
    let r0 = z.r; let c0 = z.cycles;
    z.step(&mut ram);
    (z.cycles - c0, z.r.wrapping_sub(r0) & 0x7F, z)
}

// ---- Zilog UM0080, unprefixed opcodes. Conditional ones: "taken" value. 0 = prefix byte.
const MAIN: [[u32; 16]; 16] = [
    //  0   1   2   3   4   5   6   7    8   9   A   B   C   D   E   F
    [   4, 10,  7,  6,  4,  4,  7,  4,   4, 11,  7,  6,  4,  4,  7,  4], // 0x  NOP LD BC,nn LD (BC),A INC BC ... EX AF ADD HL,BC LD A,(BC) DEC BC ...
    [  13, 10,  7,  6,  4,  4,  7,  4,  12, 11,  7,  6,  4,  4,  7,  4], // 1x  DJNZ(taken) ... JR e
    [  12, 10, 16,  6,  4,  4,  7,  4,  12, 11, 16,  6,  4,  4,  7,  4], // 2x  JR NZ, LD (nn),HL ... DAA, JR Z, LD HL,(nn), CPL
    [  12, 10, 13,  6, 11, 11, 10,  4,  12, 11, 13,  6,  4,  4,  7,  4], // 3x  JR NC, LD SP,nn, LD (nn),A, INC SP, INC (HL), DEC (HL), LD (HL),n, SCF ...
    [   4,  4,  4,  4,  4,  4,  7,  4,   4,  4,  4,  4,  4,  4,  7,  4], // 4x  LD B,r / LD C,r
    [   4,  4,  4,  4,  4,  4,  7,  4,   4,  4,  4,  4,  4,  4,  7,  4], // 5x
    [   4,  4,  4,  4,  4,  4,  7,  4,   4,  4,  4,  4,  4,  4,  7,  4], // 6x
    [   7,  7,  7,  7,  7,  7,  4,  7,   4,  4,  4,  4,  4,  4,  7,  4], // 7x  LD (HL),r, HALT, LD A,r
    [   4,  4,  4,  4,  4,  4,  7,  4,   4,  4,  4,  4,  4,  4,  7,  4], // 8x  ADD/ADC
    [   4,  4,  4,  4,  4,  4,  7,  4,   4,  4,  4,  4,  4,  4,  7,  4], // 9x  SUB/SBC
    [   4,  4,  4,  4,  4,  4,  7,  4,   4,  4,  4,  4,  4,  4,  7,  4], // Ax  AND/XOR
    [   4,  4,  4,  4,  4,  4,  7,  4,   4,  4,  4,  4,  4,  4,  7,  4], // Bx  OR/CP
    [  11, 10, 10, 10, 17, 11,  7, 11,  11, 10, 10,  0, 17, 17,  7, 11], // Cx  RET NZ POP BC JP NZ JP CALL NZ PUSH ADD n RST0 RET Z RET JP Z [CB] CALL Z CALL ADC n RST8
    [  11, 10, 10, 11, 17, 11,  7, 11,  11,  4, 10, 11, 17,  0,  7, 11], // Dx  ... OUT (n),A ... EXX ... IN A,(n) ... [DD]
    [  11, 10, 10, 19, 17, 11,  7, 11,  11,  4, 10,  4, 17,  0,  7, 11], // Ex  ... EX (SP),HL ... JP (HL) ... EX DE,HL ... [ED]
    [  11, 10, 10,  4, 17, 11,  7, 11,  11,  6, 10,  4, 17,  0,  7, 11], // Fx  ... DI ... LD SP,HL ... EI ... [FD]
];

// not-taken timings for conditional instructions (Zilog UM0080)
fn not_taken(op: u8) -> Option<u32> {
    match op {
        0x10 => Some(8),                                            // DJNZ, B becomes 0
        0x20 | 0x28 | 0x30 | 0x38 => Some(7),                       // JR cc
        0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => Some(5),   // RET cc
        0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => Some(10),  // JP cc
        0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => Some(10),  // CALL cc
        _ => None,
    }
}

/// flag value that makes condition cc (0..7 = NZ Z NC C PO PE P M) true/false
fn flags_for(cc: u8, truth: bool) -> u8 {
    let bit = [FZ, FZ, FC, FC, FP, FP, FS, FS][cc as usize];
    let set_means_true = cc & 1 == 1;
    if set_means_true == truth { bit } else { 0 }
}

// ---- CB: Zilog: rot/shift r 8, (HL) 15; BIT b,r 8, (HL) 12; RES/SET r 8, (HL) 15.
fn cb_ref(op: u8) -> u32 {
    let hl = op & 7 == 6;
    match op >> 6 { 1 => if hl { 12 } else { 8 }, _ => if hl { 15 } else { 8 } }
}

// ---- ED documented opcodes (Zilog UM0080). (opcode, T-states, name)
const ED: &[(u8, u32, &str)] = &[
    (0x40, 12, "IN B,(C)"), (0x41, 12, "OUT (C),B"), (0x42, 15, "SBC HL,BC"), (0x43, 20, "LD (nn),BC"),
    (0x44, 8, "NEG"), (0x45, 14, "RETN"), (0x46, 8, "IM 0"), (0x47, 9, "LD I,A"),
    (0x48, 12, "IN C,(C)"), (0x49, 12, "OUT (C),C"), (0x4A, 15, "ADC HL,BC"), (0x4B, 20, "LD BC,(nn)"),
    (0x4D, 14, "RETI"), (0x4F, 9, "LD R,A"),
    (0x50, 12, "IN D,(C)"), (0x51, 12, "OUT (C),D"), (0x52, 15, "SBC HL,DE"), (0x53, 20, "LD (nn),DE"),
    (0x56, 8, "IM 1"), (0x57, 9, "LD A,I"),
    (0x58, 12, "IN E,(C)"), (0x59, 12, "OUT (C),E"), (0x5A, 15, "ADC HL,DE"), (0x5B, 20, "LD DE,(nn)"),
    (0x5E, 8, "IM 2"), (0x5F, 9, "LD A,R"),
    (0x60, 12, "IN H,(C)"), (0x61, 12, "OUT (C),H"), (0x62, 15, "SBC HL,HL"), (0x63, 20, "LD (nn),HL [ED]"),
    (0x67, 18, "RRD"),
    (0x68, 12, "IN L,(C)"), (0x69, 12, "OUT (C),L"), (0x6A, 15, "ADC HL,HL"), (0x6B, 20, "LD HL,(nn) [ED]"),
    (0x6F, 18, "RLD"),
    (0x72, 15, "SBC HL,SP"), (0x73, 20, "LD (nn),SP"),
    (0x78, 12, "IN A,(C)"), (0x79, 12, "OUT (C),A"), (0x7A, 15, "ADC HL,SP"), (0x7B, 20, "LD SP,(nn)"),
    (0xA0, 16, "LDI"), (0xA1, 16, "CPI"), (0xA2, 16, "INI"), (0xA3, 16, "OUTI"),
    (0xA8, 16, "LDD"), (0xA9, 16, "CPD"), (0xAA, 16, "IND"), (0xAB, 16, "OUTD"),
];
// repeating block instructions: (opcode, repeating iteration, final iteration)
const ED_REP: &[(u8, u32, u32, &str)] = &[
    (0xB0, 21, 16, "LDIR"), (0xB1, 21, 16, "CPIR"), (0xB2, 21, 16, "INIR"), (0xB3, 21, 16, "OTIR"),
    (0xB8, 21, 16, "LDDR"), (0xB9, 21, 16, "CPDR"), (0xBA, 21, 16, "INDR"), (0xBB, 21, 16, "OTDR"),
];

// ---- DD/FD documented forms (Zilog UM0080), T-states incl. the prefix.
const XY: &[(u8, u32, &str)] = &[
    (0x09, 15, "ADD IX,BC"), (0x19, 15, "ADD IX,DE"), (0x21, 14, "LD IX,nn"), (0x22, 20, "LD (nn),IX"),
    (0x23, 10, "INC IX"), (0x29, 15, "ADD IX,IX"), (0x2A, 20, "LD IX,(nn)"), (0x2B, 10, "DEC IX"),
    (0x34, 23, "INC (IX+d)"), (0x35, 23, "DEC (IX+d)"), (0x36, 19, "LD (IX+d),n"), (0x39, 15, "ADD IX,SP"),
    (0x46, 19, "LD B,(IX+d)"), (0x4E, 19, "LD C,(IX+d)"), (0x56, 19, "LD D,(IX+d)"), (0x5E, 19, "LD E,(IX+d)"),
    (0x66, 19, "LD H,(IX+d)"), (0x6E, 19, "LD L,(IX+d)"), (0x7E, 19, "LD A,(IX+d)"),
    (0x70, 19, "LD (IX+d),B"), (0x71, 19, "LD (IX+d),C"), (0x72, 19, "LD (IX+d),D"), (0x73, 19, "LD (IX+d),E"),
    (0x74, 19, "LD (IX+d),H"), (0x75, 19, "LD (IX+d),L"), (0x77, 19, "LD (IX+d),A"),
    (0x86, 19, "ADD A,(IX+d)"), (0x8E, 19, "ADC A,(IX+d)"), (0x96, 19, "SUB (IX+d)"), (0x9E, 19, "SBC A,(IX+d)"),
    (0xA6, 19, "AND (IX+d)"), (0xAE, 19, "XOR (IX+d)"), (0xB6, 19, "OR (IX+d)"), (0xBE, 19, "CP (IX+d)"),
    (0xE1, 14, "POP IX"), (0xE3, 23, "EX (SP),IX"), (0xE5, 15, "PUSH IX"), (0xE9, 8, "JP (IX)"), (0xF9, 10, "LD SP,IX"),
];
// DD CB d op, documented (HL)-column forms: rot/shift 23, BIT 20, RES/SET 23
fn xycb_ref(op: u8) -> u32 { if op >> 6 == 1 { 20 } else { 23 } }

fn main() {
    let mut checked = 0; let mut bad = 0;
    let mut report = |name: String, got: (u64, u8), want: (u32, u8)| {
        checked += 1;
        if got.0 != want.0 as u64 || got.1 != want.1 {
            bad += 1;
            println!("MISMATCH {:<28} core T={} R+{}  manual T={} R+{}", name, got.0, got.1, want.0, want.1);
        }
    };
    let none = |_: &mut Z80, _: &mut Ram| {};

    // ---- unprefixed
    for op in 0u16..256 {
        let op = op as u8;
        if matches!(op, 0xCB | 0xDD | 0xED | 0xFD) { continue; }
        let want = MAIN[(op >> 4) as usize][(op & 15) as usize];
        let x = op >> 6; let y = (op >> 3) & 7; let z = op & 7;
        let cc = if x == 3 && (z == 0 || z == 2 || z == 4) { Some(y) }
                 else if x == 0 && z == 0 && y >= 4 { Some(y - 4) } else { None };
        if op == 0x10 {
            let (t, r, _) = exec(&[op, 0], &|z, _| z.b = 2); report(format!("{:02X} DJNZ taken", op), (t, r), (want, 1));
            let (t, r, _) = exec(&[op, 0], &|z, _| z.b = 1); report(format!("{:02X} DJNZ not taken", op), (t, r), (not_taken(op).unwrap(), 1));
        } else if let Some(cc) = cc {
            let (t, r, _) = exec(&[op, 0, 0], &move |z, _| z.f = flags_for(cc, true)); report(format!("{:02X} cc taken", op), (t, r), (want, 1));
            let (t, r, _) = exec(&[op, 0, 0], &move |z, _| z.f = flags_for(cc, false)); report(format!("{:02X} cc not taken", op), (t, r), (not_taken(op).unwrap(), 1));
        } else {
            let (t, r, _) = exec(&[op, 0, 0], &none); report(format!("{:02X}", op), (t, r), (want, 1));
        }
    }
    // HALT: the instruction, then one halted cycle (CPU executes NOPs: 4 T, R+1)
    {
        let mut ram = Ram { mem: vec![0; 65536] }; ram.mem[ORG as usize] = 0x76;
        let mut z = Z80::new(); z.pc = ORG;
        z.step(&mut ram);
        let c1 = z.cycles; let r1 = z.r; z.step(&mut ram);
        report("76 HALT (halted cycle)".into(), (z.cycles - c1, z.r.wrapping_sub(r1) & 0x7F), (4, 1));
        report("76 HALT pc stays".into(), (z.pc as u64, 0), (ORG as u32, 0));
    }
    // ---- CB
    for op in 0u16..256 {
        let op = op as u8;
        let (t, r, _) = exec(&[0xCB, op], &none);
        let tag = if (0x30..0x38).contains(&op) { " (SLL, undocumented)" } else { "" };
        report(format!("CB {:02X}{}", op, tag), (t, r), (cb_ref(op), 2));
    }
    // ---- ED
    for &(op, want, name) in ED {
        let (t, r, _) = exec(&[0xED, op, 0, 0], &none);
        let rwant = if op == 0x4F { r } else { 2 }; // LD R,A overwrites R: T-states only
        report(format!("ED {:02X} {}", op, name), (t, r), (want, rwant));
    }
    for &(op, rep, fin, name) in ED_REP {
        // repeating iteration: BC/B = 2; CPxR: A differs from (HL)
        let (t, r, _) = exec(&[0xED, op], &|z, _| { z.set_bc(0x0202); z.a = 0xFF; });
        report(format!("ED {:02X} {} repeating", op, name), (t, r), (rep, 2));
        let fin_bc = if op & 2 != 0 { 0x0101 } else { 0x0001 };
        let (t2, r2, _) = exec(&[0xED, op], &move |z, _| { z.set_bc(fin_bc); z.a = 0xFF; });
        report(format!("ED {:02X} {} final", op, name), (t2, r2), (fin, 2));
        if op & 3 == 1 { // CPIR/CPDR also terminate on a match
            let (t, r, _) = exec(&[0xED, op], &|z, _| { z.set_bc(0x0202); z.a = 0x00; });
            report(format!("ED {:02X} {} final (match)", op, name), (t, r), (fin, 2));
        }
    }
    // ---- DD / FD
    for pre in [0xDDu8, 0xFD] {
        for &(op, want, name) in XY {
            let (t, r, _) = exec(&[pre, op, 0x05, 0x00], &none);
            report(format!("{:02X} {:02X} {}", pre, op, name.replace("IX", if pre == 0xDD { "IX" } else { "IY" })), (t, r), (want, 2));
        }
        for op in 0u16..256 {
            let op = op as u8;
            if op & 7 != 6 { continue; } // documented forms only
            let (t, r, _) = exec(&[pre, 0xCB, 0x05, op], &none);
            report(format!("{:02X} CB d {:02X}", pre, op), (t, r), (xycb_ref(op), 2));
        }
    }
    println!("timing: {} checks, {} mismatches", checked, bad);
    if bad != 0 { std::process::exit(1); }
}
