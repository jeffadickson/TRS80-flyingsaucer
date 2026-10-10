"""Static register pass: what each routine may change, on every path.

For a routine entered at E, every register and flag starts tagged with its own
name ("still the value it had on entry"). Each instruction is applied to the tags:
a write makes the destination '?' (or a copy of another tag for LD r,r'), PUSH
puts tags on an abstract stack and POP takes them back, EX DE,HL / EX (SP),HL /
EXX / EX AF,AF' swap them. A CALL applies the callee's own summary. Every path
is followed to its exit; a register is "preserved" only if it carries its own
tag at every exit.

Exits recognised: RET (and conditional RET), a jump to the restart (42EAh,
which resets SP: "never returns"), LD SP (never returns), JP (HL).

The undocumented X and Y flag bits (3 and 5) follow S in every instruction
this program uses; they are tracked with S and not listed separately.

analyze(mem, entry) -> Summary   (mem: callable addr -> byte)
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from z80dis import decode

REGS = ['A', 'B', 'C', 'D', 'E', 'H', 'L', 'IXH', 'IXL', 'IYH', 'IYL']
FLAGS = ['S', 'Z', 'Hf', 'PV', 'N', 'Cf']
ALL = REGS + FLAGS
ALT = ["A'", "B'", "C'", "D'", "E'", "H'", "L'", "F'"]
PAIR = {'bc': ('B', 'C'), 'de': ('D', 'E'), 'hl': ('H', 'L'), 'ix': ('IXH', 'IXL'), 'iy': ('IYH', 'IYL')}
R8 = {'a': 'A', 'b': 'B', 'c': 'C', 'd': 'D', 'e': 'E', 'h': 'H', 'l': 'L', 'ixh': 'IXH', 'ixl': 'IXL', 'iyh': 'IYH', 'iyl': 'IYL'}
RESTART = 0x42EA

F_ALL = FLAGS
F_INC = ['S', 'Z', 'Hf', 'PV', 'N']
F_ROTA = ['Hf', 'N', 'Cf']


class Summary:
    def __init__(self, entry):
        self.entry = entry
        self.exit_tags = None      # reg -> tag at RET exits (merged)
        self.changes = set()       # regs / flags not preserved on some RET path
        self.exits = set()         # 'RET', 'never returns (restart)', ...
        self.stack = 0             # max bytes pushed below the return address, callees included
        self.calls = set()
        self.imbalanced = []       # (pc, depth) RETs with something left on the stack
        self.path_changes = {}     # reg -> set of exit pcs where it differs (for "on some paths")
        self.ret_pcs = set()


def effects(ins):
    """(written registers, written flags, special) for one instruction."""
    t = ins.text.split(';')[0].strip().lower()
    if ins.parts:
        f, v = ins.parts; t = f.format('%04Xh' % v).lower()
    m = t.split(' ', 1); op = m[0]; args = m[1].split(',') if len(m) > 1 else []
    W, F = [], []
    def dst(x):
        x = x.strip()
        if x in R8: return [R8[x]]
        if x in PAIR: return list(PAIR[x])
        return []
    if op == 'ld':
        if args[0].strip() == 'sp': return [], [], 'ldsp'
        if args[0].strip() in ('a',) and args[1].strip() in ('i', 'r'): return ['A'], F_INC, None
        return dst(args[0]), [], ('copy', args[0].strip(), args[1].strip())
    if op in ('add', 'adc', 'sub', 'sbc', 'and', 'xor', 'or', 'cp', 'neg'):
        if args and args[0].strip() in ('hl', 'ix', 'iy') and len(args) == 2:
            return dst(args[0]), (['Hf', 'N', 'Cf'] if op == 'add' else F_ALL), None
        return ([] if op == 'cp' else ['A']), F_ALL, None
    if op in ('inc', 'dec'):
        a = args[0].strip()
        if a in PAIR or a == 'sp': return dst(a), [], None
        return dst(a), F_INC, None
    if op in ('rlca', 'rrca', 'rla', 'rra'): return ['A'], F_ROTA, None
    if op == 'daa': return ['A'], ['S', 'Z', 'Hf', 'PV', 'Cf'], None
    if op == 'cpl': return ['A'], ['Hf', 'N'], None
    if op in ('scf', 'ccf'): return [], ['Hf', 'N', 'Cf'], None
    if op in ('rlc', 'rrc', 'rl', 'rr', 'sla', 'sra', 'sll', 'srl'): return dst(args[-1]), F_ALL, None
    if op == 'bit': return [], F_INC, None
    if op in ('set', 'res'): return dst(args[-1]) if len(args) == 3 or args[-1].strip() in R8 else [], [], None
    if op in ('ldi', 'ldd', 'ldir', 'lddr'): return ['B', 'C', 'D', 'E', 'H', 'L'], ['Hf', 'PV', 'N'], None
    if op in ('cpi', 'cpd', 'cpir', 'cpdr'): return ['B', 'C', 'H', 'L'], F_INC, None
    if op == 'djnz': return ['B'], [], None
    if op == 'in': return dst(args[0]), (F_INC if '(c)' in t else []), None
    if op == 'push': return [], [], ('push', args[0].strip())
    if op == 'pop': return [], [], ('pop', args[0].strip())
    if op == 'ex': return [], [], ('ex', args[0].strip(), args[1].strip())
    if op == 'exx': return [], [], ('exx',)
    return [], [], None


def _tags_af(state):
    return [state['A']] + [state[f] for f in FLAGS]


class Analyzer:
    def __init__(self, mem, extern=None):
        self.mem = mem
        self.memo = {}
        # summaries for code outside the program: {addr: set of regs/flags it changes}
        for a, ch in (extern or {}).items():
            s = Summary(a); s.changes = set(ch); s.exits = {'RET'}
            s.exit_tags = {r: ('?' if r in ch else r) for r in ALL + ALT}
            self.memo[a] = s

    def analyze(self, entry, _depth=0):
        if entry in self.memo:
            return self.memo[entry]
        S = Summary(entry)
        self.memo[entry] = S    # guards recursion (none expected)
        init = {r: r for r in ALL}
        for r in ALT: init[r] = r
        work = [(entry, tuple(sorted(init.items())), ())]
        seen = set()
        exit_states = []
        maxdepth = 0
        steps = 0
        while work:
            pc, st, stk = work.pop()
            key = (pc, st, stk)
            if key in seen: continue
            seen.add(key)
            steps += 1
            if steps > 200000: raise RuntimeError('regs: runaway at %04X' % entry)
            state = dict(st); stack = list(stk)
            ins = decode(self.mem, pc)
            W, F, sp = effects(ins)
            nxt = [(pc + ins.length)]
            # ---- control flow
            kind = ins.kind
            if kind == 'call' or kind == 'rst':
                sub = self.analyze(ins.target, _depth + 1)
                S.calls.add(ins.target)
                d = len(stack) * 2 + 2 + sub.stack
                maxdepth = max(maxdepth, d)
                if sub.exit_tags is None:     # callee never returns
                    if not ins.cond: nxt = []
                    else: pass
                    after = None
                else:
                    after = {r: (state[sub.exit_tags[r]] if sub.exit_tags[r] in state else '?') for r in ALL + ALT}
                succ = []
                if ins.cond: succ.append((pc + ins.length, dict(state)))
                if after is not None: succ.append((pc + ins.length, after))
                for p, s2 in succ:
                    work.append((p, tuple(sorted(s2.items())), tuple(stack)))
                continue
            if kind == 'ret':
                if stack:
                    S.imbalanced.append((pc, len(stack) * 2))
                    S.exits.add('discards %d stack bytes' % (len(stack) * 2))
                else:
                    exit_states.append((pc, dict(state)))
                    S.exits.add('RET'); S.ret_pcs.add(pc)
                if ins.cond: work.append((pc + 1, st, stk))
                continue
            if kind == 'jpind' or kind == 'halt':
                S.exits.add('jp (hl)' if kind == 'jpind' else 'halt'); continue
            if sp == 'ldsp':
                S.exits.add('never returns (resets SP)'); continue
            if kind in ('jp', 'jr') and ins.target == RESTART:
                S.exits.add('never returns (restart)')
                if ins.cond: work.append((pc + ins.length, st, stk))
                continue
            # ---- data effects
            for r in W: state[r] = '?'
            for f in F: state[f] = '?'
            if sp:
                if sp[0] == 'copy':
                    d, s = sp[1], sp[2]
                    if d in R8 and s in R8: state[R8[d]] = state[R8[s]]
                elif sp[0] == 'push':
                    p = sp[1]
                    if p == 'af': stack.append(('AF', tuple(_tags_af(state))))
                    else: stack.append((p, tuple(state[x] for x in PAIR[p])))
                    maxdepth = max(maxdepth, len(stack) * 2)
                elif sp[0] == 'pop':
                    p = sp[1]
                    if not stack:
                        S.exits.add('pops its return address'); continue
                    _, tags = stack.pop()
                    if p == 'af':
                        if len(tags) == 7:
                            state['A'] = tags[0]
                            for f, tg in zip(FLAGS, tags[1:]): state[f] = tg
                        else:
                            state['A'] = '?'
                            for f in FLAGS: state[f] = '?'
                    else:
                        hi, lo = PAIR[p]
                        if len(tags) == 2: state[hi], state[lo] = tags
                        else: state[hi] = state[lo] = '?'
                elif sp[0] == 'ex':
                    a, b = sp[1], sp[2]
                    if (a, b) == ('de', 'hl'):
                        state['D'], state['E'], state['H'], state['L'] = state['H'], state['L'], state['D'], state['E']
                    elif a == '(sp)':
                        if not stack:
                            S.exits.add('ex (sp) with its return address');
                            hi, lo = PAIR[b]; state[hi] = state[lo] = '?'
                        else:
                            k, tags = stack[-1]
                            hi, lo = PAIR[b]
                            mine = (state[hi], state[lo])
                            if len(tags) == 2: state[hi], state[lo] = tags
                            else: state[hi] = state[lo] = '?'
                            stack[-1] = (b, mine)
                    elif a == 'af':
                        for r, r2 in (('A', "A'"),):
                            state[r], state[r2] = state[r2], state[r]
                        fl = [state[f] for f in FLAGS]; f2 = state["F'"]
                        for f in FLAGS: state[f] = f2 if f2 == "F'" else '?'
                        state["F'"] = '?' if any(x != fl[0] for x in fl) else fl[0]
                elif sp[0] == 'exx':
                    for r, r2 in zip(['B', 'C', 'D', 'E', 'H', 'L'], ["B'", "C'", "D'", "E'", "H'", "L'"]):
                        state[r], state[r2] = state[r2], state[r]
            succ = []
            if kind in ('jp', 'jr', 'djnz'):
                succ.append(ins.target)
                if ins.cond: succ.append(pc + ins.length)
            else:
                succ.append(pc + ins.length)
            for p in succ:
                work.append((p, tuple(sorted(state.items())), tuple(stack)))
        S.stack = maxdepth
        if exit_states:
            merged = {}
            for r in ALL + ALT:
                tags = set(s[r] for _, s in exit_states)
                merged[r] = tags.pop() if len(tags) == 1 else '?'
                for pcx, s in exit_states:
                    if s[r] != r:
                        S.path_changes.setdefault(r, set()).add(pcx)
            S.exit_tags = merged
            S.changes = set(r for r in ALL if any(s[r] != r for _, s in exit_states))
            # registers changed only on some exit paths
            S.some_paths = {r: pcs for r, pcs in S.path_changes.items()
                            if r in ALL and len(pcs) < len(exit_states) and any(s[r] == r for _, s in exit_states)}
        else:
            S.some_paths = {}
        return S


def fmt_regs(changes):
    regs = [r for r in REGS if r in changes]
    out = []
    for p, (hi, lo) in (('IX', ('IXH', 'IXL')), ('IY', ('IYH', 'IYL'))):
        if hi in regs or lo in regs:
            regs = [r for r in regs if r not in (hi, lo)]; out.append(p)
    regs = [('C (register)' if r == 'C' else r) for r in regs] + out
    fl = [{'Hf': 'H', 'Cf': 'C', 'PV': 'P/V'}.get(f, f) for f in FLAGS if f in changes]
    s = ', '.join(regs)
    if fl: s += ('; ' if s else '') + 'flags ' + ' '.join(fl)
    return s or 'nothing'


if __name__ == '__main__':
    from image import load
    run, _, _ = load()
    an = Analyzer(lambda a: run.get(a & 0xFFFF, 0xFF), {0x002B: {'A', 'D', 'E'} | set(FLAGS)})
    for e in sys.argv[1:] or ['4C26', '4BC5', '4BCE', '4BFD', '4C10', '4C1F']:
        s = an.analyze(int(e, 16))
        print('%04X changes: %s | preserves: %s | stack %d | exits %s' % (
            s.entry, fmt_regs(s.changes), fmt_regs(set(ALL) - s.changes), s.stack, sorted(s.exits)))
