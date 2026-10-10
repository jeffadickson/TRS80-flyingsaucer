//! Minimal TRS-80 Model I machine: 48K RAM, memory-mapped keyboard and video,
//! cassette-port sound output, a substitute ROM (the Tandy ROM is not
//! distributed) and high-level emulation (HLE) of the few TRSDOS file
//! calls the programs use: Galaxy Invasion Plus's HISCORES/GAL file and the
//! sector of FROGGER/CMD that Frogger rewrites with its high scores.

use crate::z80::{Bus, Z80};

pub const CPU_HZ: f64 = 1_774_080.0;        // Model I clock (10.6445 MHz / 6)
pub const FRAME_T: u64 = 29_568;            // 1774080 / 60 Hz: one US video field

pub struct Machine {
    pub mem: Vec<u8>,
    pub kbd: [u8; 8],          // keyboard matrix rows 0..7 (bit set = key down)
    pub lowercase_mod: bool,   // false = stock Model I (bit 6 of video RAM missing)
    pub joystick: u8,          // value read from port 00 (FF = none fitted)
    pub port_ff: u8,
    pub sound: Vec<(u64, u8)>, // (T-state, cassette bits 0-1) every change
    pub port_writes: Vec<(u64, u8, u8)>,
    pub cycles: u64,           // mirrored from CPU for timestamping
    pub wide: bool,            // 32-character mode (port FF bit 3)
    pub hiscore_file: Option<Vec<u8>>, // HLE DOS: contents of HISCORES/GAL
    pub write_log: Option<Vec<(u64, u16, u16, u8)>>, // (cycles, pc, addr, value) for SMC study
    pub pc: u16,
    pub rom_reads: Vec<u16>,
    pub watch: Vec<u16>,
    pub watch_log: Vec<(u64, u16, u16, u8)>,
}

impl Machine {
    pub fn new() -> Self {
        let mut mem = vec![0u8; 65536];
        // ---- substitute ROM (0000-2FFF). NOT Tandy code. ----
        for b in mem[0..0x3000].iter_mut() { *b = 0xFF; }
        mem[0x0000] = 0xF3; mem[0x0001] = 0x76;          // DI ; HALT  (reset lands here)
        mem[0x002B] = 0xC9;                              // $KBD   : trapped (HLE), RET
        mem[0x0033] = 0xC9;                              // $DSP   : trapped (HLE), RET
        mem[0x0049] = 0xC9;                              // $KEY   : trapped (HLE), waits for a key
        mem[0x01C9] = 0xC9;                              // $CLS   : trapped (HLE)
        mem[0x0630] = 0xC9;                              // line-input backspace handler: trapped (HLE)
        mem[0x402D] = 0x76;                              // DOS "return to DOS": HALT here
        // 0060h delay: DEC BC / LD A,B / OR C / JR NZ,0060h / RET - the documented
        // Level II routine, 26 T-states per pass (14.66 us at 1.774 MHz).
        mem[0x0060..0x0066].copy_from_slice(&[0x0B, 0x78, 0xB1, 0x20, 0xFB, 0xC9]);
        mem[0x0125] = 0x00;                              // not 'I' (49h): read as Model I
        // DOS vectors used by the loader are trapped; put RET there.
        for a in [0x4409u16, 0x4420, 0x4424, 0x4428, 0x4436, 0x4439, 0x443C] { mem[a as usize] = 0xC9; }
        // cursor position for $DSP
        mem[0x4020] = 0x00; mem[0x4021] = 0x3C;
        // video RAM starts blank (the JS machine, saucsys.js, does the same)
        for b in mem[0x3C00..0x4000].iter_mut() { *b = 0x20; }
        Machine { mem, kbd: [0; 8], lowercase_mod: false, joystick: 0xFF, port_ff: 0,
                  sound: Vec::new(), port_writes: Vec::new(), cycles: 0, wide: false,
                  hiscore_file: None, write_log: None, pc: 0, rom_reads: Vec::new(), watch: Vec::new(), watch_log: Vec::new() }
    }

    pub fn load_cmd(&mut self, data: &[u8]) -> u16 {
        let mut i = 0; let mut entry = 0;
        while i + 1 < data.len() {
            let t = data[i]; let l = data[i + 1] as usize;
            let n = if l > 2 { l } else { l + 256 };
            match t {
                1 => { let a = data[i + 2] as usize | (data[i + 3] as usize) << 8;
                       for j in 0..n - 2 { self.mem[(a + j) & 0xFFFF] = data[i + 4 + j]; } }
                2 => { entry = data[i + 2] as u16 | (data[i + 3] as u16) << 8; break; }
                _ => {}
            }
            i += 2 + n;
        }
        entry
    }

    /// Video RAM as the CRT would show it (64x16 codes).
    pub fn screen(&self) -> Vec<u8> { self.mem[0x3C00..0x4000].to_vec() }

    fn video_store(&self, v: u8) -> u8 {
        if self.lowercase_mod { return v; }
        // Stock Model I: seven 2102s, bit 6 not stored. The display (and a
        // read-back) sees bit 6 = NOT(bit 5 OR bit 7). See README.
        let s = v & 0xBF;
        if s & 0xA0 == 0 { s | 0x40 } else { s }
    }

    pub fn key_down(&mut self, name: &str, down: bool) {
        if let Some((row, bit)) = key_pos(name) {
            if down { self.kbd[row] |= 1 << bit; } else { self.kbd[row] &= !(1 << bit); }
        }
    }
}

/// Model I keyboard matrix: row n is at 3800h + (1<<n).
pub fn key_pos(name: &str) -> Option<(usize, u8)> {
    let n = name.to_ascii_uppercase();
    let rows: [[&str; 8]; 8] = [
        ["@", "A", "B", "C", "D", "E", "F", "G"],
        ["H", "I", "J", "K", "L", "M", "N", "O"],
        ["P", "Q", "R", "S", "T", "U", "V", "W"],
        ["X", "Y", "Z", "", "", "", "", ""],
        ["0", "1", "2", "3", "4", "5", "6", "7"],
        ["8", "9", ":", ";", ",", "-", ".", "/"],
        ["ENTER", "CLEAR", "BREAK", "UP", "DOWN", "LEFT", "RIGHT", "SPACE"],
        ["SHIFT", "", "", "", "", "", "", ""],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (b, k) in row.iter().enumerate() { if !k.is_empty() && *k == n { return Some((r, b as u8)); } }
    }
    None
}

impl Bus for Machine {
    #[inline]
    fn read(&mut self, a: u16) -> u8 {
        match a {
            0x3800..=0x3BFF => {
                let sel = (a & 0xFF) as u8; let mut v = 0;
                for r in 0..8 { if sel & (1 << r) != 0 { v |= self.kbd[r]; } }
                v
            }
            // FDC 1771 status (Expansion Interface). Only what the loader's
            // "is a diskette turning in drive 0?" test needs: bit 1 = index
            // pulse, ~4 ms once per 200 ms revolution (300 rpm). Not busy,
            // not write protected, ready.
            0x37EC => { let rev = (CPU_HZ * 0.2) as u64; if self.cycles % rev < 7000 { 0x02 } else { 0x00 } }
            0x37E0..=0x37FF => 0xFF,
            0x3000..=0x37DF => 0xFF,
            0x3C00..=0x3FFF => self.mem[a as usize],
            _ => self.mem[a as usize],
        }
    }
    #[inline]
    fn write(&mut self, a: u16, v: u8) {
        if !self.watch.is_empty() && self.watch.contains(&a) { self.watch_log.push((self.cycles, self.pc, a, v)); }
        if let Some(log) = self.write_log.as_mut() { if a >= 0x7000 { log.push((self.cycles, self.pc, a, v)); } }
        match a {
            0x0000..=0x37FF => {}         // ROM / I/O: ignored
            0x3800..=0x3BFF => {}
            0x3C00..=0x3FFF => { let s = self.video_store(v); self.mem[a as usize] = s; }
            _ => self.mem[a as usize] = v,
        }
    }
    fn input(&mut self, port: u16) -> u8 {
        match port & 0xFF { 0x00 => self.joystick, 0xFF => 0x7F, _ => 0xFF }
    }
    fn output(&mut self, port: u16, v: u8) {
        let p = (port & 0xFF) as u8;
        self.port_writes.push((self.cycles, p, v));
        if p == 0xFF {
            if (v & 3) != (self.port_ff & 3) || self.sound.is_empty() { self.sound.push((self.cycles, v & 3)); }
            self.wide = v & 0x08 != 0;
            self.port_ff = v;
        }
    }
}

/// Run state: CPU + machine + HLE traps.
pub struct System {
    pub cpu: Z80,
    pub m: Machine,
    pub cursor_on: bool,
    pub dos_log: Vec<String>,
    pub hle_count: [u64; 8],
    /// Trap the TRSDOS entry points (4409h-443Ch). Off by default: for a program
    /// that does not run under DOS these are ordinary RAM, and in Flying Saucers
    /// they hold game code (4424h and 4428h are on the demonstration-mode path).
    pub dos_hle: bool,
}

impl System {
    pub fn new() -> Self { System { cpu: Z80::new(), m: Machine::new(), cursor_on: false, dos_log: vec![], hle_count: [0; 8], dos_hle: false } }

    fn ret(&mut self) { let lo = self.m.mem[self.cpu.sp as usize] as u16; let hi = self.m.mem[self.cpu.sp.wrapping_add(1) as usize] as u16;
        self.cpu.sp = self.cpu.sp.wrapping_add(2); self.cpu.pc = hi << 8 | lo; self.cpu.cycles += 10; }

    /// High-level emulation of ROM and DOS entry points. Returns true if handled.
    fn hle(&mut self) -> bool {
        let pc = self.cpu.pc;
        match pc {
            0x002B => { // $KBD: scan keyboard, return ASCII in A (0 = none). Substitute.
                self.hle_count[0] += 1;
                self.cpu.a = self.scan_ascii(); self.cpu.cycles += 400; self.ret(); true
            }
            0x0033 => { // $DSP: display character in A at cursor (4020h). Substitute.
                self.hle_count[1] += 1;
                let c = self.cpu.a; self.dsp(c); self.cpu.cycles += 300; self.ret(); true
            }
            0x0049 => { // $KEY: wait for a key, return it in A. While no key is down the
                // CPU stays here, burning time, so the emulated clock keeps running.
                self.hle_count[2] += 1;
                let c = self.scan_ascii();
                if c == 0 { self.cpu.cycles += 200; return true; }
                self.cpu.a = c; self.cpu.cycles += 400; self.ret(); true
            }
            0x01C9 => { // $CLS: blank the screen and home the cursor. Substitute.
                self.hle_count[3] += 1;
                for a in 0x3C00..0x4000u16 { self.m.write(a, 0x20); }
                self.m.mem[0x4020] = 0x00; self.m.mem[0x4021] = 0x3C;
                self.cpu.cycles += 1024 * 21; self.ret(); true
            }
            0x0630 => { // Level II line input, backspace (BS) handler, reached by JP with
                // the loop's address pushed. Substitute: if a character has been typed
                // (B < C), step HL back, give the count back and erase on screen.
                self.hle_count[4] += 1;
                let b = self.cpu.b; let c = self.cpu.c;
                if b < c { let hl = self.cpu.hl().wrapping_sub(1); self.cpu.h = (hl >> 8) as u8; self.cpu.l = hl as u8;
                    self.cpu.b = b + 1; self.dsp(0x08); }
                self.cpu.cycles += 300; self.ret(); true
            }
            0x4420 | 0x4424 | 0x4428 | 0x4436 | 0x4439 | 0x443C | 0x4409 if self.dos_hle => { self.dos(pc); self.ret(); true }
            _ => false,
        }
    }

    fn scan_ascii(&mut self) -> u8 {
        // Returns the first pressed key; real ROM debounces and only reports new keys.
        // Level II codes: letters are upper case unshifted, SHIFT adds 20h;
        // on rows 4-5 SHIFT flips bit 4 ('1'->'!', ','->'<', '.'->'>'), except '0'.
        let k = &self.m.kbd; let shift = k[7] & 1 != 0;
        for r in 0..7 { for b in 0..8 { if k[r] & (1 << b) != 0 {
            let c = match r {
                0..=3 => { let c = b'@' + (r as u8) * 8 + b as u8; if shift && c != b'@' { c + 32 } else { c } }
                4 | 5 => { let c = if r == 4 { b'0' + b as u8 } else { [b'8', b'9', b':', b';', b',', b'-', b'.', b'/'][b] };
                           if shift && c != b'0' { c ^ 0x10 } else { c } }
                _ => [0x0D, 0x1F, 0x01, 0x5B, 0x0A, 0x08, 0x09, 0x20][b],
            };
            // emulate "new key" semantics: consume
            self.m.kbd[r] &= !(1 << b);
            return c;
        } } }
        0
    }

    fn dsp(&mut self, c: u8) {
        let mut cur = (self.m.mem[0x4020] as u16 | (self.m.mem[0x4021] as u16) << 8).clamp(0x3C00, 0x3FFF);
        match c {
            0x0D | 0x0A => { cur = (cur & !0x3F) + 0x40; }
            0x08 => { if cur > 0x3C00 { cur -= 1; self.m.write(cur, 0x20); } }
            0x0E => self.cursor_on = true,
            0x0F => self.cursor_on = false,
            0x1C => cur = 0x3C00,
            0x1D => cur &= !0x3F,
            0x1E => { let mut a = cur; while a & 0x3F != 0 || a == cur { self.m.write(a, 0x20); a += 1; if a & 0x3F == 0 { break; } } }
            0x1F => { let mut a = cur; while a < 0x4000 { self.m.write(a, 0x20); a += 1; } }
            0x17 => {}
            0x20..=0xFF => {
                // Level II folds lower case to upper case unless the lowercase driver is installed.
                let v = if !self.m.lowercase_mod && (0x60..0x80).contains(&c) { c - 0x20 } else { c };
                self.m.write(cur, v); cur += 1;
            }
            _ => {}
        }
        if cur >= 0x4000 { // scroll
            self.m.mem.copy_within(0x3C40..0x4000, 0x3C00);
            for a in 0x3FC0..0x4000 { self.m.mem[a] = 0x20; }
            cur -= 0x40;
        }
        self.m.mem[0x4020] = cur as u8; self.m.mem[0x4021] = (cur >> 8) as u8;
    }

    fn dos(&mut self, pc: u16) {
        // TRSDOS file SVCs, HLE. Z set = success. The loader only ever uses one
        // file (HISCORES/GAL), one 256-byte record at a buffer in HL (INIT/OPEN).
        let de = self.cpu.de();
        let name: String = (0..13).map(|i| self.m.mem[(de + i) as usize]).take_while(|&b| b >= 0x20).map(|b| b as char).collect();
        let ok = |cpu: &mut Z80| { cpu.f = (cpu.f | crate::z80::FZ) & !crate::z80::FC; cpu.a = 0; };
        match pc {
            0x4420 => { // @INIT: open existing or create. C set if file was created (TRSDOS convention)
                let existed = self.m.hiscore_file.is_some();
                if !existed { self.m.hiscore_file = Some(vec![0; 256]); }
                // remember buffer address in FCB bytes 3-4 like TRSDOS
                let hl = self.cpu.hl(); self.m.mem[(de + 3) as usize] = hl as u8; self.m.mem[(de + 4) as usize] = (hl >> 8) as u8;
                ok(&mut self.cpu); if !existed { self.cpu.f |= crate::z80::FC; }
                self.dos_log.push(format!("{:>12} @INIT  {} -> {}", self.cpu.cycles, name, if existed { "existing" } else { "created" }));
            }
            0x4424 => { let hl = self.cpu.hl(); self.m.mem[(de + 3) as usize] = hl as u8; self.m.mem[(de + 4) as usize] = (hl >> 8) as u8;
                if self.m.hiscore_file.is_some() { ok(&mut self.cpu); } else { self.cpu.a = 0x18; self.cpu.f &= !crate::z80::FZ; }
                self.dos_log.push(format!("{:>12} @OPEN  {}", self.cpu.cycles, name)); }
            0x4428 => { ok(&mut self.cpu); self.dos_log.push(format!("{:>12} @CLOSE {}", self.cpu.cycles, name)); }
            0x4436 => { let buf = self.m.mem[(de + 3) as usize] as usize | (self.m.mem[(de + 4) as usize] as usize) << 8;
                if let Some(f) = &self.m.hiscore_file { for i in 0..256 { self.m.mem[buf + i] = f[i]; } }
                ok(&mut self.cpu); self.dos_log.push(format!("{:>12} @READ  {} -> {:04X}", self.cpu.cycles, name, buf)); }
            0x4439 | 0x443C => { let buf = self.m.mem[(de + 3) as usize] as usize | (self.m.mem[(de + 4) as usize] as usize) << 8;
                // Frogger keeps its own program file open and rewrites its first
                // 256-byte record (the high-score table) in place.
                let rec = self.m.mem[buf..buf + 256].to_vec();
                match self.m.hiscore_file.as_mut() {
                    Some(f) if f.len() >= 256 => f[..256].copy_from_slice(&rec),
                    _ => self.m.hiscore_file = Some(rec),
                }
                ok(&mut self.cpu); self.dos_log.push(format!("{:>12} @WRITE {} <- {:04X}", self.cpu.cycles, name, buf)); }
            _ => { self.dos_log.push(format!("{:>12} @ERROR code {:02X}", self.cpu.cycles, self.cpu.a)); }
        }
        self.cpu.cycles += 2000;
    }

    /// Execute one instruction (or one HLE call). Returns T-states.
    pub fn step(&mut self) -> u64 {
        let c0 = self.cpu.cycles;
        self.m.cycles = c0; self.m.pc = self.cpu.pc;
        if !self.hle() {
            if self.cpu.pc < 0x3000 && !(0x0060..=0x0065).contains(&self.cpu.pc) && self.cpu.pc != 0 && self.cpu.pc != 1 {
                self.m.rom_reads.push(self.cpu.pc);
            }
            self.cpu.step(&mut self.m);
        }
        self.cpu.cycles - c0
    }

    pub fn run_until(&mut self, t: u64) { while self.cpu.cycles < t { self.step(); } }
}
