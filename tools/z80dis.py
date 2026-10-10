"""Table-driven Z80 disassembler.

decode(mem, pc) -> Insn(addr, length, bytes, mnem, ops, kind, target)
  mem: callable(addr)->byte
  kind: 'jp','jr','call','ret','rst','djnz','jpind','halt',None
  target: branch/call target (absolute) when static
  refs: list of (addr, how) memory references (absolute operands), how in {'r','w','rw','imm16'}
Operands are returned as a format list; absolute 16-bit values are tagged so the
listing generator can substitute labels.
"""
R = ['b', 'c', 'd', 'e', 'h', 'l', '(hl)', 'a']
RP = ['bc', 'de', 'hl', 'sp']
RP2 = ['bc', 'de', 'hl', 'af']
CC = ['nz', 'z', 'nc', 'c', 'po', 'pe', 'p', 'm']
ALU = ['add a,', 'adc a,', 'sub ', 'sbc a,', 'and ', 'xor ', 'or ', 'cp ']
ROT = ['rlc', 'rrc', 'rl', 'rr', 'sla', 'sra', 'sll', 'srl']


class Insn:
    __slots__ = ('addr', 'length', 'bytes', 'text', 'kind', 'target', 'cond',
                 'imm16', 'mem', 'undoc', 'parts')

    def __init__(self):
        self.kind = None; self.target = None; self.cond = False
        self.imm16 = None   # (value, role) role: 'addr' (memory/branch) or 'imm'
        self.mem = None     # (addr, 'r'|'w') absolute memory access
        self.undoc = False
        self.parts = None   # (fmt, value) where fmt contains {} for 16-bit operand


def h8(v): return ('0%02Xh' % v) if v >= 0xA0 else ('%02Xh' % v)
def h16(v): return ('0%04Xh' % v) if v >= 0xA000 else ('%04Xh' % v)


def decode(mem, pc):
    ins = Insn(); ins.addr = pc
    p = pc
    def nb():
        nonlocal p
        v = mem(p & 0xFFFF); p += 1; return v
    def nw():
        lo = nb(); hi = nb(); return lo | hi << 8

    op = nb()
    idx = None
    if op in (0xDD, 0xFD):
        idx = 'ix' if op == 0xDD else 'iy'
        op2 = mem(p & 0xFFFF)
        if op2 in (0xDD, 0xFD, 0xED):
            ins.text = 'db ' + h8(op); ins.length = 1; ins.bytes = [op]; ins.undoc = True
            return ins
        op = nb()

    def finish(text, **kw):
        ins.text = text
        ins.length = p - pc
        ins.bytes = [mem((pc + i) & 0xFFFF) for i in range(ins.length)]
        for k, v in kw.items(): setattr(ins, k, v)
        return ins

    def reg(r, disp=None, allow_idx_hl=True):
        if r == 6:
            if idx:
                d = disp
                return '(%s%s%s)' % (idx, '+' if d >= 0 else '-', h8(abs(d)))
            return '(hl)'
        if idx and allow_idx_hl and r in (4, 5):
            ins.undoc = True
            return idx + ('h' if r == 4 else 'l')
        return R[r]

    def rp(i): return idx if (i == 2 and idx) else RP[i]
    def rp2(i): return idx if (i == 2 and idx) else RP2[i]
    def sdisp():
        v = nb(); return v - 256 if v >= 128 else v

    if op == 0xCB:
        if idx:
            d = sdisp(); o = nb()
            x, y, z = o >> 6, (o >> 3) & 7, o & 7
            m = '(%s%s%s)' % (idx, '+' if d >= 0 else '-', h8(abs(d)))
            if z != 6: ins.undoc = True
            if x == 0:
                if y == 6: ins.undoc = True
                t = '%s %s' % (ROT[y], m)
            elif x == 1: t = 'bit %d,%s' % (y, m)
            elif x == 2: t = 'res %d,%s' % (y, m)
            else: t = 'set %d,%s' % (y, m)
            if z != 6 and x != 1: t += ',' + R[z]
            return finish(t)
        o = nb(); x, y, z = o >> 6, (o >> 3) & 7, o & 7
        if x == 0:
            if y == 6: ins.undoc = True
            return finish('%s %s' % (ROT[y], R[z]))
        return finish('%s %d,%s' % (['', 'bit', 'res', 'set'][x], y, R[z]))

    if op == 0xED:
        o = nb(); x, y, z = o >> 6, (o >> 3) & 7, o & 7; pp, q = y >> 1, y & 1
        if x == 1:
            if z == 0: return finish('in %s,(c)' % R[y] if y != 6 else 'in f,(c)', undoc=(y == 6))
            if z == 1: return finish('out (c),%s' % R[y] if y != 6 else 'out (c),0', undoc=(y == 6))
            if z == 2: return finish('%s hl,%s' % ('sbc' if q == 0 else 'adc', RP[pp]))
            if z == 3:
                a = nw()
                if q == 0:
                    return finish('ld ({}),%s' % RP[pp], imm16=(a, 'addr'), mem=(a, 'w'), parts=('ld ({}),%s' % RP[pp], a))
                return finish('ld %s,({})' % RP[pp], imm16=(a, 'addr'), mem=(a, 'r'), parts=('ld %s,({})' % RP[pp], a))
            if z == 4: return finish('neg', undoc=(y != 0))
            if z == 5: return finish('reti' if y == 1 else 'retn', kind='ret', undoc=(y > 1))
            if z == 6: return finish('im %s' % ['0', '0', '1', '2', '0', '0', '1', '2'][y], undoc=(y not in (0, 2, 3)))
            return finish(['ld i,a', 'ld r,a', 'ld a,i', 'ld a,r', 'rrd', 'rld', 'nop', 'nop'][y], undoc=(y >= 6))
        if x == 2 and z <= 3 and y >= 4:
            names = [['ldi', 'cpi', 'ini', 'outi'], ['ldd', 'cpd', 'ind', 'outd'],
                     ['ldir', 'cpir', 'inir', 'otir'], ['lddr', 'cpdr', 'indr', 'otdr']]
            return finish(names[y - 4][z])
        ins.undoc = True
        return finish('db 0EDh,' + h8(o))

    x, y, z = op >> 6, (op >> 3) & 7, op & 7; pp, q = y >> 1, y & 1
    if x == 0:
        if z == 0:
            if y == 0: return finish('nop')
            if y == 1: return finish("ex af,af'")
            d = sdisp(); t = (p + d) & 0xFFFF
            if y == 2: return finish('djnz {}', kind='djnz', target=t, cond=True, parts=('djnz {}', t))
            if y == 3: return finish('jr {}', kind='jr', target=t, parts=('jr {}', t))
            return finish('jr %s,{}' % CC[y - 4], kind='jr', target=t, cond=True, parts=('jr %s,{}' % CC[y - 4], t))
        if z == 1:
            if q == 0:
                v = nw(); return finish('ld %s,{}' % rp(pp), imm16=(v, 'imm'), parts=('ld %s,{}' % rp(pp), v))
            return finish('add %s,%s' % (rp(2), rp(pp)))
        if z == 2:
            if q == 0:
                if pp == 0: return finish('ld (bc),a')
                if pp == 1: return finish('ld (de),a')
                a = nw()
                if pp == 2: return finish('ld ({}),%s' % rp(2), imm16=(a, 'addr'), mem=(a, 'w'), parts=('ld ({}),%s' % rp(2), a))
                return finish('ld ({}),a', imm16=(a, 'addr'), mem=(a, 'w'), parts=('ld ({}),a', a))
            if pp == 0: return finish('ld a,(bc)')
            if pp == 1: return finish('ld a,(de)')
            a = nw()
            if pp == 2: return finish('ld %s,({})' % rp(2), imm16=(a, 'addr'), mem=(a, 'r'), parts=('ld %s,({})' % rp(2), a))
            return finish('ld a,({})', imm16=(a, 'addr'), mem=(a, 'r'), parts=('ld a,({})', a))
        if z == 3: return finish('%s %s' % ('inc' if q == 0 else 'dec', rp(pp)))
        if z in (4, 5):
            d = sdisp() if (idx and y == 6) else None
            return finish('%s %s' % ('inc' if z == 4 else 'dec', reg(y, d)))
        if z == 6:
            d = sdisp() if (idx and y == 6) else None
            n = nb(); return finish('ld %s,%s' % (reg(y, d), h8(n)))
        return finish(['rlca', 'rrca', 'rla', 'rra', 'daa', 'cpl', 'scf', 'ccf'][y])
    if x == 1:
        if op == 0x76: return finish('halt', kind='halt')
        if y == 6 or z == 6:
            d = sdisp() if idx else None
            if y == 6: return finish('ld %s,%s' % (reg(6, d), R[z]))
            return finish('ld %s,%s' % (R[y], reg(6, d)))
        return finish('ld %s,%s' % (reg(y), reg(z)))
    if x == 2:
        d = sdisp() if (idx and z == 6) else None
        return finish(ALU[y] + reg(z, d))
    # x == 3
    if z == 0: return finish('ret %s' % CC[y], kind='ret', cond=True)
    if z == 1:
        if q == 0: return finish('pop %s' % rp2(pp))
        if pp == 0: return finish('ret', kind='ret')
        if pp == 1: return finish('exx')
        if pp == 2: return finish('jp (%s)' % rp(2), kind='jpind')
        return finish('ld sp,%s' % rp(2))
    if z == 2:
        a = nw(); return finish('jp %s,{}' % CC[y], kind='jp', target=a, cond=True, imm16=(a, 'addr'), parts=('jp %s,{}' % CC[y], a))
    if z == 3:
        if y == 0:
            a = nw(); return finish('jp {}', kind='jp', target=a, imm16=(a, 'addr'), parts=('jp {}', a))
        if y == 2: n = nb(); return finish('out (%s),a' % h8(n))
        if y == 3: n = nb(); return finish('in a,(%s)' % h8(n))
        return finish(['', '', '', '', 'ex (sp),%s' % rp(2), 'ex de,hl', 'di', 'ei'][y])
    if z == 4:
        a = nw(); return finish('call %s,{}' % CC[y], kind='call', target=a, cond=True, imm16=(a, 'addr'), parts=('call %s,{}' % CC[y], a))
    if z == 5:
        if q == 0: return finish('push %s' % rp2(pp))
        a = nw(); return finish('call {}', kind='call', target=a, imm16=(a, 'addr'), parts=('call {}', a))
    if z == 6:
        n = nb(); return finish(ALU[y] + h8(n))
    return finish('rst %s' % h8(y * 8), kind='rst', target=y * 8)


def fmt(ins, label=None):
    """Render with optional label lookup for the 16-bit operand."""
    if ins.parts:
        f, v = ins.parts
        s = label(v) if label else None
        return f.format(s if s else h16(v))
    return ins.text
