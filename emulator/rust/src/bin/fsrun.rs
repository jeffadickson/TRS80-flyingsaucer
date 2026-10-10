//! Headless runner for the Flying Saucers study (Radio Shack 26-1905, 1979).
//!
//! fsrun <file.cmd> [options]
//!   --frames N         run N fields of 1/60 s (29,568 T-states each)
//!   --script FILE      lines "<frame> <down|up|tap> KEY [frames]"  (tap holds 3 frames)
//!   --dump FILE        video RAM after every field (1,024 bytes each)
//!   --every K          dump only every K-th field
//!   --trace FILE       per field: field cycles pc af bc de hl ix iy sp r screenhash ramhash
//!   --profile FILE     per-routine T-states (shadow call stack): exclusive and inclusive
//!   --hits A,B,..:FILE every time PC reaches one of these addresses: field cycles pc regs
//!   --codewrites FILE  every write into 42EAh-4C47h (the code): cycles pc addr value
//!   --memdump F:PATH   full 64K memory after field F (repeatable)
//!   --robot F[:SEED]   from field F a simple player aims at saucers it sees in video
//!                      RAM, steers the missile in flight and fires; for captures only
//!   --poke A=V,..      patch memory after loading (hex)
//!   --pokeat F:A=V,..  patch memory at the start of field F (hex)
//!   --robotlog FILE    write the robot's presses as a script (so a browser can replay them)
//!   --moment F:LABELPC after field F, run on to PC; print pc, screen hash, RAM hash and stop
//!   --passlog FILE     at the top of every game-loop pass: cycles, screen hash, data hash (4C4Bh-4FE6h)
//!   --keylog FILE      every key the program read through $KBD: field cycles code
use std::io::Write;
use z80emu::trs80::{System, FRAME_T};

const CODE_LO: u16 = 0x42EA;
const CODE_HI: u16 = 0x4C48;
const MAIN_TOP: u16 = 0x4404; // top of the game loop

fn fnv(b: &[u8]) -> u64 { let mut h: u64 = 0xcbf29ce484222325; for x in b { h ^= *x as u64; h = h.wrapping_mul(0x100000001b3); } h }

struct Frame { target: u16, sp: u16, ret: u16, t0: u64, child: u64, regs: [u8; 12] }

/// A F B C D E H L IXh IXl IYh IYl
fn snap(c: &z80emu::z80::Z80) -> [u8; 12] { [c.a, c.f, c.b, c.c, c.d, c.e, c.h, c.l, (c.ix >> 8) as u8, c.ix as u8, (c.iy >> 8) as u8, c.iy as u8] }
/// bit per register / flag that differs: A B C D E H L IX IY then flags S Z H PV N C at bits 16..21
fn diff(a: &[u8; 12], b: &[u8; 12]) -> u32 { let mut m = 0u32; let regs = [0usize, 2, 3, 4, 5, 6, 7];
    for (k, r) in regs.iter().enumerate() { if a[*r] != b[*r] { m |= 1 << k; } }
    if a[8] != b[8] || a[9] != b[9] { m |= 1 << 7; } if a[10] != b[10] || a[11] != b[11] { m |= 1 << 8; }
    for (k, bit) in [0x80u8, 0x40, 0x10, 0x04, 0x02, 0x01].iter().enumerate() { if (a[1] ^ b[1]) & bit != 0 { m |= 1 << (16 + k); } }
    m }

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = std::fs::read(&args[1]).expect("cmd file");
    let mut frames = 600u64; let mut script = String::new(); let mut dump = None; let mut every = 1u64;
    let mut trace = None; let mut profile = None; let mut hits: Vec<u16> = vec![]; let mut hitsf = None;
    let mut codewrites = None; let mut memdumps: Vec<(u64, String)> = vec![]; let mut robot: Option<(u64, u64)> = None;
    let mut pokes = vec![]; let mut keylog = None; let mut pokeat: Vec<(u64, String)> = vec![]; let mut robotlog = None; let mut passlog = None; let mut moment: Option<(u64, u16)> = None;
    let mut sys = System::new();
    let mut i = 2;
    while i < args.len() {
        let a = args[i].as_str(); let v = || args[i + 1].clone();
        match a {
            "--frames" => { frames = v().parse().unwrap(); i += 1; }
            "--script" => { script = std::fs::read_to_string(v()).unwrap(); i += 1; }
            "--dump" => { dump = Some(v()); i += 1; }
            "--every" => { every = v().parse().unwrap(); i += 1; }
            "--trace" => { trace = Some(v()); i += 1; }
            "--profile" => { profile = Some(v()); i += 1; }
            "--hits" => { let x = v(); let (l, f) = x.split_once(':').unwrap();
                hits = l.split(',').map(|h| u16::from_str_radix(h, 16).unwrap()).collect(); hitsf = Some(f.to_string()); i += 1; }
            "--codewrites" => { codewrites = Some(v()); i += 1; }
            "--memdump" => { let x = v(); let (f, p) = x.split_once(':').unwrap(); memdumps.push((f.parse().unwrap(), p.into())); i += 1; }
            "--robot" => { let x = v(); let mut q = x.split(':'); let f = q.next().unwrap().parse().unwrap();
                let s = q.next().map(|s| s.parse().unwrap()).unwrap_or(1); robot = Some((f, s)); i += 1; }
            "--poke" => { pokes.push(v()); i += 1; }
            "--pokeat" => { let x = v(); let (f, p) = x.split_once(':').unwrap(); pokeat.push((f.parse().unwrap(), p.into())); i += 1; }
            "--passlog" => { passlog = Some(v()); i += 1; }
            "--robotlog" => { robotlog = Some(v()); i += 1; }
            "--moment" => { let x = v(); let (f, p) = x.split_once(':').unwrap(); moment = Some((f.parse().unwrap(), u16::from_str_radix(p, 16).unwrap())); i += 1; }
            "--keylog" => { keylog = Some(v()); i += 1; }
            _ => panic!("unknown option {}", a),
        }
        i += 1;
    }
    let entry = sys.m.load_cmd(&cmd);
    for p in &pokes { for kv in p.split(',') { let (a, b) = kv.split_once('=').unwrap();
        sys.m.mem[usize::from_str_radix(a, 16).unwrap()] = u8::from_str_radix(b, 16).unwrap(); } }
    if codewrites.is_some() { for a in CODE_LO..CODE_HI { sys.m.watch.push(a); } }
    sys.cpu.pc = entry; sys.cpu.sp = 0x41E0;
    let mut ev: Vec<(u64, bool, String)> = vec![];
    for line in script.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() || t[0].starts_with('#') { continue; }
        let f: u64 = t[0].parse().unwrap();
        match t[1] {
            "down" => ev.push((f, true, t[2].into())),
            "up" => ev.push((f, false, t[2].into())),
            _ => { let d: u64 = t.get(3).map(|x| x.parse().unwrap()).unwrap_or(3); ev.push((f, true, t[2].into())); ev.push((f + d, false, t[2].into())); }
        }
    }
    let mut dumpf = dump.map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut tracef = trace.map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut hitw = hitsf.map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut passw = passlog.map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut keyw = keylog.map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let prof_on = profile.is_some();
    let mut excl: std::collections::BTreeMap<u16, (u64, u64)> = Default::default(); // routine -> (exclusive, calls)
    let mut incl: std::collections::BTreeMap<u16, (u64, u64, u64)> = Default::default(); // routine -> (inclusive, max, min)
    let mut stack: Vec<Frame> = vec![Frame { target: 0, sp: 0xFFFF, ret: 0, t0: 0, child: 0, regs: [0; 12] }];
    let mut dmask: std::collections::BTreeMap<u16, (u32, u64)> = Default::default();
    let mut execn: Vec<u64> = vec![0; 65536]; let mut taken: Vec<u64> = vec![0; 65536];
    let mut loops = 0u64; let mut loop_marks: Vec<u64> = vec![];
    let mut rb = Robot { seed: 1, hold: 0, rest: 0, log: vec![], last: None };
    if let Some((_, s)) = robot { rb.seed = s; }
    for f in 0..frames {
        for (ef, down, k) in &ev { if *ef == f { sys.m.key_down(k, *down); } }
        for (pf, p) in &pokeat { if *pf == f { for kv in p.split(',') { let (a, b) = kv.split_once('=').unwrap();
            sys.m.mem[usize::from_str_radix(a, 16).unwrap()] = u8::from_str_radix(b, 16).unwrap(); } } }
        if let Some((rf, _)) = robot { if f >= rf { rb.play(&mut sys, f); } }
        let end = (f + 1) * FRAME_T;
        while sys.cpu.cycles < end {
            let pc = sys.cpu.pc;
            if pc == MAIN_TOP { loops += 1; loop_marks.push(sys.cpu.cycles);
                if let Some(w) = passw.as_mut() { writeln!(w, "{} {:016X} {:016X}", sys.cpu.cycles, fnv(&sys.m.mem[0x3C00..0x4000]), fnv(&sys.m.mem[0x4C4B..0x4FE7])).unwrap(); } }
            if !hits.is_empty() && hits.contains(&pc) {
                if let Some(w) = hitw.as_mut() { let c = &sys.cpu;
                    writeln!(w, "{} {} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X}", f, c.cycles, pc, c.af(), c.bc(), c.de(), c.hl(), c.sp).unwrap(); }
            }
            let (op, sp0, t0) = (sys.m.mem[pc as usize], sys.cpu.sp, sys.cpu.cycles);
            let key_before = sys.hle_count[0];
            sys.step();
            if sys.hle_count[0] != key_before && sys.cpu.a != 0 {
                if let Some(w) = keyw.as_mut() { writeln!(w, "{} {} {:02X}", f, sys.cpu.cycles, sys.cpu.a).unwrap(); }
            }
            if prof_on {
                let dt = sys.cpu.cycles - t0;
                execn[pc as usize] += 1;
                let cond = matches!(op, 0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA | 0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC)
                    || matches!(op, 0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 | 0x20 | 0x28 | 0x30 | 0x38 | 0x10);
                if cond { let len: u16 = if (op & 0xC7) == 0xC0 { 1 } else if op <= 0x38 { 2 } else { 3 }; if sys.cpu.pc != pc.wrapping_add(len) { taken[pc as usize] += 1; } }
                let top = stack.last().unwrap().target;
                let e = excl.entry(top).or_insert((0, 0)); e.0 += dt;
                let is_call = op == 0xCD || (op & 0xC7) == 0xC4;
                if is_call && sys.cpu.sp == sp0.wrapping_sub(2) && sys.cpu.pc != pc.wrapping_add(3) {
                    stack.push(Frame { target: sys.cpu.pc, sp: sys.cpu.sp, ret: pc.wrapping_add(3), t0: sys.cpu.cycles - dt, child: 0, regs: snap(&sys.cpu) });
                    excl.entry(sys.cpu.pc).or_insert((0, 0)).1 += 1;
                } else {
                    while stack.len() > 1 {
                        let fr = stack.last().unwrap();
                        if sys.cpu.pc == fr.ret && sys.cpu.sp == fr.sp.wrapping_add(2) || sys.cpu.sp > fr.sp.wrapping_add(2) {
                            let fr = stack.pop().unwrap(); let d = sys.cpu.cycles - fr.t0;
                            if sys.cpu.pc == fr.ret { let e = dmask.entry(fr.target).or_insert((0, 0)); e.0 |= diff(&fr.regs, &snap(&sys.cpu)); e.1 += 1; }
                            let e = incl.entry(fr.target).or_insert((0, 0, u64::MAX)); e.0 += d; e.1 = e.1.max(d); e.2 = e.2.min(d);
                            let _ = fr.child;
                        } else { break; }
                    }
                }
            }
        }
        if let Some((mf, mpc)) = moment { if f == mf {
            let mut n = 0u64; while sys.cpu.pc != mpc && n < 5_000_000 { sys.step(); n += 1; }
            let shown: Vec<u8> = sys.m.mem[0x3C00..0x4000].to_vec();
            println!("{{\"pc\": \"{:04X}\", \"t\": {}, \"frame_hash\": \"{:016X}\", \"ram_hash\": \"{:016X}\"}}", sys.cpu.pc, sys.cpu.cycles, fnv(&shown), fnv(&sys.m.mem[0x4000..0x5100]));
            break;
        } }
        for (mf, mp) in &memdumps { if *mf == f { std::fs::write(mp, &sys.m.mem).unwrap(); } }
        if let Some(d) = dumpf.as_mut() { if f % every == 0 { d.write_all(&sys.m.screen()).unwrap(); } }
        if let Some(t) = tracef.as_mut() {
            let c = &sys.cpu;
            writeln!(t, "{} {} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:02X} {:016X} {:016X}", f, c.cycles, c.pc, c.af(), c.bc(), c.de(), c.hl(), c.ix, c.iy, c.sp, c.r,
                fnv(&sys.m.mem[0x3C00..0x4000]), fnv(&sys.m.mem[0x4000..0x5100])).unwrap();
        }
    }
    if let Some(p) = profile { let mut s = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        writeln!(s, "# loops {} cycles {}", loops, sys.cpu.cycles).unwrap();
        for w in loop_marks.windows(2) { writeln!(s, "L {}", w[1] - w[0]).unwrap(); }
        for (k, (t, n)) in &excl { let (it, mx, mn) = incl.get(k).copied().unwrap_or((0, 0, 0));
            writeln!(s, "R {:04X} excl {} calls {} incl {} max {} min {}", k, t, n, it, mx, if mn == u64::MAX { 0 } else { mn }).unwrap(); }
        for (k, (m, n)) in &dmask { writeln!(s, "D {:04X} {:08X} {}", k, m, n).unwrap(); }
        for a in 0..65536usize { if execn[a] > 0 { writeln!(s, "X {:04X} {} {}", a, execn[a], taken[a]).unwrap(); } } }
    if let Some(p) = codewrites { let mut s = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        for (c, pc, a, v) in &sys.m.watch_log { writeln!(s, "{} {:04X} {:04X} {:02X}", c, pc, a, v).unwrap(); } }
    if let Some(p) = robotlog { let mut s = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        for (f, d, k) in &rb.log { writeln!(s, "{} {} {}", f, if *d { "down" } else { "up" }, k).unwrap(); } }
    let mut rr = sys.m.rom_reads.clone(); rr.sort(); rr.dedup();
    eprintln!("fsrun: pc={:04X} cycles={} loops={} kbd-calls={} rom-exec={:04X?}", sys.cpu.pc, sys.cpu.cycles, loops, sys.hle_count[0], &rr[..rr.len().min(12)]);
}

/// Capture robot: looks at video RAM for saucer graphics (codes 80h-BFh in rows
/// 2-11), picks the lowest, turns the cannon toward it with ',' and '.', fires
/// with Z when no missile of its own is in flight, and keeps steering while the
/// missile flies. The program reads a key only when it goes down, so every press
/// is a tap with a rest after it.
struct Robot { seed: u64, hold: u32, rest: u32, log: Vec<(u64, bool, String)>, last: Option<String> }
impl Robot {
    fn play(&mut self, sys: &mut System, f: u64) {
        if self.hold > 0 { self.hold -= 1; if self.hold == 0 { for k in [",", ".", "Z"] { sys.m.key_down(k, false); } if let Some(k) = self.last.take() { self.log.push((f, false, k)); } self.rest = 1; } return; }
        if self.rest > 0 { self.rest -= 1; return; }
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let m = &sys.m.mem;
        let aim = m[0x4CF9] as i32; // 1..5, 3 = straight up
        // the missile in flight, if any: a '0' (30h) anywhere above the cannon
        let mut mis: Option<(i32, i32)> = None;
        for a in 0x3C40..0x3F80usize { if m[a] == 0x30 { mis = Some((((a - 0x3C00) % 64) as i32, ((a - 0x3C00) / 64) as i32)); break; } }
        let (ox, oy) = mis.unwrap_or((31, 12));
        // nearest saucer cell (graphics in rows 2..11)
        let mut best: Option<(i32, i32, i32)> = None;
        for a in 0x3C80..0x3F00usize { let v = m[a]; if (0x81..0xC0).contains(&v) {
            let x = ((a - 0x3C00) % 64) as i32; let y = ((a - 0x3C00) / 64) as i32;
            if y >= oy { continue; }
            let d = (x - ox).abs() + (oy - y) * 2; if best.map_or(true, |b| d < b.2) { best = Some((x, y, d)); } } }
        let Some((tx, ty, _)) = best else { return; };
        let dy = (oy - ty).max(1); let want = (3 + ((tx - ox) as f64 / dy as f64).round() as i32).clamp(1, 5);
        let key = if want < aim { "," } else if want > aim { "." } else if mis.is_none() && (self.seed >> 40) % 3 == 0 { "Z" } else { "" };
        if !key.is_empty() { sys.m.key_down(key, true); self.hold = 2; self.log.push((f, true, key.to_string())); self.last = Some(key.to_string()); }
    }
}
