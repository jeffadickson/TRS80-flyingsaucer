//! Call one routine of the program in a fresh machine, many times (batch).
//!
//! callrun <file.cmd> < cases.txt > results.txt
//! Each input line:  ENTRY A F B C D E H L IXh IXl IYh IYl ; POKES ; READS
//!   numbers in hex; POKES = "addr=hexbytes,addr=hexbytes"; READS = "addr:len,addr:len"
//! Each output line: A F B C D E H L IX IY cycles ; hexbytes for each READ (space separated)
use std::io::{BufRead, Write};
use z80emu::trs80::System;

fn main() {
    let path = std::env::args().nth(1).expect("cmd file");
    let cmd = std::fs::read(&path).unwrap();
    let mut base = System::new();
    base.m.load_cmd(&cmd);
    let stdin = std::io::stdin(); let stdout = std::io::stdout(); let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line.unwrap(); if line.trim().is_empty() { continue; }
        let parts: Vec<&str> = line.split(';').collect();
        let regs: Vec<u16> = parts[0].split_whitespace().map(|x| u16::from_str_radix(x, 16).unwrap()).collect();
        let mut sys = System::new();
        sys.m.mem.copy_from_slice(&base.m.mem);
        if parts.len() > 1 { for p in parts[1].split(',').filter(|s| !s.trim().is_empty()) {
            let (a, h) = p.trim().split_once('=').unwrap(); let mut a = usize::from_str_radix(a, 16).unwrap();
            for k in (0..h.len()).step_by(2) { sys.m.mem[a] = u8::from_str_radix(&h[k..k + 2], 16).unwrap(); a += 1; }
        } }
        let c = &mut sys.cpu;
        c.pc = regs[0]; c.a = regs[1] as u8; c.f = regs[2] as u8; c.b = regs[3] as u8; c.c = regs[4] as u8;
        c.d = regs[5] as u8; c.e = regs[6] as u8; c.h = regs[7] as u8; c.l = regs[8] as u8;
        c.ix = regs[9] << 8 | regs[10]; c.iy = regs[11] << 8 | regs[12];
        c.sp = 0xBFF0; sys.m.mem[0xBFF0] = 0xF0; sys.m.mem[0xBFF1] = 0xFF; sys.m.mem[0xFFF0] = 0x76; // return -> HALT at FFF0
        let mut n = 0u64;
        while sys.cpu.pc != 0xFFF0 && n < 50_000_000 { sys.step(); n += 1; }
        let c = &sys.cpu;
        write!(out, "{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:04X} {:04X} {} ;", c.a, c.f, c.b, c.c, c.d, c.e, c.h, c.l, c.ix, c.iy, c.cycles).unwrap();
        if parts.len() > 2 { for r in parts[2].split(',').filter(|s| !s.trim().is_empty()) {
            let (a, l) = r.trim().split_once(':').unwrap(); let a = usize::from_str_radix(a, 16).unwrap(); let l = usize::from_str_radix(l, 16).unwrap();
            write!(out, " ").unwrap(); for k in 0..l { write!(out, "{:02X}", sys.m.mem[a + k]).unwrap(); }
        } }
        writeln!(out).unwrap();
    }
}
