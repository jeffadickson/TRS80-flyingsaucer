// test_moments.js -- every moment (moments/*.json) replayed on the web edition's
// machine: its input script up to the moment's field, then on to the breakpoint.
// PC, screen hash and RAM hash must equal what the Rust emulator recorded
// (tools/captures.py writes "expect").
//
//   node tests/test_moments.js FILE.CMD
const fs = require('fs');
const path = require('path');
const ROOT = path.join(__dirname, '..');
const { SaucerMachine } = require(path.join(ROOT, 'emulator', 'js', 'saucsys.js'));
function fnv(bytes) {
  let h = 0xcbf29ce484222325n;
  for (const b of bytes) { h ^= BigInt(b); h = (h * 0x100000001b3n) & 0xFFFFFFFFFFFFFFFFn; }
  return h.toString(16).toUpperCase().padStart(16, '0');
}
const cmd = new Uint8Array(fs.readFileSync(process.argv[2]));
let bad = 0;
for (const f of fs.readdirSync(path.join(ROOT, 'moments')).filter(f => f.endsWith('.json')).sort()) {
  const m = JSON.parse(fs.readFileSync(path.join(ROOT, 'moments', f), 'utf8'));
  const ev = [];
  for (const line of fs.readFileSync(path.join(ROOT, m.input_script), 'utf8').split('\n')) {
    const t = line.trim().split(/\s+/);
    if (!t[0] || t[0].startsWith('#')) continue;
    const fr = parseInt(t[0], 10);
    if (t[1] === 'down') ev.push([fr, true, t[2]]);
    else if (t[1] === 'up') ev.push([fr, false, t[2]]);
    else { const d = t[3] ? parseInt(t[3], 10) : 3; ev.push([fr, true, t[2]], [fr + d, false, t[2]]); }
  }
  const mc = new SaucerMachine(); mc.boot(cmd);
  const t0 = Date.now();
  for (let fr = 0; fr <= m.frame; fr++) {
    for (const [ef, down, k] of ev) if (ef === fr) { if (down) mc.pressKey(k); else mc.releaseKey(k); }
    mc.runTo((fr + 1) * SaucerMachine.FRAME_T);
  }
  const bp = parseInt(m.breakpoint_addr, 16);
  for (let n = 0; mc.cpu.pc !== bp && n < 5e6; n++) mc.step();
  const got = { pc: mc.cpu.pc.toString(16).toUpperCase().padStart(4, '0'), t: mc.cpu.t,
                frame_hash: fnv(mc.shown()), ram_hash: fnv(mc.mem.subarray(0x4000, 0x5100)) };
  const ok = got.pc === m.expect.pc && got.t === m.expect.t && got.frame_hash === m.expect.frame_hash && got.ram_hash === m.expect.ram_hash;
  if (!ok) bad++;
  console.log('%s %s field %d -> %s at T %d  (%d ms)%s', ok ? 'ok ' : 'BAD', m.id.padEnd(7), m.frame, m.breakpoint, got.t, Date.now() - t0,
              ok ? '' : '\n  js   ' + JSON.stringify(got) + '\n  rust ' + JSON.stringify(m.expect));
}
process.exit(bad ? 1 : 0);
