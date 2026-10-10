// test_js.js -- the web edition's machine (emulator/js/saucsys.js on trs80.js)
// against the Rust study emulator, with zero tolerance: same /CMD file, same
// scripted keys, and every field's T-state count, PC, registers, R, screen
// hash and RAM hash must equal the line fsrun --trace wrote for that field.
//
//   node tests/test_js.js FILE.CMD script.txt rust.trace FIELDS
const fs = require('fs');
const path = require('path');
const { SaucerMachine } = require(path.join(__dirname, '..', 'emulator', 'js', 'saucsys.js'));

const [cmdf, scriptf, tracef, fieldsArg] = process.argv.slice(2);
const fields = parseInt(fieldsArg, 10);
const ev = [];
for (const line of fs.readFileSync(scriptf, 'utf8').split('\n')) {
  const t = line.trim().split(/\s+/);
  if (!t[0] || t[0].startsWith('#')) continue;
  const f = parseInt(t[0], 10);
  if (t[1] === 'down') ev.push([f, true, t[2]]);
  else if (t[1] === 'up') ev.push([f, false, t[2]]);
  else { const d = t[3] ? parseInt(t[3], 10) : 3; ev.push([f, true, t[2]], [f + d, false, t[2]]); }
}
function fnv(bytes) {           // 64-bit FNV-1a, as fsrun's fnv()
  let h = 0xcbf29ce484222325n;
  for (const b of bytes) { h ^= BigInt(b); h = (h * 0x100000001b3n) & 0xFFFFFFFFFFFFFFFFn; }
  return h.toString(16).toUpperCase().padStart(16, '0');
}
const hex = (v, n) => v.toString(16).toUpperCase().padStart(n, '0');
const rust = fs.readFileSync(tracef, 'utf8').trim().split('\n');
const m = new SaucerMachine();
m.boot(new Uint8Array(fs.readFileSync(cmdf)));
let bad = 0, first = null;
for (let f = 0; f < fields; f++) {
  for (const [ef, down, k] of ev) if (ef === f) { if (down) m.pressKey(k); else m.releaseKey(k); }
  m.runTo((f + 1) * SaucerMachine.FRAME_T);
  const z = m.cpu;
  const line = [f, z.t, hex(z.pc, 4), hex(z.a << 8 | z.f, 4), hex(z.b << 8 | z.c, 4), hex(z.d << 8 | z.e, 4),
    hex(z.h << 8 | z.l, 4), hex(z.ix, 4), hex(z.iy, 4), hex(z.sp, 4), hex(z.r, 2),
    fnv(m.shown()), fnv(m.mem.subarray(0x4000, 0x5100))].join(' ');
  if (line !== rust[f]) { bad++; if (!first) first = [f, line, rust[f]]; }
}
console.log('js: %s, %d fields, %d differ', path.basename(cmdf), fields, bad);
if (first) console.log(' first difference at field %d\n  js:   %s\n  rust: %s', first[0], first[1], first[2]);
process.exit(bad ? 1 : 0);
