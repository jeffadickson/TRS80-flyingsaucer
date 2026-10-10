//! Headless TRS-80 Model I runner, used for the Frogger study (it began as the
//! Galaxy Invasion Plus study emulator).
//!
//! trs80run <file.cmd> [options]
//!   --frames N            run N video fields (1/60 s each, 29568 T-states)
//!   --script FILE         lines: "<frame> <down|up|tap> KEY [frames]"  (tap = down for 3 frames)
//!   --dump FILE           write every frame's video RAM (1024 bytes + 1 flag byte) to FILE
//!   --every K             dump only every K-th frame
//!   --trace FILE          per-frame trace: frame cycles pc af bc de hl ix iy sp ramhash
//!   --sound FILE          write cassette-port change log (T-state value) to FILE
//!   --hiscore FILE        HISCORES/GAL contents (loaded if present, saved at exit)
//!   --lowercase           emulate a Model I with the lowercase modification
//!   --joystick HEX        value returned by port 00 (default FF = no joystick)
//!   --profile FILE        write per-PC T-state totals (instruction-level profile)
//!   --writes FILE         log every write to program memory (cycles pc addr value)
//!   --poke ADDR=VAL,...   patch memory after load (hex)
//!   --dos-log             print the HLE DOS call log
use std::io::Write;
use z80emu::trs80::{System, FRAME_T};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = std::fs::read(&args[1]).expect("cmd file");
    let mut frames = 600u64; let mut script = String::new(); let mut dump = None; let mut every = 1u64;
    let mut trace = None; let mut sound = None; let mut hiscore = None; let mut profile = None; let mut writes = None;
    let mut dos_log = false; let mut pcdumps: Vec<(u16,u64,String)> = Vec::new(); let mut pchits: Vec<u64> = Vec::new(); let mut autoplay: Option<u64> = None; let mut autoplay_end: u64 = u64::MAX; let mut auto_seed: u64 = 1; let mut budget: Option<String> = None; let mut loop_lo: u16 = 0x808A; let mut loop_hi: u16 = 0x80EC; let mut loop_top: u16 = 0x808C; let mut bud_screen: u8 = 0; let mut pokes = Vec::new(); let mut memdumps: Vec<(u64,String)> = Vec::new();
    let mut sys = System::new();
    let mut i = 2;
    while i < args.len() {
        let a = args[i].as_str();
        let v = || args[i + 1].clone();
        match a {
            "--frames" => { frames = v().parse().unwrap(); i += 1; }
            "--script" => { script = std::fs::read_to_string(v()).unwrap(); i += 1; }
            "--dump" => { dump = Some(v()); i += 1; }
            "--every" => { every = v().parse().unwrap(); i += 1; }
            "--trace" => { trace = Some(v()); i += 1; }
            "--sound" => { sound = Some(v()); i += 1; }
            "--hiscore" => { hiscore = Some(v()); i += 1; }
            "--profile" => { profile = Some(v()); i += 1; }
            "--writes" => { writes = Some(v()); i += 1; }
            "--poke" => { pokes.push(v()); i += 1; }
            "--memdump" => { let x = v(); let (f, p) = x.split_once(':').unwrap(); memdumps.push((f.parse().unwrap(), p.to_string())); i += 1; }
            "--lowercase" => sys.m.lowercase_mod = true,
            "--joystick" => { sys.m.joystick = u8::from_str_radix(&v(), 16).unwrap(); i += 1; }
            "--dos-log" => dos_log = true,
            "--memdump-pc" => { let x = v(); let q: Vec<&str> = x.split(':').collect(); pcdumps.push((u16::from_str_radix(q[0],16).unwrap(), q[1].parse().unwrap(), q[2].to_string())); pchits.push(0); i += 1; }
            "--autoplay" => { autoplay = Some(v().parse().unwrap()); i += 1; }
            "--autoplay-end" => { autoplay_end = v().parse().unwrap(); i += 1; }
            "--seed" => { auto_seed = v().parse().unwrap(); i += 1; }
            "--park" => { ROBOT.lock().unwrap().park = true; }
            "--budget" => { budget = Some(v()); i += 1; }
            "--budget-screen" => { bud_screen = v().parse().unwrap(); i += 1; }
            "--loop" => { let x = v(); let q: Vec<&str> = x.split(|c| c == '-' || c == ':').collect(); loop_lo = u16::from_str_radix(q[0],16).unwrap(); loop_hi = u16::from_str_radix(q[1],16).unwrap(); loop_top = u16::from_str_radix(q.get(2).unwrap_or(&q[0]),16).unwrap(); i += 1; }
            "--watch" => { for w in v().split(',') { if let Some((x,y)) = w.split_once('-') { for q in u16::from_str_radix(x,16).unwrap()..=u16::from_str_radix(y,16).unwrap() { sys.m.watch.push(q); } } else { sys.m.watch.push(u16::from_str_radix(w,16).unwrap()); } } i += 1; }
            _ => panic!("unknown option {}", a),
        }
        i += 1;
    }
    // The program file itself is the high-score file (FROGGER/CMD rewrites its first record).
    sys.m.hiscore_file = Some(cmd.clone());
    if let Some(h) = &hiscore { if let Ok(d) = std::fs::read(h) { sys.m.hiscore_file = Some(d); } }
    if writes.is_some() { sys.m.write_log = Some(Vec::new()); }
    let entry = sys.m.load_cmd(&cmd);
    for p in &pokes { for kv in p.split(',') { let (a, b) = kv.split_once('=').unwrap();
        sys.m.mem[usize::from_str_radix(a, 16).unwrap()] = u8::from_str_radix(b, 16).unwrap(); } }
    sys.cpu.pc = entry; sys.cpu.sp = 0x41E0; // DOS leaves its stack low in memory
    // key events
    let mut ev: Vec<(u64, bool, String)> = Vec::new();
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
    let mut prof: Vec<u64> = if profile.is_some() { vec![0; 65536] } else { vec![] };
    let mut bud: std::collections::BTreeMap<u32, (u64, u64)> = Default::default();
    let mut calls: std::collections::BTreeMap<u32, u64> = Default::default();
    let mut ticks: std::collections::BTreeMap<u32, u64> = Default::default();
    let mut cur: u32 = 0xFFFF; let mut loops: u64 = 0; let mut loop_start: Vec<(u64, u8, u8)> = Vec::new();
    for f in 0..frames {
        for (ef, down, k) in &ev { if *ef == f { sys.m.key_down(k, *down); } }
        if let Some(af) = autoplay { if f >= af && f < autoplay_end { auto_pilot(&mut sys, f, &mut auto_seed); } else if f == autoplay_end { for k in ["LEFT","RIGHT","UP","DOWN"] { sys.m.key_down(k, false); } } }
        let end = (f + 1) * FRAME_T;
        if !pcdumps.is_empty() {
            while sys.cpu.cycles < end {
                for (k, (a, n, p)) in pcdumps.iter().enumerate() { if sys.cpu.pc == *a { pchits[k] += 1; if pchits[k] == *n { std::fs::write(p, &sys.m.mem).unwrap(); eprintln!("pc dump {:04X} #{} at frame {} t={}", a, n, f, sys.cpu.cycles); } } }
                sys.step();
            }
        } else if profile.is_some() || budget.is_some() {
            while sys.cpu.cycles < end {
                let pc = sys.cpu.pc;
                if budget.is_some() {
                    // Budget: attribute time to the routine CALLed from the main loop
                    // (--loop LO-HI:TOP, default 808A-80EC:808C, Frogger's MainLoop).
                    // Column 3 is the part of that time spent in PlayNote (8FB8h-9010h).
                    let in_loop = (loop_lo..loop_hi).contains(&pc);
                    if in_loop {
                        if pc == loop_top { loops += 1; loop_start.push((sys.cpu.cycles, sys.m.mem[0x8089], sys.m.mem[0x94AD])); }
                        let op = sys.m.mem[pc as usize];
                        if op == 0xCD { cur = sys.m.mem[pc as usize + 1] as u32 | (sys.m.mem[pc as usize + 2] as u32) << 8; } else { cur = loop_lo as u32; }
                    }
                    let t = sys.step();
                    let tick = (0x8FB8..0x9011).contains(&pc);
                    let counted = bud_screen == 0 || (sys.m.mem[0x8089] == bud_screen && sys.m.mem[0x94AD] >= 10 && sys.m.mem[0x94AD] < 0x3B);
                    if counted { let e = bud.entry(cur).or_insert((0u64, 0u64)); e.0 += t; if tick { e.1 += t; }
                    if pc == 0x8FB8 { *ticks.entry(cur).or_insert(0) += 1; }
                    if in_loop { let op = sys.m.mem[pc as usize]; if op == 0xCD { *calls.entry(cur).or_insert(0) += 1; } } }
                    if !prof.is_empty() { prof[pc as usize] += t; }
                } else { let t = sys.step(); prof[pc as usize] += t; }
            }
        } else { sys.run_until(end); }
        for (mf, mp) in &memdumps { if *mf == f { std::fs::write(mp, &sys.m.mem).unwrap(); } }
        if let Some(d) = dumpf.as_mut() { if f % every == 0 { d.write_all(&sys.m.screen()).unwrap(); d.write_all(&[sys.m.wide as u8]).unwrap(); } }
        if let Some(t) = tracef.as_mut() {
            let c = &sys.cpu;
            let mut h: u64 = 0xcbf29ce484222325; for b in &sys.m.mem[0x3C00..] { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
            writeln!(t, "{} {} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:02X} {:016X}", f, c.cycles, c.pc, c.af(), c.bc(), c.de(), c.hl(), c.ix, c.iy, c.sp, c.r, h).unwrap();
        }
    }
    if let Some(p) = &budget { let mut s = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        writeln!(s, "# loops {}", loops).unwrap();
        for w in loop_start.windows(2) { writeln!(s, "L {} {} {} {}", w[0].0, w[1].0 - w[0].0, w[0].1, w[0].2).unwrap(); }
        for (k, (t, tk)) in &bud { writeln!(s, "R {:04X} {} {} {} {}", k, t, tk, calls.get(k).unwrap_or(&0), ticks.get(k).unwrap_or(&0)).unwrap(); } }
    if let Some(p) = sound { let mut s = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        for (t, v) in &sys.m.sound { writeln!(s, "{} {}", t, v).unwrap(); } }
    if let Some(p) = profile { let mut s = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        for (a, t) in prof.iter().enumerate() { if *t > 0 { writeln!(s, "{:04X} {}", a, t).unwrap(); } } }
    if let Some(p) = writes { let mut s = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        for (c, pc, a, v) in sys.m.write_log.as_ref().unwrap() { writeln!(s, "{} {:04X} {:04X} {:02X}", c, pc, a, v).unwrap(); } }
    if let (Some(h), Some(d)) = (&hiscore, &sys.m.hiscore_file) { std::fs::write(h, d).unwrap(); }
    for (c, pc, a, v) in &sys.m.watch_log { println!("W f={} t={} pc={:04X} {:04X}={:02X}", c / FRAME_T, c, pc, a, v); }
    if dos_log { for l in &sys.dos_log { eprintln!("{}", l); } }
    let mut rr = sys.m.rom_reads.clone(); rr.sort(); rr.dedup();
    eprintln!("final pc={:04X} cycles={} hle kbd={} dsp={} key={} cls={} bs={} rom-exec-outside-stubs={:04X?}", sys.cpu.pc, sys.cpu.cycles, sys.hle_count[0], sys.hle_count[1], sys.hle_count[2], sys.hle_count[3], sys.hle_count[4], &rr[..rr.len().min(20)]);
}


/// The capture robot: a simple Frogger player used only to film gameplay.
/// It reads the game's own tables (frog lane 92C4h, column 92C3h, the road
/// objects at 7FADh, the river objects at 8001h/8013h/8025h with lanes from
/// 8C11h, the homes at 9721h) and presses one arrow key at a time, releasing
/// it once the frog has moved, because the game only acts on a change of keys.
fn auto_pilot(sys: &mut System, f: u64, seed: &mut u64) {
    let m = &sys.m.mem;
    let screen = m[0x8089];               // 1 road, 2 river
    let lane = m[0x92C4] as i32;          // 0 top .. 7 bottom
    let fx = m[0x92C3] as i32;            // frog column 0..58
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    // release keys for two fields after every press
    let st = &mut ROBOT.lock().unwrap();
    // hold the key until the frog has moved (the game reads the keys once a pass)
    let now = (m[0x92C4], m[0x92C3], m[0x8089]);
    if st.hold > 0 && now != st.at { st.hold = 1; }
    if st.hold > 0 { st.hold -= 1; if st.hold == 0 { for k in ["UP","DOWN","LEFT","RIGHT"] { sys.m.key_down(k, false); } st.rest = 2; } return; }
    if st.rest > 0 { st.rest -= 1; return; }
    // --park: once the frog is on the river bank, stay there
    if st.park && screen == 2 && lane == 7 { return; }
    // occupied column ranges of a lane, `ahead` passes from now
    let occ = |l: i32, ahead: i32| -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        if screen == 1 {
            for k in 0..21 { let r = 0x7FAD + k * 4; let sp = m[r];
                if sp == 0 || m[r + 2] as i32 != l { continue; }
                let w = [8, 8, 16, 8, 6, 8, 9][(sp - 1) as usize];
                let dx = m[r + 3] as i8 as i32; let x = ((m[r + 1] as i32 + dx * ahead) % 96 + 96) % 96;
                v.push((x - 16, x - 16 + w - 1)); }
        } else {
            for k in 0..18 { if m[0x8C11 + k] as i32 != l { continue; }
                let sp = m[0x8001 + k] as usize;
                let w = [19, 32, 28, 28, 27, 28, 27, 25, 0, 28, 29][sp.min(10)];
                if sp == 8 || sp == 2 || sp == 3 || sp == 4 && ahead > 0 { if sp == 8 || sp == 4 { continue; } }
                let dx = m[0x8025 + k] as i8 as i32; let x = ((m[0x8013 + k] as i32 + dx * ahead) % 120 + 120) % 120;
                v.push((x - 32, x - 32 + w - 1)); }
        }
        v
    };
    let hit = |l: i32, x0: i32, ahead: i32| occ(l, ahead).iter().any(|&(a, b)| b >= x0 - 1 && a <= x0 + 7);
    let on = |l: i32, x0: i32, ahead: i32| occ(l, ahead).iter().any(|&(a, b)| a <= x0 + 1 && b >= x0 + 5);
    let mut key = "";
    if screen == 1 {
        // first pass (0..12) at which lane l at column x would be hit
        let danger = |l: i32, x: i32| -> i32 { if l >= 7 || l < 0 { return 99; } (0..12).find(|&t| hit(l, x, t)).unwrap_or(99) };
        let up = if lane == 0 { 99 } else { danger(lane - 1, fx) };
        let here = danger(lane, fx);
        if up >= 6 || (lane == 0) { key = "UP"; }
        else if here <= 2 {
            let mut best = (here, "");
            let opts = [(up, "UP"), (if lane < 7 { danger(lane + 1, fx) } else { -1 }, "DOWN"),
                        (if fx >= 4 { danger(lane, fx - 4) } else { -1 }, "LEFT"), (if fx <= 54 { danger(lane, fx + 4) } else { -1 }, "RIGHT")];
            for (d, k) in opts { if d > best.0 { best = (d, k); } }
            key = best.1;
        }
    } else if lane == 1 {
        // line up under an empty home, then jump
        let mut best: Option<i32> = None;
        for h in 0..5 { let hx = m[0x9721 + h * 2] as i32; let st = m[0x9722 + h * 2];
            if st == 0 || st == 3 { if best.map_or(true, |b| (hx - fx).abs() < (b - fx).abs()) { best = Some(hx); } } }
        if let Some(hx) = best {
            if (hx - fx).abs() <= 2 { key = "UP"; }
            else if hx < fx && (0..2).any(|t| on(1, fx - 4, t)) { key = "LEFT"; }
            else if hx > fx && (0..2).any(|t| on(1, fx + 4, t)) { key = "RIGHT"; }
        }
    } else {
        if (1..3).all(|t| on(lane - 1, fx, t)) { key = "UP"; }
        else if lane < 7 && !(1..3).all(|t| on(lane, fx, t)) {
            if lane < 7 && (lane + 1 == 7 || (1..3).all(|t| on(lane + 1, fx, t))) { key = "DOWN"; }
        }
    }
    let _ = f;
    if (*seed >> 33) % 17 == 0 { key = ""; }
    if !key.is_empty() { sys.m.key_down(key, true); st.hold = 14; st.at = now; }
}

struct Robot { hold: u32, rest: u32, park: bool, at: (u8, u8, u8) }
static ROBOT: std::sync::Mutex<Robot> = std::sync::Mutex::new(Robot { hold: 0, rest: 0, park: false, at: (0, 0, 0) });
