//! Runs ZEXDOC/ZEXALL CP/M exercisers on the Z80 core.
use z80emu::z80::{Bus, Z80};
use std::io::Write;
struct Cpm { mem: Vec<u8> }
impl Bus for Cpm {
    fn read(&mut self, a: u16) -> u8 { self.mem[a as usize] }
    fn write(&mut self, a: u16, v: u8) { self.mem[a as usize] = v; }
    fn input(&mut self, _p: u16) -> u8 { 0xFF }
    fn output(&mut self, _p: u16, _v: u8) {}
}
fn main() {
    let path = std::env::args().nth(1).expect("zexdoc.com path");
    let prog = std::fs::read(&path).unwrap();
    let mut bus = Cpm { mem: vec![0; 65536] };
    bus.mem[0x100..0x100 + prog.len()].copy_from_slice(&prog);
    bus.mem[5] = 0xC9; // RET at BDOS entry; trapped below
    bus.mem[6] = 0x00; bus.mem[7] = 0xF0; // top of TPA used by zex for SP
    let mut cpu = Z80::new(); cpu.pc = 0x100; cpu.sp = 0xF000;
    let out = std::io::stdout(); let mut out = out.lock();
    loop {
        if cpu.pc == 5 {
            match cpu.c {
                2 => { out.write_all(&[cpu.e]).unwrap(); }
                9 => { let mut a = cpu.de(); while bus.mem[a as usize] != b'$' { out.write_all(&[bus.mem[a as usize]]).unwrap(); a = a.wrapping_add(1); } }
                _ => {}
            }
            out.flush().unwrap();
        }
        if cpu.pc == 0 { break; }
        cpu.step(&mut bus);
    }
    println!("\nTotal T-states: {}", cpu.cycles);
}
