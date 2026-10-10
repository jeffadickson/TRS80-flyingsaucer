//! Z80 CPU core. Instruction-stepped, T-state counted, with undocumented
//! flags (X/Y), MEMPTR (WZ), refresh register R and DD/FD/CB/ED prefixes.
//! Validated with ZEXDOC and ZEXALL (see tests/zex.rs and README).

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn input(&mut self, port: u16) -> u8;
    fn output(&mut self, port: u16, val: u8);
}

pub const FS: u8 = 0x80;
pub const FZ: u8 = 0x40;
pub const FY: u8 = 0x20;
pub const FH: u8 = 0x10;
pub const FX: u8 = 0x08;
pub const FP: u8 = 0x04;
pub const FN: u8 = 0x02;
pub const FC: u8 = 0x01;

#[derive(Clone, Default, Debug)]
pub struct Z80 {
    pub a: u8, pub f: u8, pub b: u8, pub c: u8, pub d: u8, pub e: u8, pub h: u8, pub l: u8,
    pub a_: u8, pub f_: u8, pub b_: u8, pub c_: u8, pub d_: u8, pub e_: u8, pub h_: u8, pub l_: u8,
    pub ix: u16, pub iy: u16, pub sp: u16, pub pc: u16,
    pub i: u8, pub r: u8,
    pub iff1: bool, pub iff2: bool, pub im: u8,
    pub halted: bool,
    pub wz: u16,
    pub cycles: u64,
    pub ei_pending: bool,
}

fn szp(v: u8) -> u8 {
    let mut f = v & (FS | FY | FX);
    if v == 0 { f |= FZ; }
    if v.count_ones() % 2 == 0 { f |= FP; }
    f
}

// index mode for DD/FD
#[derive(Clone, Copy, PartialEq)]
enum Idx { HL, IX, IY }

impl Z80 {
    pub fn new() -> Self { let mut z = Z80::default(); z.sp = 0xFFFF; z.a = 0xFF; z.f = 0xFF; z }

    #[inline] pub fn bc(&self) -> u16 { (self.b as u16) << 8 | self.c as u16 }
    #[inline] pub fn de(&self) -> u16 { (self.d as u16) << 8 | self.e as u16 }
    #[inline] pub fn hl(&self) -> u16 { (self.h as u16) << 8 | self.l as u16 }
    #[inline] pub fn af(&self) -> u16 { (self.a as u16) << 8 | self.f as u16 }
    #[inline] pub fn set_bc(&mut self, v: u16) { self.b = (v >> 8) as u8; self.c = v as u8; }
    #[inline] pub fn set_de(&mut self, v: u16) { self.d = (v >> 8) as u8; self.e = v as u8; }
    #[inline] pub fn set_hl(&mut self, v: u16) { self.h = (v >> 8) as u8; self.l = v as u8; }
    #[inline] pub fn set_af(&mut self, v: u16) { self.a = (v >> 8) as u8; self.f = v as u8; }

    #[inline] fn inc_r(&mut self) { self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F); }

    #[inline] fn fetch<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let v = bus.read(self.pc); self.pc = self.pc.wrapping_add(1); v
    }
    #[inline] fn fetch16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.fetch(bus) as u16; let hi = self.fetch(bus) as u16; hi << 8 | lo
    }
    #[inline] fn rd16<B: Bus>(&mut self, bus: &mut B, a: u16) -> u16 {
        bus.read(a) as u16 | (bus.read(a.wrapping_add(1)) as u16) << 8
    }
    #[inline] fn wr16<B: Bus>(&mut self, bus: &mut B, a: u16, v: u16) {
        bus.write(a, v as u8); bus.write(a.wrapping_add(1), (v >> 8) as u8);
    }
    #[inline] fn push<B: Bus>(&mut self, bus: &mut B, v: u16) {
        self.sp = self.sp.wrapping_sub(1); bus.write(self.sp, (v >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1); bus.write(self.sp, v as u8);
    }
    #[inline] fn pop<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = bus.read(self.sp) as u16; self.sp = self.sp.wrapping_add(1);
        let hi = bus.read(self.sp) as u16; self.sp = self.sp.wrapping_add(1);
        hi << 8 | lo
    }

    fn get_rp(&self, p: u8, ix: Idx) -> u16 {
        match p { 0 => self.bc(), 1 => self.de(), 2 => self.get_xy(ix), _ => self.sp }
    }
    fn set_rp(&mut self, p: u8, ix: Idx, v: u16) {
        match p { 0 => self.set_bc(v), 1 => self.set_de(v), 2 => self.set_xy(ix, v), _ => self.sp = v }
    }
    fn get_xy(&self, ix: Idx) -> u16 { match ix { Idx::HL => self.hl(), Idx::IX => self.ix, Idx::IY => self.iy } }
    fn set_xy(&mut self, ix: Idx, v: u16) { match ix { Idx::HL => self.set_hl(v), Idx::IX => self.ix = v, Idx::IY => self.iy = v } }

    // 8-bit register by index, with IXH/IXL substitution (r != 6)
    fn get_r(&self, r: u8, ix: Idx) -> u8 {
        match r {
            0 => self.b, 1 => self.c, 2 => self.d, 3 => self.e,
            4 => match ix { Idx::HL => self.h, Idx::IX => (self.ix >> 8) as u8, Idx::IY => (self.iy >> 8) as u8 },
            5 => match ix { Idx::HL => self.l, Idx::IX => self.ix as u8, Idx::IY => self.iy as u8 },
            7 => self.a, _ => unreachable!(),
        }
    }
    fn set_r(&mut self, r: u8, ix: Idx, v: u8) {
        match r {
            0 => self.b = v, 1 => self.c = v, 2 => self.d = v, 3 => self.e = v,
            4 => match ix { Idx::HL => self.h = v, Idx::IX => self.ix = (self.ix & 0xFF) | (v as u16) << 8, Idx::IY => self.iy = (self.iy & 0xFF) | (v as u16) << 8 },
            5 => match ix { Idx::HL => self.l = v, Idx::IX => self.ix = (self.ix & 0xFF00) | v as u16, Idx::IY => self.iy = (self.iy & 0xFF00) | v as u16 },
            7 => self.a = v, _ => unreachable!(),
        }
    }

    fn cond(&self, y: u8) -> bool {
        match y {
            0 => self.f & FZ == 0, 1 => self.f & FZ != 0,
            2 => self.f & FC == 0, 3 => self.f & FC != 0,
            4 => self.f & FP == 0, 5 => self.f & FP != 0,
            6 => self.f & FS == 0, _ => self.f & FS != 0,
        }
    }

    // ---------- ALU ----------
    fn alu(&mut self, op: u8, v: u8) {
        let a = self.a;
        match op {
            0 | 1 => { // ADD / ADC
                let c = if op == 1 { (self.f & FC) as u16 } else { 0 };
                let r16 = a as u16 + v as u16 + c; let r = r16 as u8;
                let mut f = r & (FS | FY | FX);
                if r == 0 { f |= FZ; }
                if (a ^ v ^ r) & 0x10 != 0 { f |= FH; }
                if (!(a ^ v) & (a ^ r)) & 0x80 != 0 { f |= FP; }
                if r16 > 0xFF { f |= FC; }
                self.a = r; self.f = f;
            }
            2 | 3 | 7 => { // SUB / SBC / CP
                let c = if op == 3 { (self.f & FC) as u16 } else { 0 };
                let r16 = (a as u16).wrapping_sub(v as u16).wrapping_sub(c); let r = r16 as u8;
                let mut f = (r & FS) | FN;
                if r == 0 { f |= FZ; }
                if (a ^ v ^ r) & 0x10 != 0 { f |= FH; }
                if ((a ^ v) & (a ^ r)) & 0x80 != 0 { f |= FP; }
                if r16 > 0xFF { f |= FC; }
                if op == 7 { f |= v & (FY | FX); } else { f |= r & (FY | FX); self.a = r; }
                self.f = f;
            }
            4 => { self.a &= v; self.f = szp(self.a) | FH; }
            5 => { self.a ^= v; self.f = szp(self.a); }
            _ => { self.a |= v; self.f = szp(self.a); }
        }
    }
    fn inc8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        let mut f = (self.f & FC) | (r & (FS | FY | FX));
        if r == 0 { f |= FZ; }
        if v & 0x0F == 0x0F { f |= FH; }
        if v == 0x7F { f |= FP; }
        self.f = f; r
    }
    fn dec8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        let mut f = (self.f & FC) | FN | (r & (FS | FY | FX));
        if r == 0 { f |= FZ; }
        if v & 0x0F == 0 { f |= FH; }
        if v == 0x80 { f |= FP; }
        self.f = f; r
    }
    fn add16(&mut self, a: u16, b: u16) -> u16 {
        let r = a as u32 + b as u32;
        let mut f = self.f & (FS | FZ | FP);
        if (a ^ b ^ r as u16) & 0x1000 != 0 { f |= FH; }
        if r > 0xFFFF { f |= FC; }
        f |= ((r >> 8) as u8) & (FY | FX);
        self.f = f; self.wz = a.wrapping_add(1); r as u16
    }
    fn adc16(&mut self, a: u16, b: u16) -> u16 {
        let c = (self.f & FC) as u32;
        let r = a as u32 + b as u32 + c; let r16 = r as u16;
        let mut f = ((r16 >> 8) as u8) & (FS | FY | FX);
        if r16 == 0 { f |= FZ; }
        if (a ^ b ^ r16) & 0x1000 != 0 { f |= FH; }
        if (!(a ^ b) & (a ^ r16)) & 0x8000 != 0 { f |= FP; }
        if r > 0xFFFF { f |= FC; }
        self.f = f; self.wz = a.wrapping_add(1); r16
    }
    fn sbc16(&mut self, a: u16, b: u16) -> u16 {
        let c = (self.f & FC) as u32;
        let r = (a as u32).wrapping_sub(b as u32).wrapping_sub(c); let r16 = r as u16;
        let mut f = (((r16 >> 8) as u8) & (FS | FY | FX)) | FN;
        if r16 == 0 { f |= FZ; }
        if (a ^ b ^ r16) & 0x1000 != 0 { f |= FH; }
        if ((a ^ b) & (a ^ r16)) & 0x8000 != 0 { f |= FP; }
        if r > 0xFFFF { f |= FC; }
        self.f = f; self.wz = a.wrapping_add(1); r16
    }
    fn rot(&mut self, y: u8, v: u8) -> u8 {
        let c = self.f & FC;
        let (r, nc) = match y {
            0 => (v.rotate_left(1), v >> 7),
            1 => (v.rotate_right(1), v & 1),
            2 => ((v << 1) | c, v >> 7),
            3 => ((v >> 1) | (c << 7), v & 1),
            4 => (v << 1, v >> 7),
            5 => ((v >> 1) | (v & 0x80), v & 1),
            6 => ((v << 1) | 1, v >> 7),   // SLL (undocumented)
            _ => (v >> 1, v & 1),
        };
        self.f = szp(r) | nc; r
    }

    pub fn interrupt<B: Bus>(&mut self, bus: &mut B) -> bool {
        if !self.iff1 || self.ei_pending { return false; }
        if self.halted { self.halted = false; self.pc = self.pc.wrapping_add(1); }
        self.iff1 = false; self.iff2 = false; self.inc_r();
        match self.im {
            2 => { let pc = self.pc; self.push(bus, pc); let v = (self.i as u16) << 8 | 0xFF; self.pc = self.rd16(bus, v); self.cycles += 19; }
            _ => { let pc = self.pc; self.push(bus, pc); self.pc = 0x38; self.cycles += 13; }
        }
        self.wz = self.pc;
        true
    }

    /// Execute one instruction. Returns T-states used.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        self.ei_pending = false;
        if self.halted { self.inc_r(); self.cycles += 4; return 4; }
        let start = self.cycles;
        let mut ix = Idx::HL;
        let mut op;
        loop {
            op = self.fetch(bus); self.inc_r();
            match op {
                0xDD => { ix = Idx::IX; self.cycles += 4; }
                0xFD => { ix = Idx::IY; self.cycles += 4; }
                _ => break,
            }
        }
        match op {
            0xCB => self.exec_cb(bus, ix),
            0xED => { self.exec_ed(bus); }
            _ => self.exec_main(bus, op, ix),
        }
        (self.cycles - start) as u32
    }

    fn idx_addr<B: Bus>(&mut self, bus: &mut B, ix: Idx) -> u16 {
        match ix {
            Idx::HL => self.hl(),
            _ => { let d = self.fetch(bus) as i8; let a = self.get_xy(ix).wrapping_add(d as i16 as u16); self.wz = a; a }
        }
    }

    fn exec_main<B: Bus>(&mut self, bus: &mut B, op: u8, ix: Idx) {
        let x = op >> 6; let y = (op >> 3) & 7; let z = op & 7; let p = y >> 1; let q = y & 1;
        let idx = ix != Idx::HL;
        match x {
            1 => {
                if op == 0x76 { self.halted = true; self.pc = self.pc.wrapping_sub(1); self.cycles += 4; return; }
                if z == 6 { // LD r,(HL)
                    let a = self.idx_addr(bus, ix); let v = bus.read(a);
                    self.set_r(y, Idx::HL, v); self.cycles += if idx { 15 } else { 7 };
                } else if y == 6 { // LD (HL),r
                    let a = self.idx_addr(bus, ix); let v = self.get_r(z, Idx::HL);
                    bus.write(a, v); self.cycles += if idx { 15 } else { 7 };
                } else {
                    let v = self.get_r(z, ix); self.set_r(y, ix, v); self.cycles += 4;
                }
                return;
            }
            2 => {
                let v = if z == 6 { let a = self.idx_addr(bus, ix); self.cycles += if idx { 15 } else { 7 }; bus.read(a) }
                        else { self.cycles += 4; self.get_r(z, ix) };
                self.alu(y, v); return;
            }
            _ => {}
        }
        if x == 0 {
            match z {
                0 => match y {
                    0 => { self.cycles += 4; }
                    1 => { std::mem::swap(&mut self.a, &mut self.a_); std::mem::swap(&mut self.f, &mut self.f_); self.cycles += 4; }
                    2 => { let d = self.fetch(bus) as i8; self.b = self.b.wrapping_sub(1);
                           if self.b != 0 { self.pc = self.pc.wrapping_add(d as i16 as u16); self.wz = self.pc; self.cycles += 13; } else { self.cycles += 8; } }
                    3 => { let d = self.fetch(bus) as i8; self.pc = self.pc.wrapping_add(d as i16 as u16); self.wz = self.pc; self.cycles += 12; }
                    _ => { let d = self.fetch(bus) as i8; if self.cond(y - 4) { self.pc = self.pc.wrapping_add(d as i16 as u16); self.wz = self.pc; self.cycles += 12; } else { self.cycles += 7; } }
                },
                1 => if q == 0 { let v = self.fetch16(bus); self.set_rp(p, ix, v); self.cycles += 10; }
                     else { let a = self.get_xy(ix); let b = self.get_rp(p, ix); let r = self.add16(a, b); self.set_xy(ix, r); self.cycles += 11; },
                2 => {
                    match (q, p) {
                        (0, 0) => { let a = self.bc(); bus.write(a, self.a); self.wz = ((self.a as u16) << 8) | (a.wrapping_add(1) & 0xFF); self.cycles += 7; }
                        (0, 1) => { let a = self.de(); bus.write(a, self.a); self.wz = ((self.a as u16) << 8) | (a.wrapping_add(1) & 0xFF); self.cycles += 7; }
                        (0, 2) => { let a = self.fetch16(bus); let v = self.get_xy(ix); self.wr16(bus, a, v); self.wz = a.wrapping_add(1); self.cycles += 16; }
                        (0, _) => { let a = self.fetch16(bus); bus.write(a, self.a); self.wz = ((self.a as u16) << 8) | (a.wrapping_add(1) & 0xFF); self.cycles += 13; }
                        (_, 0) => { let a = self.bc(); self.a = bus.read(a); self.wz = a.wrapping_add(1); self.cycles += 7; }
                        (_, 1) => { let a = self.de(); self.a = bus.read(a); self.wz = a.wrapping_add(1); self.cycles += 7; }
                        (_, 2) => { let a = self.fetch16(bus); let v = self.rd16(bus, a); self.set_xy(ix, v); self.wz = a.wrapping_add(1); self.cycles += 16; }
                        (_, _) => { let a = self.fetch16(bus); self.a = bus.read(a); self.wz = a.wrapping_add(1); self.cycles += 13; }
                    }
                }
                3 => { let v = self.get_rp(p, ix); let v = if q == 0 { v.wrapping_add(1) } else { v.wrapping_sub(1) }; self.set_rp(p, ix, v); self.cycles += 6; }
                4 | 5 => {
                    if y == 6 {
                        let a = self.idx_addr(bus, ix); let v = bus.read(a);
                        let r = if z == 4 { self.inc8(v) } else { self.dec8(v) }; bus.write(a, r);
                        self.cycles += if idx { 19 } else { 11 };
                    } else {
                        let v = self.get_r(y, ix); let r = if z == 4 { self.inc8(v) } else { self.dec8(v) }; self.set_r(y, ix, r); self.cycles += 4;
                    }
                }
                6 => {
                    if y == 6 {
                        let a = self.idx_addr(bus, ix); let n = self.fetch(bus); bus.write(a, n);
                        self.cycles += if idx { 15 } else { 10 };
                    } else { let n = self.fetch(bus); self.set_r(y, ix, n); self.cycles += 7; }
                }
                _ => {
                    self.cycles += 4;
                    let a = self.a; let c = self.f & FC;
                    match y {
                        0 => { self.a = a.rotate_left(1); self.f = (self.f & (FS | FZ | FP)) | (self.a & (FY | FX)) | (a >> 7); }
                        1 => { self.a = a.rotate_right(1); self.f = (self.f & (FS | FZ | FP)) | (self.a & (FY | FX)) | (a & 1); }
                        2 => { self.a = (a << 1) | c; self.f = (self.f & (FS | FZ | FP)) | (self.a & (FY | FX)) | (a >> 7); }
                        3 => { self.a = (a >> 1) | (c << 7); self.f = (self.f & (FS | FZ | FP)) | (self.a & (FY | FX)) | (a & 1); }
                        4 => { // DAA
                            let mut corr = 0u8; let mut carry = c;
                            if (self.f & FH) != 0 || (a & 0x0F) > 9 { corr |= 0x06; }
                            if c != 0 || a > 0x99 { corr |= 0x60; carry = 1; }
                            let r = if self.f & FN != 0 { a.wrapping_sub(corr) } else { a.wrapping_add(corr) };
                            let mut f = szp(r) | (self.f & FN) | carry;
                            if (a ^ r) & 0x10 != 0 { f |= FH; }
                            self.a = r; self.f = f;
                        }
                        5 => { self.a = !a; self.f = (self.f & (FS | FZ | FP | FC)) | FH | FN | (self.a & (FY | FX)); }
                        6 => { self.f = (self.f & (FS | FZ | FP)) | FC | (a & (FY | FX)); }
                        _ => { self.f = ((self.f & (FS | FZ | FP | FC)) | (if c != 0 { FH } else { 0 }) | (a & (FY | FX))) ^ FC; }
                    }
                }
            }
            return;
        }
        // x == 3
        match z {
            0 => { if self.cond(y) { self.pc = self.pop(bus); self.wz = self.pc; self.cycles += 11; } else { self.cycles += 5; } }
            1 => {
                if q == 0 { let v = self.pop(bus); match p { 0 => self.set_bc(v), 1 => self.set_de(v), 2 => self.set_xy(ix, v), _ => self.set_af(v) } self.cycles += 10; }
                else { match p {
                    0 => { self.pc = self.pop(bus); self.wz = self.pc; self.cycles += 10; }
                    1 => { // EXX
                        std::mem::swap(&mut self.b, &mut self.b_); std::mem::swap(&mut self.c, &mut self.c_);
                        std::mem::swap(&mut self.d, &mut self.d_); std::mem::swap(&mut self.e, &mut self.e_);
                        std::mem::swap(&mut self.h, &mut self.h_); std::mem::swap(&mut self.l, &mut self.l_);
                        self.cycles += 4; }
                    2 => { self.pc = self.get_xy(ix); self.cycles += 4; }
                    _ => { self.sp = self.get_xy(ix); self.cycles += 6; }
                } }
            }
            2 => { let a = self.fetch16(bus); self.wz = a; if self.cond(y) { self.pc = a; } self.cycles += 10; }
            3 => match y {
                0 => { let a = self.fetch16(bus); self.pc = a; self.wz = a; self.cycles += 10; }
                1 => unreachable!(),
                2 => { let n = self.fetch(bus); let port = (self.a as u16) << 8 | n as u16; bus.output(port, self.a);
                       self.wz = ((self.a as u16) << 8) | (n.wrapping_add(1) as u16); self.cycles += 11; }
                3 => { let n = self.fetch(bus); let port = (self.a as u16) << 8 | n as u16; self.wz = port.wrapping_add(1); self.a = bus.input(port); self.cycles += 11; }
                4 => { let sp = self.sp; let v = self.rd16(bus, sp); let r = self.get_xy(ix); self.wr16(bus, sp, r); self.set_xy(ix, v); self.wz = v; self.cycles += 19; }
                5 => { let d = self.de(); let h = self.hl(); self.set_de(h); self.set_hl(d); self.cycles += 4; }
                6 => { self.iff1 = false; self.iff2 = false; self.cycles += 4; }
                _ => { self.iff1 = true; self.iff2 = true; self.ei_pending = true; self.cycles += 4; }
            },
            4 => { let a = self.fetch16(bus); self.wz = a; if self.cond(y) { let pc = self.pc; self.push(bus, pc); self.pc = a; self.cycles += 17; } else { self.cycles += 10; } }
            5 => {
                if q == 0 { let v = match p { 0 => self.bc(), 1 => self.de(), 2 => self.get_xy(ix), _ => self.af() }; self.push(bus, v); self.cycles += 11; }
                else { // p==0 CALL nn (others are prefixes, handled earlier)
                    let a = self.fetch16(bus); self.wz = a; let pc = self.pc; self.push(bus, pc); self.pc = a; self.cycles += 17; }
            }
            6 => { let n = self.fetch(bus); self.alu(y, n); self.cycles += 7; }
            _ => { let pc = self.pc; self.push(bus, pc); self.pc = (y as u16) * 8; self.wz = self.pc; self.cycles += 11; }
        }
    }

    fn exec_cb<B: Bus>(&mut self, bus: &mut B, ix: Idx) {
        if ix != Idx::HL {
            // DD CB d op : displacement before opcode; opcode fetch is not an M1 (no R increment)
            let d = self.fetch(bus) as i8; let op = self.fetch(bus);
            let a = self.get_xy(ix).wrapping_add(d as i16 as u16); self.wz = a;
            let x = op >> 6; let y = (op >> 3) & 7; let z = op & 7;
            let v = bus.read(a);
            if x == 1 {
                let bit = v & (1 << y);
                let mut f = (self.f & FC) | FH | (((a >> 8) as u8) & (FY | FX));
                if bit == 0 { f |= FZ | FP; }
                if y == 7 && bit != 0 { f |= FS; }
                self.f = f; self.cycles += 16; return;
            }
            let r = match x { 0 => self.rot(y, v), 2 => v & !(1 << y), _ => v | (1 << y) };
            bus.write(a, r);
            if z != 6 { self.set_r(z, Idx::HL, r); }
            self.cycles += 19; return;
        }
        let op = self.fetch(bus); self.inc_r();
        let x = op >> 6; let y = (op >> 3) & 7; let z = op & 7;
        let (v, addr) = if z == 6 { let a = self.hl(); (bus.read(a), a) } else { (self.get_r(z, Idx::HL), 0) };
        if x == 1 {
            let bit = v & (1 << y);
            let mut f = (self.f & FC) | FH;
            if z == 6 { f |= ((self.wz >> 8) as u8) & (FY | FX); } else { f |= v & (FY | FX); }
            if bit == 0 { f |= FZ | FP; }
            if y == 7 && bit != 0 { f |= FS; }
            self.f = f; self.cycles += if z == 6 { 12 } else { 8 }; return;
        }
        let r = match x { 0 => self.rot(y, v), 2 => v & !(1 << y), _ => v | (1 << y) };
        if z == 6 { bus.write(addr, r); self.cycles += 15; } else { self.set_r(z, Idx::HL, r); self.cycles += 8; }
    }

    fn exec_ed<B: Bus>(&mut self, bus: &mut B) {
        let op = self.fetch(bus); self.inc_r();
        let x = op >> 6; let y = (op >> 3) & 7; let z = op & 7; let p = y >> 1; let q = y & 1;
        if x == 1 {
            match z {
                0 => { let port = self.bc(); let v = bus.input(port); self.wz = port.wrapping_add(1);
                       if y != 6 { self.set_r(y, Idx::HL, v); } self.f = (self.f & FC) | szp(v); self.cycles += 12; }
                1 => { let port = self.bc(); let v = if y == 6 { 0 } else { self.get_r(y, Idx::HL) }; bus.output(port, v); self.wz = port.wrapping_add(1); self.cycles += 12; }
                2 => { let h = self.hl(); let r = self.get_rp(p, Idx::HL); let v = if q == 0 { self.sbc16(h, r) } else { self.adc16(h, r) }; self.set_hl(v); self.cycles += 15; }
                3 => { let a = self.fetch16(bus); if q == 0 { let v = self.get_rp(p, Idx::HL); self.wr16(bus, a, v); } else { let v = self.rd16(bus, a); self.set_rp(p, Idx::HL, v); } self.wz = a.wrapping_add(1); self.cycles += 20; }
                4 => { let a = self.a; self.a = 0; self.alu(2, a); self.cycles += 8; }
                5 => { self.pc = self.pop(bus); self.wz = self.pc; self.iff1 = self.iff2; self.cycles += 14; }
                6 => { self.im = match y & 3 { 0 | 1 => 0, 2 => 1, _ => 2 }; self.cycles += 8; }
                _ => match y {
                    0 => { self.i = self.a; self.cycles += 9; }
                    1 => { self.r = self.a; self.cycles += 9; }
                    2 => { self.a = self.i; self.f = (self.f & FC) | (szp(self.a) & !FP) | if self.iff2 { FP } else { 0 }; self.cycles += 9; }
                    3 => { self.a = self.r; self.f = (self.f & FC) | (szp(self.a) & !FP) | if self.iff2 { FP } else { 0 }; self.cycles += 9; }
                    4 => { // RRD
                        let a = self.hl(); let m = bus.read(a);
                        bus.write(a, (self.a << 4) | (m >> 4)); self.a = (self.a & 0xF0) | (m & 0x0F);
                        self.f = (self.f & FC) | szp(self.a); self.wz = a.wrapping_add(1); self.cycles += 18; }
                    5 => { // RLD
                        let a = self.hl(); let m = bus.read(a);
                        bus.write(a, (m << 4) | (self.a & 0x0F)); self.a = (self.a & 0xF0) | (m >> 4);
                        self.f = (self.f & FC) | szp(self.a); self.wz = a.wrapping_add(1); self.cycles += 18; }
                    _ => { self.cycles += 8; }
                },
            }
            return;
        }
        if x == 2 && z <= 3 && y >= 4 {
            let inc = y & 1 == 0; let rep = y >= 6;
            match z {
                0 => { // LDI/LDD
                    let v = bus.read(self.hl()); bus.write(self.de(), v);
                    let d: u16 = if inc { 1 } else { 0xFFFF };
                    self.set_hl(self.hl().wrapping_add(d)); self.set_de(self.de().wrapping_add(d));
                    self.set_bc(self.bc().wrapping_sub(1));
                    let n = v.wrapping_add(self.a);
                    let mut f = self.f & (FS | FZ | FC);
                    if n & 0x08 != 0 { f |= FX; } if n & 0x02 != 0 { f |= FY; }
                    if self.bc() != 0 { f |= FP; }
                    self.f = f;
                    if rep && self.bc() != 0 { self.pc = self.pc.wrapping_sub(2); self.wz = self.pc.wrapping_add(1); self.cycles += 21; } else { self.cycles += 16; }
                }
                1 => { // CPI/CPD
                    let v = bus.read(self.hl()); let r = self.a.wrapping_sub(v);
                    let d: u16 = if inc { 1 } else { 0xFFFF };
                    self.set_hl(self.hl().wrapping_add(d)); self.set_bc(self.bc().wrapping_sub(1));
                    self.wz = if inc { self.wz.wrapping_add(1) } else { self.wz.wrapping_sub(1) };
                    let mut f = (self.f & FC) | FN | (r & FS);
                    if r == 0 { f |= FZ; }
                    let h = (self.a ^ v ^ r) & 0x10 != 0; if h { f |= FH; }
                    let n = r.wrapping_sub(if h { 1 } else { 0 });
                    if n & 0x08 != 0 { f |= FX; } if n & 0x02 != 0 { f |= FY; }
                    if self.bc() != 0 { f |= FP; }
                    self.f = f;
                    if rep && self.bc() != 0 && r != 0 { self.pc = self.pc.wrapping_sub(2); self.wz = self.pc.wrapping_add(1); self.cycles += 21; } else { self.cycles += 16; }
                }
                2 => { // INI/IND
                    let port = self.bc(); let v = bus.input(port);
                    self.wz = if inc { port.wrapping_add(1) } else { port.wrapping_sub(1) };
                    bus.write(self.hl(), v);
                    let d: u16 = if inc { 1 } else { 0xFFFF }; self.set_hl(self.hl().wrapping_add(d));
                    self.b = self.b.wrapping_sub(1);
                    let k = v as u16 + (if inc { self.c.wrapping_add(1) } else { self.c.wrapping_sub(1) }) as u16;
                    let mut f = szp(self.b) & !FP;
                    if v & 0x80 != 0 { f |= FN; } if k > 0xFF { f |= FH | FC; }
                    if ((k as u8 & 7) ^ self.b).count_ones() % 2 == 0 { f |= FP; }
                    self.f = f;
                    if rep && self.b != 0 { self.pc = self.pc.wrapping_sub(2); self.cycles += 21; } else { self.cycles += 16; }
                }
                _ => { // OUTI/OUTD
                    let v = bus.read(self.hl()); self.b = self.b.wrapping_sub(1);
                    let port = self.bc(); bus.output(port, v);
                    self.wz = if inc { port.wrapping_add(1) } else { port.wrapping_sub(1) };
                    let d: u16 = if inc { 1 } else { 0xFFFF }; self.set_hl(self.hl().wrapping_add(d));
                    let k = v as u16 + self.l as u16;
                    let mut f = szp(self.b) & !FP;
                    if v & 0x80 != 0 { f |= FN; } if k > 0xFF { f |= FH | FC; }
                    if ((k as u8 & 7) ^ self.b).count_ones() % 2 == 0 { f |= FP; }
                    self.f = f;
                    if rep && self.b != 0 { self.pc = self.pc.wrapping_sub(2); self.cycles += 21; } else { self.cycles += 16; }
                }
            }
            return;
        }
        self.cycles += 8; // invalid ED = NONI
    }
}
