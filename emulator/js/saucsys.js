/* saucsys.js -- the Flying Saucers machine for the web edition: the series'
 * JavaScript TRS-80 Model I (trs80.js) with the same substitute ROM and the
 * same high-level emulation of the one ROM entry the program calls as the
 * Rust study emulator (emulator/rust/src/trs80.rs), cycle for cycle, so the
 * two run the program identically (tests/test_js.js checks every field's
 * T-states, registers, R, screen and RAM).
 *
 * No Tandy ROM code is used. The program calls only
 *   002Bh  $KBD  scan the keyboard: A = code of a key just pressed, 0 if none
 *          (Level II codes: letters upper case unshifted, SHIFT adds 20h; on
 *          the number/punctuation rows SHIFT flips bit 4; reported once)
 *
 *   const { SaucerMachine } = require('./saucsys.js');   // or window.SAUCSYS
 *   const m = new SaucerMachine();
 *   m.boot(cmdBytes);  m.runTo(field * SaucerMachine.FRAME_T);
 */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory(require('./trs80.js'));
  else root.SAUCSYS = factory(root.TRS80EMU);
}(typeof self !== 'undefined' ? self : this, function (EMU) {
  'use strict';

  class SaucerMachine extends EMU.TRS80 {
    constructor() { super({ stock: true }); }

    /** The Rust machine's power-on state (trs80.rs Machine::new and Z80::new). */
    reset() {
      super.reset();
      const m = this.mem;
      m.fill(0xFF, 0, 0x3000);
      m.fill(0x20, 0x3C00, 0x4000);                         // video RAM starts blank (as in trs80.rs)
      m[0x0000] = 0xF3; m[0x0001] = 0x76;                   // DI ; HALT
      m[0x002B] = 0xC9; m[0x0033] = 0xC9; m[0x0049] = 0xC9; m[0x01C9] = 0xC9; m[0x0630] = 0xC9;
      m.set([0x0B, 0x78, 0xB1, 0x20, 0xFB, 0xC9], 0x0060);
      m[0x0125] = 0x00;
      for (const a of [0x4409, 0x4420, 0x4424, 0x4428, 0x4436, 0x4439, 0x443C]) m[a] = 0xC9;
      m[0x402D] = 0x76; m[0x4020] = 0x00; m[0x4021] = 0x3C;
      const z = this.cpu;
      z.a = 0xFF; z.f = 0xFF; z.b = z.c = z.d = z.e = z.h = z.l = 0;
      z.a_ = z.f_ = z.b_ = z.c_ = z.d_ = z.e_ = z.h_ = z.l_ = 0;
      z.ix = z.iy = 0; z.i = z.r = 0; z.sp = 0xFFFF;
    }

    /** Load a /CMD file and start it the way fsrun does. */
    boot(bytes) {
      this.reset();
      const entry = this.loadCmd(bytes);
      this.cpu.pc = entry; this.cpu.sp = 0x41E0;
      return entry;
    }

    ret() {
      const z = this.cpu, m = this.mem;
      z.pc = m[z.sp] | (m[(z.sp + 1) & 0xFFFF] << 8);
      z.sp = (z.sp + 2) & 0xFFFF; z.t += 10;
    }

    step() {
      const z = this.cpu;
      if (z.pc === 0x002B) { z.a = this.scanAscii(); z.t += 400; this.ret(); return 410; }
      return super.step();
    }

    scanAscii() {
      const k = this.keys, shift = (k[7] & 1) !== 0;
      for (let r = 0; r < 7; r++) for (let b = 0; b < 8; b++) if (k[r] & (1 << b)) {
        let c;
        if (r <= 3) { c = 0x40 + r * 8 + b; if (shift && c !== 0x40) c += 32; }
        else if (r <= 5) {
          c = r === 4 ? 0x30 + b : [0x38, 0x39, 0x3A, 0x3B, 0x2C, 0x2D, 0x2E, 0x2F][b];
          if (shift && c !== 0x30) c ^= 0x10;
        } else c = [0x0D, 0x1F, 0x01, 0x5B, 0x0A, 0x08, 0x09, 0x20][b];
        k[r] &= ~(1 << b);
        return c;
      }
      return 0;
    }

    /** Video RAM as the stock Model I shows it (bit 6 = NOT(bit 5 OR bit 7)). */
    shown() {
      const v = new Uint8Array(1024);
      for (let i = 0; i < 1024; i++) { const s = this.mem[0x3C00 + i] & 0xBF; v[i] = (s & 0xA0) ? s : s | 0x40; }
      return v;
    }
  }
  SaucerMachine.FRAME_T = EMU.FRAME_T;
  SaucerMachine.CLOCK_HZ = EMU.CLOCK_HZ;
  return { SaucerMachine, EMU };
}));
