/* trs80.js -- TRS-80 Model I emulator, JavaScript port of z80.c + trs80.c,
 * and the Model 4 (class M4), a port of m4.c.
 *
 * One self-contained file for node (require) and browsers (global TRS80EMU).
 * The Z80 core is a line-by-line port of ../z80.c: same decoding, same
 * T-state tables, same R/MEMPTR/flag behaviour; equivalence with the C
 * version is checked by validation/js_equivalence.txt (identical traces).
 *
 *   const { TRS80 } = require('./trs80.js');
 *   const m = new TRS80({ stock: false });
 *   const entry = m.loadCmd(uint8array);
 *   m.runTo(60 * TRS80.FRAME_T);         // 60 frames
 *   m.pressKey('ENTER'); m.releaseKey('ENTER');
 *   const vram = m.video();               // Uint8Array(1024)
 */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.TRS80EMU = factory();
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  // ---------------------------------------------------------------------
  // Flags
  // ---------------------------------------------------------------------
  const CF = 0x01, NF = 0x02, PF = 0x04, XF = 0x08, HF = 0x10, YF = 0x20, ZF = 0x40, SF = 0x80;

  // ---------------------------------------------------------------------
  // Timing tables (UM0080) -- identical to z80.c
  // ---------------------------------------------------------------------
  const T_MAIN = new Uint8Array([
    4,10, 7, 6, 4, 4, 7, 4, 4,11, 7, 6, 4, 4, 7, 4,
    8,10, 7, 6, 4, 4, 7, 4,12,11, 7, 6, 4, 4, 7, 4,
    7,10,16, 6, 4, 4, 7, 4, 7,11,16, 6, 4, 4, 7, 4,
    7,10,13, 6,11,11,10, 4, 7,11,13, 6, 4, 4, 7, 4,
    4, 4, 4, 4, 4, 4, 7, 4, 4, 4, 4, 4, 4, 4, 7, 4,
    4, 4, 4, 4, 4, 4, 7, 4, 4, 4, 4, 4, 4, 4, 7, 4,
    4, 4, 4, 4, 4, 4, 7, 4, 4, 4, 4, 4, 4, 4, 7, 4,
    7, 7, 7, 7, 7, 7, 4, 7, 4, 4, 4, 4, 4, 4, 7, 4,
    4, 4, 4, 4, 4, 4, 7, 4, 4, 4, 4, 4, 4, 4, 7, 4,
    4, 4, 4, 4, 4, 4, 7, 4, 4, 4, 4, 4, 4, 4, 7, 4,
    4, 4, 4, 4, 4, 4, 7, 4, 4, 4, 4, 4, 4, 4, 7, 4,
    4, 4, 4, 4, 4, 4, 7, 4, 4, 4, 4, 4, 4, 4, 7, 4,
    5,10,10,10,10,11, 7,11, 5,10,10, 0,10,17, 7,11,
    5,10,10,11,10,11, 7,11, 5, 4,10,11,10, 0, 7,11,
    5,10,10,19,10,11, 7,11, 5, 4,10, 4,10, 0, 7,11,
    5,10,10, 4,10,11, 7,11, 5, 6,10, 4,10, 0, 7,11,
  ]);
  const T_ED = new Uint8Array(256).fill(8);
  [[0x40, [12,12,15,20, 8,14, 8, 9,12,12,15,20, 8,14, 8, 9]],
   [0x50, [12,12,15,20, 8,14, 8, 9,12,12,15,20, 8,14, 8, 9]],
   [0x60, [12,12,15,20, 8,14, 8,18,12,12,15,20, 8,14, 8,18]],
   [0x70, [12,12,15,20, 8,14, 8, 8,12,12,15,20, 8,14, 8, 8]],
   [0xA0, [16,16,16,16, 8, 8, 8, 8,16,16,16,16, 8, 8, 8, 8]],
   [0xB0, [16,16,16,16, 8, 8, 8, 8,16,16,16,16, 8, 8, 8, 8]]].forEach(([base, row]) => {
    for (let i = 0; i < 16; i++) T_ED[base + i] = row[i];
  });
  function tCB(op) {
    if ((op & 7) !== 6) return 8;
    return (op >= 0x40 && op < 0x80) ? 12 : 15;
  }
  function tDD(op) {
    switch (op) {
      case 0x09: case 0x19: case 0x29: case 0x39: return 15;
      case 0x21: return 14;
      case 0x22: case 0x2A: return 20;
      case 0x23: case 0x2B: return 10;
      case 0x24: case 0x25: case 0x2C: case 0x2D: return 8;
      case 0x26: case 0x2E: return 11;
      case 0x34: case 0x35: return 23;
      case 0x36: return 19;
      case 0xE1: return 14;
      case 0xE3: return 23;
      case 0xE5: return 15;
      case 0xE9: return 8;
      case 0xF9: return 10;
    }
    if (op >= 0x40 && op < 0xC0 && op !== 0x76) {
      if ((op & 7) === 6) return 19;
      if (op < 0x80 && ((op >> 3) & 7) === 6) return 19;
      return 8;
    }
    return 4 + T_MAIN[op];
  }
  // precomputed for speed
  const T_CB = new Uint8Array(256), T_DD = new Uint8Array(256), T_DDCB = new Uint8Array(256);
  for (let i = 0; i < 256; i++) {
    T_CB[i] = tCB(i); T_DD[i] = tDD(i); T_DDCB[i] = (i >= 0x40 && i < 0x80) ? 20 : 23;
  }

  const SZ53 = new Uint8Array(256), SZ53P = new Uint8Array(256);
  for (let i = 0; i < 256; i++) {
    let f = i & (SF | YF | XF);
    if (i === 0) f |= ZF;
    SZ53[i] = f;
    let bits = 0;
    for (let b = 0; b < 8; b++) bits += (i >> b) & 1;
    SZ53P[i] = f | ((bits & 1) ? 0 : PF);
  }

  // ---------------------------------------------------------------------
  // Z80 core.  `bus` provides read(a), write(a, v), input(port), output(port, v).
  // ---------------------------------------------------------------------
  class Z80 {
    constructor(bus) {
      this.bus = bus;
      this.reset();
      this.t = 0;
    }

    reset() {
      this.a = this.f = this.b = this.c = this.d = this.e = this.h = this.l = 0xFF;
      this.a_ = this.f_ = this.b_ = this.c_ = this.d_ = this.e_ = this.h_ = this.l_ = 0xFF;
      this.ix = this.iy = 0xFFFF;
      this.sp = 0xFFFF;
      this.pc = 0;
      this.wz = 0;
      this.i = this.r = 0;
      this.iff1 = this.iff2 = 0;
      this.im = 0;
      this.halted = 0;
      this.ei_block = 0;
      this.int_line = 0;
      this.int_data = 0xFF;
      this.nmi_pending = 0;
    }

    // ---- memory helpers ----
    rb(a) { return this.bus.read(a); }
    wb(a, v) { this.bus.write(a, v); }
    rw(a) { const lo = this.bus.read(a); return lo | (this.bus.read((a + 1) & 0xFFFF) << 8); }
    ww(a, v) { this.bus.write(a, v & 0xFF); this.bus.write((a + 1) & 0xFFFF, (v >> 8) & 0xFF); }
    incR() { this.r = (this.r & 0x80) | ((this.r + 1) & 0x7F); }
    fetchOp() { this.incR(); const v = this.bus.read(this.pc); this.pc = (this.pc + 1) & 0xFFFF; return v; }
    fetch8() { const v = this.bus.read(this.pc); this.pc = (this.pc + 1) & 0xFFFF; return v; }
    fetch16() { const v = this.rw(this.pc); this.pc = (this.pc + 2) & 0xFFFF; return v; }
    push(v) {
      this.sp = (this.sp - 1) & 0xFFFF; this.bus.write(this.sp, (v >> 8) & 0xFF);
      this.sp = (this.sp - 1) & 0xFFFF; this.bus.write(this.sp, v & 0xFF);
    }
    pop() { const v = this.rw(this.sp); this.sp = (this.sp + 2) & 0xFFFF; return v; }

    // ---- register access ----
    get BC() { return (this.b << 8) | this.c; }
    get DE() { return (this.d << 8) | this.e; }
    get HL() { return (this.h << 8) | this.l; }
    get AF() { return (this.a << 8) | this.f; }
    set BC(v) { this.b = (v >> 8) & 0xFF; this.c = v & 0xFF; }
    set DE(v) { this.d = (v >> 8) & 0xFF; this.e = v & 0xFF; }
    set HL(v) { this.h = (v >> 8) & 0xFF; this.l = v & 0xFF; }
    set AF(v) { this.a = (v >> 8) & 0xFF; this.f = v & 0xFF; }

    getHL(pfx) { return pfx === 0 ? ((this.h << 8) | this.l) : pfx === 1 ? this.ix : this.iy; }
    setHL(pfx, v) {
      v &= 0xFFFF;
      if (pfx === 0) { this.h = v >> 8; this.l = v & 0xFF; } else if (pfx === 1) this.ix = v; else this.iy = v;
    }
    getR(idx, pfx) {
      switch (idx) {
        case 0: return this.b;
        case 1: return this.c;
        case 2: return this.d;
        case 3: return this.e;
        case 4: return pfx === 0 ? this.h : ((pfx === 1 ? this.ix : this.iy) >> 8);
        case 5: return pfx === 0 ? this.l : ((pfx === 1 ? this.ix : this.iy) & 0xFF);
        default: return this.a;
      }
    }
    setR(idx, pfx, v) {
      switch (idx) {
        case 0: this.b = v; break;
        case 1: this.c = v; break;
        case 2: this.d = v; break;
        case 3: this.e = v; break;
        case 4:
          if (pfx === 0) this.h = v;
          else if (pfx === 1) this.ix = (this.ix & 0x00FF) | (v << 8);
          else this.iy = (this.iy & 0x00FF) | (v << 8);
          break;
        case 5:
          if (pfx === 0) this.l = v;
          else if (pfx === 1) this.ix = (this.ix & 0xFF00) | v;
          else this.iy = (this.iy & 0xFF00) | v;
          break;
        default: this.a = v; break;
      }
    }
    getRP(p, pfx) {
      switch (p) {
        case 0: return (this.b << 8) | this.c;
        case 1: return (this.d << 8) | this.e;
        case 2: return this.getHL(pfx);
        default: return this.sp;
      }
    }
    setRP(p, pfx, v) {
      v &= 0xFFFF;
      switch (p) {
        case 0: this.b = v >> 8; this.c = v & 0xFF; break;
        case 1: this.d = v >> 8; this.e = v & 0xFF; break;
        case 2: this.setHL(pfx, v); break;
        default: this.sp = v; break;
      }
    }
    getRP2(p, pfx) { return p === 3 ? ((this.a << 8) | this.f) : this.getRP(p, pfx); }
    setRP2(p, pfx, v) { if (p === 3) { this.a = (v >> 8) & 0xFF; this.f = v & 0xFF; } else this.setRP(p, pfx, v); }

    operandAddr(pfx) {
      if (pfx === 0) return (this.h << 8) | this.l;
      let d = this.fetch8(); if (d > 127) d -= 256;
      const a = ((pfx === 1 ? this.ix : this.iy) + d) & 0xFFFF;
      this.wz = a;
      return a;
    }
    cond(y) {
      const f = this.f;
      switch (y) {
        case 0: return !(f & ZF);
        case 1: return (f & ZF) !== 0;
        case 2: return !(f & CF);
        case 3: return (f & CF) !== 0;
        case 4: return !(f & PF);
        case 5: return (f & PF) !== 0;
        case 6: return !(f & SF);
        default: return (f & SF) !== 0;
      }
    }

    // ---- ALU ----
    aluAdd(v, carry) {
      const a = this.a, r = a + v + carry, res = r & 0xFF;
      this.f = SZ53[res] | ((a ^ v ^ r) & HF) | ((((a ^ ~v) & (a ^ r)) & 0x80) >> 5) | (r >> 8);
      this.a = res;
    }
    subFlags(a, v, carry) {
      const r = (a - v - carry) & 0x1FF, res = r & 0xFF;   // 9-bit borrow
      this.f = SZ53[res] | NF | ((a ^ v ^ r) & HF) | ((((a ^ v) & (a ^ r)) & 0x80) >> 5) | ((r >> 8) & CF);
      return res;
    }
    aluOp(op, v) {
      switch (op) {
        case 0: this.aluAdd(v, 0); break;
        case 1: this.aluAdd(v, this.f & CF); break;
        case 2: this.a = this.subFlags(this.a, v, 0); break;
        case 3: this.a = this.subFlags(this.a, v, this.f & CF); break;
        case 4: this.a &= v; this.f = SZ53P[this.a] | HF; break;
        case 5: this.a ^= v; this.f = SZ53P[this.a]; break;
        case 6: this.a |= v; this.f = SZ53P[this.a]; break;
        default:
          this.subFlags(this.a, v, 0);
          this.f = (this.f & ~(XF | YF)) | (v & (XF | YF));
          break;
      }
    }
    inc8(v) {
      const r = (v + 1) & 0xFF;
      this.f = (this.f & CF) | SZ53[r] | ((r & 0x0F) === 0 ? HF : 0) | (r === 0x80 ? PF : 0);
      return r;
    }
    dec8(v) {
      const r = (v - 1) & 0xFF;
      this.f = (this.f & CF) | NF | SZ53[r] | ((r & 0x0F) === 0x0F ? HF : 0) | (r === 0x7F ? PF : 0);
      return r;
    }
    add16(a, v) {
      const r = a + v;
      this.wz = (a + 1) & 0xFFFF;
      this.f = (this.f & (SF | ZF | PF)) | ((r >> 8) & (XF | YF)) | (((a ^ v ^ r) >> 8) & HF) | (r >> 16);
      return r & 0xFFFF;
    }
    adc16(a, v) {
      const r = a + v + (this.f & CF);
      this.wz = (a + 1) & 0xFFFF;
      this.f = ((r >> 8) & (SF | XF | YF)) | ((r & 0xFFFF) ? 0 : ZF) | (((a ^ v ^ r) >> 8) & HF) |
               ((((a ^ ~v) & (a ^ r)) & 0x8000) ? PF : 0) | ((r >> 16) & 1);
      return r & 0xFFFF;
    }
    sbc16(a, v) {
      const r = (a - v - (this.f & CF)) & 0x1FFFF;       // 17-bit borrow
      this.wz = (a + 1) & 0xFFFF;
      this.f = NF | ((r >> 8) & (SF | XF | YF)) | ((r & 0xFFFF) ? 0 : ZF) | (((a ^ v ^ r) >> 8) & HF) |
               ((((a ^ v) & (a ^ r)) & 0x8000) ? PF : 0) | ((r >> 16) & 1);
      return r & 0xFFFF;
    }
    rot(op, v) {
      let c, r;
      switch (op) {
        case 0: c = v >> 7; r = ((v << 1) | c) & 0xFF; break;
        case 1: c = v & 1; r = (v >> 1) | (c << 7); break;
        case 2: c = v >> 7; r = ((v << 1) | (this.f & CF)) & 0xFF; break;
        case 3: c = v & 1; r = (v >> 1) | ((this.f & CF) << 7); break;
        case 4: c = v >> 7; r = (v << 1) & 0xFF; break;
        case 5: c = v & 1; r = (v >> 1) | (v & 0x80); break;
        case 6: c = v >> 7; r = ((v << 1) | 1) & 0xFF; break;
        default: c = v & 1; r = v >> 1; break;
      }
      this.f = SZ53P[r] | c;
      return r;
    }
    bit(n, v, xy) {
      let f = (this.f & CF) | HF | (xy & (XF | YF));
      if (!(v & (1 << n))) f |= ZF | PF;
      if (n === 7 && (v & 0x80)) f |= SF;
      this.f = f;
    }
    daa() {
      let a = this.a, diff = 0, c = this.f & CF, h;
      if ((this.f & HF) || (a & 0x0F) > 9) diff = 0x06;
      if (c || a > 0x99) { diff |= 0x60; c = CF; }
      if (this.f & NF) {
        h = ((this.f & HF) && (a & 0x0F) < 6) ? HF : 0;
        a = (a - diff) & 0xFF;
      } else {
        h = ((a & 0x0F) > 9) ? HF : 0;
        a = (a + diff) & 0xFF;
      }
      this.f = SZ53P[a] | (this.f & NF) | h | c;
      this.a = a;
    }

    // ---- CB / DDCB ----
    execCB(op) {
      const x = op >> 6, y = (op >> 3) & 7, r = op & 7;
      const addr = (this.h << 8) | this.l;
      const v = (r === 6) ? this.rb(addr) : this.getR(r, 0);
      let res;
      switch (x) {
        case 0: res = this.rot(y, v); break;
        case 1: this.bit(y, v, r === 6 ? (this.wz >> 8) : v); return;
        case 2: res = v & ~(1 << y) & 0xFF; break;
        default: res = (v | (1 << y)) & 0xFF; break;
      }
      if (r === 6) this.wb(addr, res); else this.setR(r, 0, res);
    }
    execDDCB(pfx) {
      let d = this.fetch8(); if (d > 127) d -= 256;
      const op = this.fetch8();
      const addr = ((pfx === 1 ? this.ix : this.iy) + d) & 0xFFFF;
      const x = op >> 6, y = (op >> 3) & 7, r = op & 7;
      this.wz = addr;
      const v = this.rb(addr);
      let res;
      switch (x) {
        case 0: res = this.rot(y, v); break;
        case 1: this.bit(y, v, addr >> 8); return op;
        case 2: res = v & ~(1 << y) & 0xFF; break;
        default: res = (v | (1 << y)) & 0xFF; break;
      }
      this.wb(addr, res);
      if (r !== 6) this.setR(r, 0, res);
      return op;
    }

    // ---- ED ----
    execBlock(y, zz) {
      const inc = (y & 1) ? -1 : 1;
      const repeat = y >= 6;
      const hl = (this.h << 8) | this.l;
      let bc = (this.b << 8) | this.c;
      switch (zz) {
        case 0: {
          const v = this.rb(hl);
          this.wb((this.d << 8) | this.e, v);
          this.DE = (((this.d << 8) | this.e) + inc) & 0xFFFF;
          this.HL = (hl + inc) & 0xFFFF;
          bc = (bc - 1) & 0xFFFF; this.BC = bc;
          const n = (v + this.a) & 0xFF;
          this.f = (this.f & (SF | ZF | CF)) | (n & XF) | ((n << 4) & YF) | (bc ? PF : 0);
          if (repeat && bc) {
            this.pc = (this.pc - 2) & 0xFFFF;
            this.wz = (this.pc + 1) & 0xFFFF;
            return 5;
          }
          return 0;
        }
        case 1: {
          const v = this.rb(hl);
          const r = (this.a - v) & 0xFF;
          const hf = (this.a ^ v ^ r) & HF;
          const n = (r - (hf ? 1 : 0)) & 0xFF;
          this.HL = (hl + inc) & 0xFFFF;
          bc = (bc - 1) & 0xFFFF; this.BC = bc;
          this.wz = (this.wz + inc) & 0xFFFF;
          this.f = (this.f & CF) | NF | (r & SF) | (r ? 0 : ZF) | hf | (n & XF) | ((n << 4) & YF) | (bc ? PF : 0);
          if (repeat && bc && r) {
            this.pc = (this.pc - 2) & 0xFFFF;
            this.wz = (this.pc + 1) & 0xFFFF;
            return 5;
          }
          return 0;
        }
        case 2: {
          const v = this.bus.input(bc);
          this.wz = (bc + inc) & 0xFFFF;
          this.wb(hl, v);
          this.b = (this.b - 1) & 0xFF;
          this.HL = (hl + inc) & 0xFFFF;
          const k = v + ((this.c + inc) & 0xFF);
          this.f = SZ53[this.b] | ((v & 0x80) ? NF : 0) | (k > 255 ? (HF | CF) : 0) | (SZ53P[(k & 7) ^ this.b] & PF);
          break;
        }
        default: {
          const v = this.rb(hl);
          this.b = (this.b - 1) & 0xFF;
          this.bus.output((this.b << 8) | this.c, v);
          this.wz = (((this.b << 8) | this.c) + inc) & 0xFFFF;
          this.HL = (hl + inc) & 0xFFFF;
          const k = v + this.l;
          this.f = SZ53[this.b] | ((v & 0x80) ? NF : 0) | (k > 255 ? (HF | CF) : 0) | (SZ53P[(k & 7) ^ this.b] & PF);
          break;
        }
      }
      if (repeat && this.b) {
        this.pc = (this.pc - 2) & 0xFFFF;
        return 5;
      }
      return 0;
    }

    execED(op) {
      const x = op >> 6, y = (op >> 3) & 7, zz = op & 7, p = y >> 1, q = y & 1;
      if (x === 2 && zz <= 3 && y >= 4) return this.execBlock(y, zz);
      if (x !== 1) return 0;
      switch (zz) {
        case 0: {
          const bc = (this.b << 8) | this.c;
          const v = this.bus.input(bc);
          this.wz = (bc + 1) & 0xFFFF;
          this.f = (this.f & CF) | SZ53P[v];
          if (y !== 6) this.setR(y, 0, v);
          break;
        }
        case 1: {
          const bc = (this.b << 8) | this.c;
          this.bus.output(bc, y === 6 ? 0 : this.getR(y, 0));
          this.wz = (bc + 1) & 0xFFFF;
          break;
        }
        case 2:
          if (q === 0) this.HL = this.sbc16((this.h << 8) | this.l, this.getRP(p, 0));
          else this.HL = this.adc16((this.h << 8) | this.l, this.getRP(p, 0));
          break;
        case 3: {
          const nn = this.fetch16();
          if (q === 0) this.ww(nn, this.getRP(p, 0));
          else this.setRP(p, 0, this.rw(nn));
          this.wz = (nn + 1) & 0xFFFF;
          break;
        }
        case 4:
          this.a = this.subFlags(0, this.a, 0);
          break;
        case 5:
          this.iff1 = this.iff2;
          this.pc = this.pop();
          this.wz = this.pc;
          break;
        case 6:
          this.im = [0, 0, 1, 2, 0, 0, 1, 2][y];
          break;
        default:
          switch (y) {
            case 0: this.i = this.a; break;
            case 1: this.r = this.a; break;
            case 2: case 3:
              this.a = (y === 2) ? this.i : this.r;
              this.f = (this.f & CF) | SZ53[this.a] | (this.iff2 ? PF : 0);
              break;
            case 4: {
              const hl = (this.h << 8) | this.l, v = this.rb(hl);
              this.wb(hl, ((this.a << 4) | (v >> 4)) & 0xFF);
              this.a = (this.a & 0xF0) | (v & 0x0F);
              this.f = (this.f & CF) | SZ53P[this.a];
              this.wz = (hl + 1) & 0xFFFF;
              break;
            }
            case 5: {
              const hl = (this.h << 8) | this.l, v = this.rb(hl);
              this.wb(hl, ((v << 4) | (this.a & 0x0F)) & 0xFF);
              this.a = (this.a & 0xF0) | (v >> 4);
              this.f = (this.f & CF) | SZ53P[this.a];
              this.wz = (hl + 1) & 0xFFFF;
              break;
            }
            default: break;
          }
          break;
      }
      return 0;
    }

    // ---- unprefixed / DD / FD ----
    execMain(op, pfx) {
      const x = op >> 6, y = (op >> 3) & 7, zz = op & 7, p = y >> 1, q = y & 1;
      switch (x) {
        case 0:
          switch (zz) {
            case 0:
              switch (y) {
                case 0: break;
                case 1: {
                  let t = this.a; this.a = this.a_; this.a_ = t;
                  t = this.f; this.f = this.f_; this.f_ = t;
                  break;
                }
                case 2: {
                  let e = this.fetch8(); if (e > 127) e -= 256;
                  this.b = (this.b - 1) & 0xFF;
                  if (this.b) { this.pc = (this.pc + e) & 0xFFFF; this.wz = this.pc; return 5; }
                  break;
                }
                case 3: {
                  let e = this.fetch8(); if (e > 127) e -= 256;
                  this.pc = (this.pc + e) & 0xFFFF; this.wz = this.pc;
                  break;
                }
                default: {
                  let e = this.fetch8(); if (e > 127) e -= 256;
                  if (this.cond(y - 4)) { this.pc = (this.pc + e) & 0xFFFF; this.wz = this.pc; return 5; }
                  break;
                }
              }
              break;
            case 1:
              if (q === 0) this.setRP(p, pfx, this.fetch16());
              else this.setHL(pfx, this.add16(this.getHL(pfx), this.getRP(p, pfx)));
              break;
            case 2:
              switch (y) {
                case 0: { const bc = (this.b << 8) | this.c; this.wb(bc, this.a);
                  this.wz = (this.a << 8) | ((bc + 1) & 0xFF); break; }
                case 1: { const bc = (this.b << 8) | this.c; this.a = this.rb(bc);
                  this.wz = (bc + 1) & 0xFFFF; break; }
                case 2: { const de = (this.d << 8) | this.e; this.wb(de, this.a);
                  this.wz = (this.a << 8) | ((de + 1) & 0xFF); break; }
                case 3: { const de = (this.d << 8) | this.e; this.a = this.rb(de);
                  this.wz = (de + 1) & 0xFFFF; break; }
                case 4: { const nn = this.fetch16(); this.ww(nn, this.getHL(pfx));
                  this.wz = (nn + 1) & 0xFFFF; break; }
                case 5: { const nn = this.fetch16(); this.setHL(pfx, this.rw(nn));
                  this.wz = (nn + 1) & 0xFFFF; break; }
                case 6: { const nn = this.fetch16(); this.wb(nn, this.a);
                  this.wz = (this.a << 8) | ((nn + 1) & 0xFF); break; }
                default: { const nn = this.fetch16(); this.a = this.rb(nn);
                  this.wz = (nn + 1) & 0xFFFF; break; }
              }
              break;
            case 3:
              this.setRP(p, pfx, (this.getRP(p, pfx) + (q ? -1 : 1)) & 0xFFFF);
              break;
            case 4:
            case 5:
              if (y === 6) {
                const a = this.operandAddr(pfx);
                const v = this.rb(a);
                this.wb(a, zz === 4 ? this.inc8(v) : this.dec8(v));
              } else {
                const v = this.getR(y, pfx);
                this.setR(y, pfx, zz === 4 ? this.inc8(v) : this.dec8(v));
              }
              break;
            case 6:
              if (y === 6) {
                const a = this.operandAddr(pfx);
                this.wb(a, this.fetch8());
              } else this.setR(y, pfx, this.fetch8());
              break;
            default: {
              const a = this.a, f = this.f;
              switch (y) {
                case 0: this.a = ((a << 1) | (a >> 7)) & 0xFF;
                  this.f = (f & (SF | ZF | PF)) | (this.a & (XF | YF)) | (a >> 7); break;
                case 1: this.a = ((a >> 1) | (a << 7)) & 0xFF;
                  this.f = (f & (SF | ZF | PF)) | (this.a & (XF | YF)) | (a & 1); break;
                case 2: this.a = ((a << 1) | (f & CF)) & 0xFF;
                  this.f = (f & (SF | ZF | PF)) | (this.a & (XF | YF)) | (a >> 7); break;
                case 3: this.a = ((a >> 1) | ((f & CF) << 7)) & 0xFF;
                  this.f = (f & (SF | ZF | PF)) | (this.a & (XF | YF)) | (a & 1); break;
                case 4: this.daa(); break;
                case 5: this.a = (~a) & 0xFF;
                  this.f = (f & (SF | ZF | PF | CF)) | HF | NF | (this.a & (XF | YF)); break;
                case 6: this.f = (f & (SF | ZF | PF)) | (a & (XF | YF)) | CF; break;
                default: this.f = (f & (SF | ZF | PF)) | (a & (XF | YF)) | ((f & CF) ? HF : CF); break;
              }
              break;
            }
          }
          break;

        case 1:
          if (y === 6 && zz === 6) {
            this.halted = 1;
          } else if (zz === 6) {
            const a = this.operandAddr(pfx);
            this.setR(y, 0, this.rb(a));
          } else if (y === 6) {
            const a = this.operandAddr(pfx);
            this.wb(a, this.getR(zz, 0));
          } else {
            this.setR(y, pfx, this.getR(zz, pfx));
          }
          break;

        case 2:
          if (zz === 6) this.aluOp(y, this.rb(this.operandAddr(pfx)));
          else this.aluOp(y, this.getR(zz, pfx));
          break;

        default:
          switch (zz) {
            case 0:
              if (this.cond(y)) { this.pc = this.pop(); this.wz = this.pc; return 6; }
              break;
            case 1:
              if (q === 0) this.setRP2(p, pfx, this.pop());
              else switch (p) {
                case 0: this.pc = this.pop(); this.wz = this.pc; break;
                case 1: {
                  let t;
                  t = this.b; this.b = this.b_; this.b_ = t; t = this.c; this.c = this.c_; this.c_ = t;
                  t = this.d; this.d = this.d_; this.d_ = t; t = this.e; this.e = this.e_; this.e_ = t;
                  t = this.h; this.h = this.h_; this.h_ = t; t = this.l; this.l = this.l_; this.l_ = t;
                  break;
                }
                case 2: this.pc = this.getHL(pfx); break;
                default: this.sp = this.getHL(pfx); break;
              }
              break;
            case 2: {
              const nn = this.fetch16();
              this.wz = nn;
              if (this.cond(y)) this.pc = nn;
              break;
            }
            case 3:
              switch (y) {
                case 0: this.pc = this.fetch16(); this.wz = this.pc; break;
                case 2: {
                  const n = this.fetch8();
                  this.bus.output((this.a << 8) | n, this.a);
                  this.wz = (this.a << 8) | ((n + 1) & 0xFF);
                  break;
                }
                case 3: {
                  const n = this.fetch8();
                  const port = (this.a << 8) | n;
                  this.wz = (port + 1) & 0xFFFF;
                  this.a = this.bus.input(port);
                  break;
                }
                case 4: {
                  const v = this.rw(this.sp);
                  this.ww(this.sp, this.getHL(pfx));
                  this.setHL(pfx, v);
                  this.wz = v;
                  break;
                }
                case 5: {
                  let t = this.d; this.d = this.h; this.h = t;
                  t = this.e; this.e = this.l; this.l = t;
                  break;
                }
                case 6: this.iff1 = this.iff2 = 0; break;
                case 7: this.iff1 = this.iff2 = 1; this.ei_block = 1; break;
                default: break;
              }
              break;
            case 4: {
              const nn = this.fetch16();
              this.wz = nn;
              if (this.cond(y)) { this.push(this.pc); this.pc = nn; return 7; }
              break;
            }
            case 5:
              if (q === 0) this.push(this.getRP2(p, pfx));
              else {
                const nn = this.fetch16();
                this.wz = nn;
                this.push(this.pc);
                this.pc = nn;
              }
              break;
            case 6:
              this.aluOp(y, this.fetch8());
              break;
            default:
              this.push(this.pc);
              this.pc = y * 8;
              this.wz = this.pc;
              break;
          }
          break;
      }
      return 0;
    }

    nmi() { this.nmi_pending = 1; }

    interrupt() {
      if (this.nmi_pending) {
        this.nmi_pending = 0;
        this.halted = 0;
        this.iff1 = 0;
        this.incR();
        this.push(this.pc);
        this.pc = 0x0066;
        this.wz = this.pc;
        return 11;
      }
      if (!this.int_line || !this.iff1 || this.ei_block) return 0;
      this.halted = 0;
      this.iff1 = this.iff2 = 0;
      this.incR();
      switch (this.im) {
        case 2:
          this.push(this.pc);
          this.pc = this.rw((this.i << 8) | this.int_data);
          this.wz = this.pc;
          return 19;
        case 1:
          this.push(this.pc);
          this.pc = 0x0038;
          this.wz = this.pc;
          return 13;
        default:
          this.push(this.pc);
          this.pc = ((this.int_data & 0xC7) === 0xC7) ? (this.int_data & 0x38) : 0x38;
          this.wz = this.pc;
          return 13;
      }
    }

    step() {
      let t = this.interrupt();
      this.ei_block = 0;
      if (t) { this.t += t; return t; }
      if (this.halted) { this.incR(); this.t += 4; return 4; }
      let op = this.fetchOp();
      switch (op) {
        case 0xCB:
          op = this.fetchOp();
          t = T_CB[op];
          this.execCB(op);
          break;
        case 0xED:
          op = this.fetchOp();
          t = T_ED[op] + this.execED(op);
          break;
        case 0xDD:
        case 0xFD: {
          const pfx = (op === 0xDD) ? 1 : 2;
          const next = this.bus.read(this.pc);
          if (next === 0xDD || next === 0xFD || next === 0xED) {
            t = 4;
          } else if (next === 0xCB) {
            this.fetchOp();
            t = T_DDCB[this.execDDCB(pfx)];
          } else {
            op = this.fetchOp();
            t = T_DD[op] + this.execMain(op, pfx);
          }
          break;
        }
        default:
          t = T_MAIN[op] + this.execMain(op, 0);
          break;
      }
      this.t += t;
      return t;
    }
  }

  // ---------------------------------------------------------------------
  // Trace format (identical to trace.h)
  // ---------------------------------------------------------------------
  function hex(v, n) { const s = v.toString(16).toUpperCase(); return s.length >= n ? s : '0'.repeat(n - s.length) + s; }
  function fnv1a(mem, start, end) {
    let h = 2166136261;
    for (let i = start; i < end; i++) { h ^= mem[i]; h = Math.imul(h, 16777619) >>> 0; }
    return h >>> 0;
  }
  const TRACE_HASH_START = 0x3C00;
  function traceFormat(index, z, hash) {
    return `${index} T=${z.t} PC=${hex(z.pc, 4)} AF=${hex(z.a, 2)}${hex(z.f, 2)} ` +
      `BC=${hex(z.b, 2)}${hex(z.c, 2)} DE=${hex(z.d, 2)}${hex(z.e, 2)} HL=${hex(z.h, 2)}${hex(z.l, 2)} ` +
      `AF'=${hex(z.a_, 2)}${hex(z.f_, 2)} BC'=${hex(z.b_, 2)}${hex(z.c_, 2)} ` +
      `DE'=${hex(z.d_, 2)}${hex(z.e_, 2)} HL'=${hex(z.h_, 2)}${hex(z.l_, 2)} ` +
      `IX=${hex(z.ix, 4)} IY=${hex(z.iy, 4)} SP=${hex(z.sp, 4)} I=${hex(z.i, 2)} R=${hex(z.r, 2)} ` +
      `IFF=${z.iff1}${z.iff2} IM=${z.im} HASH=${hex(hash, 8)}`;
  }

  // ---------------------------------------------------------------------
  // TRS-80 Model I machine (see trs80.h for the memory map)
  // ---------------------------------------------------------------------
  const CLOCK_HZ = 1774080, FRAME_T = 29568;
  const KEYMAP = { '@': [0, 0] };
  'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('').forEach((c, i) => { KEYMAP[c] = [(i + 1) >> 3, (i + 1) & 7]; });
  for (let i = 0; i < 8; i++) KEYMAP[String(i)] = [4, i];
  '89:;,-./'.split('').forEach((c, i) => { KEYMAP[c] = [5, i]; });
  ['ENTER', 'CLEAR', 'BREAK', 'UP', 'DOWN', 'LEFT', 'RIGHT', 'SPACE'].forEach((n, i) => { KEYMAP[n] = [6, i]; });
  KEYMAP.SHIFT = [7, 0];

  function stockVideo(v) {
    v &= 0xBF;
    if (!(v & 0xA0)) v |= 0x40;
    return v;
  }

  class TRS80 {
    constructor(opts) {
      opts = opts || {};
      this.mem = new Uint8Array(65536);
      this.keys = new Uint8Array(8);
      this.stock = opts.stock ? 1 : 0;
      this.portFF = 0;
      this.sndT = []; this.sndV = [];
      this.prof = null;
      this.marker = -1; this.markerHits = 0; this.markerTraceOn = false; this.markers = [];
      this.bp = null; this.stopReason = 0;
      const m = this;
      this.cpu = new Z80({
        read(a) { return m.busRead(a); },
        write(a, v) { m.busWrite(a, v); },
        input(p) { return m.ioIn(p); },
        output(p, v) { m.ioOut(p, v); },
      });
      this.reset();
    }

    busRead(a) {
      if (a >= 0x4000) return this.mem[a];
      if (a >= 0x3C00) return this.stock ? stockVideo(this.mem[a]) : this.mem[a];
      if (a >= 0x3800) {
        const sel = a & 0xFF; let v = 0;
        for (let r = 0; r < 8; r++) if (sel & (1 << r)) v |= this.keys[r];
        return v;
      }
      if (a >= 0x37E0) return 0xFF;
      return this.mem[a];
    }
    busWrite(a, v) {
      if (a >= 0x4000) { this.mem[a] = v; return; }
      if (a >= 0x3C00) { this.mem[a] = this.stock ? (v & 0xBF) : v; }
    }
    ioIn(port) { return 0xFF; }
    ioOut(port, v) {
      if ((port & 0xFF) !== 0xFF) return;
      this.portFF = v;
      this.sndT.push(this.cpu.t);
      this.sndV.push(v);
    }

    reset() {
      this.mem.fill(0xFF, 0, 0x3C00);
      this.mem.fill(0x20, 0x3C00, 0x4000);
      this.mem.fill(0x00, 0x4000, 0x10000);
      this.keys.fill(0);
      this.portFF = 0;
      this.cpu.reset();
      this.cpu.t = 0;
      this.sndT = []; this.sndV = [];
      this.markerHits = 0; this.markers = [];
      if (this.prof) this.prof.fill(0);
    }
    setStock(on) {
      this.stock = on ? 1 : 0;
      if (this.stock) for (let a = 0x3C00; a < 0x4000; a++) this.mem[a] &= 0xBF;
    }

    /** Load a /CMD image (Uint8Array).  Returns the entry address; throws on error. */
    loadCmd(buf) {
      let pos = 0;
      const len = buf.length;
      while (pos < len) {
        const type = buf[pos++];
        if (pos >= len) throw new Error('truncated /CMD record');
        const L = buf[pos++];
        if (type === 0x01) {
          const n = L < 3 ? L + 256 : L;
          if (pos + n > len) throw new Error('truncated /CMD record');
          const addr = buf[pos] | (buf[pos + 1] << 8);
          for (let i = 2; i < n; i++) this.busWrite((addr + i - 2) & 0xFFFF, buf[pos + i]);
          pos += n;
        } else if (type === 0x02) {
          if (L < 2 || pos + L > len) throw new Error('truncated /CMD record');
          const entry = buf[pos] | (buf[pos + 1] << 8);
          const z = this.cpu;
          z.pc = entry; z.sp = 0; z.iff1 = z.iff2 = 0; z.im = 1; z.halted = 0; z.ei_block = 0;
          return entry;
        } else {
          const n = L ? L : 256;
          if (pos + n > len) throw new Error('truncated /CMD record');
          pos += n;
        }
      }
      throw new Error('no transfer-address record');
    }

    ramHash() { return fnv1a(this.mem, TRACE_HASH_START, 0x10000); }

    step() {
      const z = this.cpu, pc = z.pc;
      if (pc === this.marker && !z.halted) {
        this.markerHits++;
        if (this.markerTraceOn) this.markers.push({ regs: this.snapshot(), hash: this.ramHash() });
      }
      const t = z.step();
      if (this.prof) this.prof[pc] += t;
      return t;
    }
    runTo(absT) {
      const start = this.cpu.t;
      this.stopReason = 0;
      let first = true;
      while (this.cpu.t < absT) {
        if (!first && this.bp && this.bp[this.cpu.pc]) { this.stopReason = 1; break; }
        first = false;
        this.step();
      }
      return this.cpu.t - start;
    }
    run(tstates) { return this.runTo(this.cpu.t + tstates); }
    runUntilPc(addr, maxT) {
      const end = this.cpu.t + maxT;
      this.stopReason = 0;
      do {
        this.step();
        if (this.cpu.pc === addr) { this.stopReason = 2; return true; }
        if (this.bp && this.bp[this.cpu.pc]) { this.stopReason = 1; return false; }
      } while (this.cpu.t < end);
      return false;
    }
    setBreakpoint(addr, on) { if (!this.bp) this.bp = new Uint8Array(65536); this.bp[addr] = on ? 1 : 0; }

    snapshot() {
      const z = this.cpu;
      return { a: z.a, f: z.f, b: z.b, c: z.c, d: z.d, e: z.e, h: z.h, l: z.l,
        a_: z.a_, f_: z.f_, b_: z.b_, c_: z.c_, d_: z.d_, e_: z.e_, h_: z.h_, l_: z.l_,
        ix: z.ix, iy: z.iy, sp: z.sp, pc: z.pc, i: z.i, r: z.r,
        iff1: z.iff1, iff2: z.iff2, im: z.im, halted: z.halted, t: z.t };
    }
    get regs() {
      const s = this.snapshot();
      return { AF: (s.a << 8) | s.f, BC: (s.b << 8) | s.c, DE: (s.d << 8) | s.e, HL: (s.h << 8) | s.l,
        "AF'": (s.a_ << 8) | s.f_, "BC'": (s.b_ << 8) | s.c_, "DE'": (s.d_ << 8) | s.e_, "HL'": (s.h_ << 8) | s.l_,
        IX: s.ix, IY: s.iy, SP: s.sp, PC: s.pc, I: s.i, R: s.r, IFF1: s.iff1, IFF2: s.iff2, IM: s.im,
        HALTED: s.halted, T: s.t, A: s.a, F: s.f };
    }
    setRegs(o) {
      const z = this.cpu;
      const pairs = { AF: ['a', 'f'], BC: ['b', 'c'], DE: ['d', 'e'], HL: ['h', 'l'],
        "AF'": ['a_', 'f_'], "BC'": ['b_', 'c_'], "DE'": ['d_', 'e_'], "HL'": ['h_', 'l_'] };
      for (const k of Object.keys(o)) {
        const v = o[k];
        if (pairs[k]) { z[pairs[k][0]] = (v >> 8) & 0xFF; z[pairs[k][1]] = v & 0xFF; }
        else if ('ABCDEFHL'.includes(k) && k.length === 1) z[k.toLowerCase()] = v & 0xFF;
        else if (k === 'T') z.t = v;
        else if (k === 'HALTED') z.halted = v ? 1 : 0;
        else z[k.toLowerCase()] = v;
      }
    }

    read(a) { return this.busRead(a & 0xFFFF); }
    write(a, v, raw) { if (raw) this.mem[a & 0xFFFF] = v & 0xFF; else this.busWrite(a & 0xFFFF, v & 0xFF); }

    setKeyRow(row, bits) { if (row >= 0 && row < 8) this.keys[row] = bits & 0xFF; }
    pressKey(name) { const [r, b] = KEYMAP[name.toUpperCase()]; this.keys[r] |= 1 << b; }
    releaseKey(name) { const [r, b] = KEYMAP[name.toUpperCase()]; this.keys[r] &= ~(1 << b); }
    releaseAll() { this.keys.fill(0); }
    setKeys(names) { this.keys.fill(0); for (const n of names) this.pressKey(n); }

    video() {
      const out = new Uint8Array(1024);
      for (let i = 0; i < 1024; i++) out[i] = this.busRead(0x3C00 + i);
      return out;
    }

    soundLog() { return { t: this.sndT, v: this.sndV }; }
    soundClear() { this.sndT = []; this.sndV = []; }

    profileEnable(on) { this.prof = on ? (this.prof || new Float64Array(65536)) : null; }
    profileReset() { if (this.prof) this.prof.fill(0); }

    setMarker(addr, trace) { this.marker = (addr == null || addr < 0) ? -1 : addr; this.markerTraceOn = !!trace; }
    markerClear() { this.markers = []; this.markerHits = 0; }
    formatMarker(i) { const r = this.markers[i]; return traceFormat(i, r.regs, r.hash); }
    formatNow(index) { return traceFormat(index, this.cpu, this.ramHash()); }
  }
  TRS80.FRAME_T = FRAME_T;
  TRS80.CLOCK_HZ = CLOCK_HZ;

  // ---------------------------------------------------------------------
  // M4 -- the TRS-80 Model 4 in Model 4 mode, a port of ../m4.c.
  // The same Z80; port 84H selects one of four memory maps, the 80 x 24
  // screen lives in a separate 2K video RAM, the keyboard matrix is the
  // Model I's (at F400H in map 2), port 90H bit 0 is the speaker.  All IN
  // return FFH and no interrupts are raised.  Every method of TRS80 works.
  //
  //   const m = new M4();
  //   m.loadCmd(bytes);                    // map 2, 80 x 24, 4 MHz, SP F3FE
  //   m.runTo(60 * M4.FRAME_T);
  //   const vram = m.video();              // Uint8Array(2048)
  //
  // new M4({ hires: true }) fits the Hi-Res Graphics Board, as m4.c does:
  // 32K of graphics RAM (128 bytes x 256 lines), X at 80H, Y at 81H, data
  // at 82H (X and Y step after each access unless the options say not),
  // options at 83H, X and Y scroll at 8CH and 8DH, text overlay at 8EH.
  // ---------------------------------------------------------------------
  const M4_CLOCK_HZ = 4055040, M4_FRAME_T = 67584, M4_HASH_START = 0x3000;
  // wait-state model settings (m4.h M4_WAIT_*) and where an instruction's I/O falls
  const WAIT = { NEVER: 0, ALWAYS: 1, IF_SET: 2, IF_CLEAR: 3, HBLANK: 0, FIXED: 1 };
  const IO_OFFSET = 8;
  const HR = { X: 0x80, Y: 0x81, DATA: 0x82, MODE: 0x83, XOFF: 0x8C, YOFF: 0x8D, OVLY: 0x8E,
               ENABLE: 0x01, XDEC: 0x04, YDEC: 0x08, XNOCLKR: 0x10, YNOCLKR: 0x20,
               XNOCLKW: 0x40, YNOCLKW: 0x80 };

  class M4 extends TRS80 {
    constructor(opts) {
      super({ stock: false });
      this.hires = !!(opts && opts.hires);
      // the board's wait-state model, as m4.c (off by default)
      this.waitWhen = WAIT.NEVER; this.waitModel = WAIT.HBLANK; this.waitFixed = 0;
      this.vt = { line: 256, lines: 264, active: 205, activeLines: 240, phase: 0 };
      this.waitTotal = 0; this.waitCount = 0;
    }

    /** when: 'never' | 'always' | 'if_set' | 'if_clear' (port 83H bit 1);
     *  model: 'hblank' (wait for the line's blanking) | 'fixed' (fixed T each access). */
    hiresWaits(when, model, fixedT) {
      this.waitWhen = WAIT[String(when).toUpperCase()];
      this.waitModel = WAIT[String(model || 'hblank').toUpperCase()];
      this.waitFixed = fixedT | 0;
    }
    videoTiming(line, lines, active, activeLines, phase) {
      if (line <= 0 || lines <= 0 || line * lines !== M4_FRAME_T || active < 0 || active > line ||
          activeLines < 0 || activeLines > lines) return false;
      this.vt = { line, lines, active, activeLines, phase: phase | 0 };
      return true;
    }
    waitClear() { this.waitTotal = 0; this.waitCount = 0; }
    hrWait() {
      const w = this.waitWhen;
      const on = w === WAIT.ALWAYS || (w === WAIT.IF_SET && (this.gmode & 0x02)) ||
                 (w === WAIT.IF_CLEAR && !(this.gmode & 0x02));
      if (!on) return;
      let extra = 0;
      if (this.waitModel === WAIT.FIXED) extra = this.waitFixed;
      else {
        const vt = this.vt, frame = vt.line * vt.lines;
        const pos = (this.cpu.t + IO_OFFSET + vt.phase) % frame;
        const line = Math.floor(pos / vt.line), x = pos % vt.line;
        if (line < vt.activeLines && x < vt.active) extra = vt.active - x;
      }
      if (extra) { this.cpu.t += extra; this.waitTotal += extra; this.waitCount++; }
    }
    step() {
      const z = this.cpu, pc = z.pc;
      if (pc === this.marker && !z.halted) {
        this.markerHits++;
        if (this.markerTraceOn) this.markers.push({ regs: this.snapshot(), hash: this.ramHash() });
      }
      const t0 = z.t;
      z.step();
      const t = z.t - t0;                 // with any wait states
      if (this.prof) this.prof[pc] += t;
      return t;
    }

    // ---- the Hi-Res Graphics Board
    hrStep(noclkX, noclkY) {
      if (!(this.gmode & noclkX)) this.gx = (this.gx + ((this.gmode & HR.XDEC) ? 255 : 1)) & 0xFF;
      if (!(this.gmode & noclkY)) this.gy = (this.gy + ((this.gmode & HR.YDEC) ? 255 : 1)) & 0xFF;
    }
    hrAddr() { return (this.gy << 7) | (this.gx & 0x7F); }
    ioIn(port) {
      if (this.hires && (port & 0xFF) === HR.DATA) {
        this.hrWait();
        const v = this.gram[this.hrAddr()];
        this.hrStep(HR.XNOCLKR, HR.YNOCLKR);
        return v;
      }
      return 0xFF;
    }
    hiresRegs() {
      return { X: this.gx, Y: this.gy, OPTIONS: this.gmode, XSCROLL: this.gxoff,
               YSCROLL: this.gyoff, OVERLAY: this.goverlay };
    }
    /** The board's picture as shown: 640 x 240, 0 or 255 (all 0 if off). */
    hiresPixels() {
      const out = new Uint8Array(640 * 240);
      if (!this.hires || !(this.gmode & HR.ENABLE)) return out;
      for (let y = 0; y < 240; y++) {
        const row = ((y + this.gyoff) & 0xFF) << 7;
        for (let bx = 0; bx < 80; bx++) {
          const b = this.gram[row + ((bx + this.gxoff) & 0x7F)];
          for (let k = 0; k < 8; k++) if (b & (0x80 >> k)) out[y * 640 + bx * 8 + k] = 255;
        }
      }
      return out;
    }

    kbRead(a) {
      const sel = a & 0xFF; let v = 0;
      for (let r = 0; r < 8; r++) if (sel & (1 << r)) v |= this.keys[r];
      return v;
    }
    pageOff() { return (this.port84 & 0x80) ? 1024 : 0; }
    busRead(a) {
      switch (this.port84 & 3) {
        case 0:
          if (a >= 0x4000) return this.mem[a];
          if (a >= 0x3C00) return this.vram[this.pageOff() + a - 0x3C00];
          if (a >= 0x3800) return this.kbRead(a);
          return 0xFF;
        case 1:
          if (a >= 0x4000 || a < 0x3800) return this.mem[a];
          if (a >= 0x3C00) return this.vram[this.pageOff() + a - 0x3C00];
          return this.kbRead(a);
        case 2:
          if (a < 0xF400) return this.mem[a];
          if (a >= 0xF800) return this.vram[a - 0xF800];
          return this.kbRead(a);
        default:
          return this.mem[a];
      }
    }
    busWrite(a, v) {
      switch (this.port84 & 3) {
        case 0:
          if (a >= 0x4000) this.mem[a] = v;
          else if (a >= 0x3C00) this.vram[this.pageOff() + a - 0x3C00] = v;
          return;
        case 1:
          if (a >= 0x4000 || a < 0x3800) this.mem[a] = v;
          else if (a >= 0x3C00) this.vram[this.pageOff() + a - 0x3C00] = v;
          return;
        case 2:
          if (a < 0xF400) this.mem[a] = v;
          else if (a >= 0xF800) this.vram[a - 0xF800] = v;
          return;
        default:
          this.mem[a] = v;
      }
    }
    ioOut(port, v) {
      const p = port & 0xFF;
      if (p >= 0x84 && p <= 0x87) this.port84 = v;
      else if (p >= 0xEC && p <= 0xEF) this.portEC = v;
      else if (p >= 0x90 && p <= 0x93) { this.sndT.push(this.cpu.t); this.sndV.push(v); this.sndP.push(0x90); }
      else if (p === 0xFF) { this.portFF = v; this.sndT.push(this.cpu.t); this.sndV.push(v); this.sndP.push(0xFF); }
      else if (this.hires) {
        switch (p) {
          case HR.X: this.gx = v; break;
          case HR.Y: this.gy = v; break;
          case HR.DATA: this.hrWait(); this.gram[this.hrAddr()] = v; this.hrStep(HR.XNOCLKW, HR.YNOCLKW); break;
          case HR.MODE: this.gmode = v; break;
          case HR.XOFF: this.gxoff = v & 0x7F; break;
          case HR.YOFF: this.gyoff = v; break;
          case HR.OVLY: this.goverlay = v & 1; break;
          default: break;
        }
      }
    }

    /** Power-on: map 0, 64 x 16, slow clock, blank screen, RAM zero. */
    reset() {
      if (!this.vram) this.vram = new Uint8Array(2048);
      if (!this.gram) this.gram = new Uint8Array(32768);
      this.mem.fill(0);
      this.vram.fill(0x20);
      this.gram.fill(0);
      this.gx = this.gy = this.gmode = this.gxoff = this.gyoff = this.goverlay = 0;
      this.keys.fill(0);
      this.port84 = 0; this.portEC = 0; this.portFF = 0;
      this.cpu.reset();
      this.cpu.t = 0;
      this.sndT = []; this.sndV = []; this.sndP = [];
      this.markerHits = 0; this.markers = [];
      if (this.prof) this.prof.fill(0);
    }
    setStock() { /* 8-bit video RAM: nothing to do */ }

    /** Load a /CMD into main RAM as TRSDOS 6 would; returns the entry. */
    loadCmd(buf) {
      let pos = 0;
      const len = buf.length;
      while (pos < len) {
        const type = buf[pos++];
        if (pos >= len) throw new Error('truncated /CMD record');
        const L = buf[pos++];
        if (type === 0x01) {
          const n = L < 3 ? L + 256 : L;
          if (pos + n > len) throw new Error('truncated /CMD record');
          const addr = buf[pos] | (buf[pos + 1] << 8);
          for (let i = 2; i < n; i++) this.mem[(addr + i - 2) & 0xFFFF] = buf[pos + i];
          pos += n;
        } else if (type === 0x02) {
          if (L < 2 || pos + L > len) throw new Error('truncated /CMD record');
          const entry = buf[pos] | (buf[pos + 1] << 8);
          const z = this.cpu;
          z.pc = entry; z.sp = 0xF3FE; z.iff1 = z.iff2 = 0; z.im = 1; z.halted = 0; z.ei_block = 0;
          this.port84 = 0x06; this.portEC = 0x40;
          return entry;
        } else {
          const n = L ? L : 256;
          if (pos + n > len) throw new Error('truncated /CMD record');
          pos += n;
        }
      }
      throw new Error('no transfer-address record');
    }

    /** FNV-1a over RAM 3000-FFFF, continued over the 2K video RAM (as m4.c). */
    ramHash() {
      let h = 2166136261;
      for (let i = M4_HASH_START; i < 0x10000; i++) { h ^= this.mem[i]; h = Math.imul(h, 16777619) >>> 0; }
      for (let i = 0; i < 2048; i++) { h ^= this.vram[i]; h = Math.imul(h, 16777619) >>> 0; }
      if (this.hires) {        // with the board: its RAM, then its six registers
        for (let i = 0; i < 32768; i++) { h ^= this.gram[i]; h = Math.imul(h, 16777619) >>> 0; }
        for (const r of [this.gx, this.gy, this.gmode, this.gxoff, this.gyoff, this.goverlay]) {
          h ^= r; h = Math.imul(h, 16777619) >>> 0;
        }
      }
      return h >>> 0;
    }

    /** All 2048 bytes of video RAM (80 x 24 shows the first 1920). */
    video() { return this.vram.slice(); }
    soundLog() { return { t: this.sndT, v: this.sndV, port: this.sndP }; }
    soundClear() { this.sndT = []; this.sndV = []; this.sndP = []; }

    /** Block-graphics pixels of rows first..last: 160 x 3(n), 0 or 255. */
    pixels(first = 0, last = 23) {
      const out = new Uint8Array((last - first + 1) * 3 * 160);
      for (let ln = first; ln <= last; ln++) {
        for (let col = 0; col < 80; col++) {
          const v = this.vram[ln * 80 + col];
          if ((v & 0xC0) !== 0x80) continue;
          for (let b = 0; b < 6; b++) {
            if (v & (1 << b)) out[((ln - first) * 3 + (b >> 1)) * 160 + col * 2 + (b & 1)] = 255;
          }
        }
      }
      return out;
    }
  }
  M4.FRAME_T = M4_FRAME_T;
  M4.CLOCK_HZ = M4_CLOCK_HZ;

  return { Z80, TRS80, M4, KEYMAP, FRAME_T, CLOCK_HZ, M4_FRAME_T, M4_CLOCK_HZ,
           traceFormat, fnv1a, TRACE_HASH_START, T_MAIN, T_ED, T_CB, T_DD, T_DDCB };
}));
