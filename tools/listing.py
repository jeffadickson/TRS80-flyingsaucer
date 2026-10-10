"""Build the commented listing of Flying Saucers from the program file + tools/annot.py.

  python3 tools/listing.py [path/to/flysauc5.cmd]

Outputs (build/):
  flysaucers.asm   pasmo source; reassembles to the original byte for byte
  flysaucers.lst   the same with addresses and bytes, for reading
  listing.json     structured listing for the web edition
  routines.json    the derived header facts (registers, stack, cycles, coverage)

Header lines derived here, not typed:
  Changes / Preserves   tools/regs.py (every path) checked against the emulator's
                        entry/exit comparison of every call in the scenarios
                        (build/run/*.prof, `make measure`); a register the
                        emulator saw change that the static pass says is preserved
                        stops the build.
  Stack                 tools/regs.py, callees included
  Cycles                measured: fewest and most T-states of a call, all scenarios
  Calls / Called from   static cross-references
"""
import os, sys, json, bisect, glob
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from paths import ORIGINAL, BUILD, RUN, GAME_LO
from z80dis import decode, h8, h16
from image import load
import regs as RG
import annot as A

src = sys.argv[1] if len(sys.argv) > 1 else ORIGINAL
RUNMEM, _, XFER = load(src)
mem = lambda a: RUNMEM.get(a & 0xFFFF, 0)
SEG = A.SEGMENTS
def loaded(a): return any(s <= a < e for s, e in SEG)

_data_sorted = sorted(A.DATA)
def is_data(a):
    return any(d[0] <= a < d[1] for d in _data_sorted)

# ---------------------------------------------------------------- code discovery
def trace(entries):
    seen = {}; work = list(entries)
    while work:
        a = work.pop()
        while a not in seen and loaded(a) and not is_data(a):
            i = decode(mem, a); seen[a] = i
            if i.kind in ('call', 'jp', 'jr', 'djnz') and i.target is not None and loaded(i.target):
                work.append(i.target)
            if (i.kind in ('ret', 'jp', 'jr') and not i.cond) or i.kind in ('jpind', 'halt'): break
            a += i.length
    return seen

CODE = trace(A.ENTRIES)
code_bytes = {}
for a, i in CODE.items():
    for k in range(i.length): code_bytes[a + k] = a

# ---------------------------------------------------------------- measurements
EXT = {0x002B: {'A', 'D', 'E'} | set(RG.FLAGS)}
AN = RG.Analyzer(mem, EXT)
prof = {'excl': {}, 'calls': {}, 'min': {}, 'max': {}, 'diff': {}, 'dcalls': {}, 'exec': {}, 'taken': {}}
profs = sorted(glob.glob(os.path.join(RUN, '*.prof')))
for p in profs:
    for ln in open(p):
        t = ln.split()
        if not t or t[0] == '#': continue
        if t[0] == 'R':
            a = int(t[1], 16); calls = int(t[5]); mx = int(t[9]); mn = int(t[11])
            prof['calls'][a] = prof['calls'].get(a, 0) + calls
            if calls:
                prof['max'][a] = max(prof['max'].get(a, 0), mx)
                if mn: prof['min'][a] = min(prof['min'].get(a, 1 << 60), mn)
        elif t[0] == 'D':
            a = int(t[1], 16); prof['diff'][a] = prof['diff'].get(a, 0) | int(t[2], 16)
            prof['dcalls'][a] = prof['dcalls'].get(a, 0) + int(t[3])
        elif t[0] == 'X':
            a = int(t[1], 16); prof['exec'][a] = prof['exec'].get(a, 0) + int(t[2]); prof['taken'][a] = prof['taken'].get(a, 0) + int(t[3])
DIFFBITS = ['A', 'B', 'C', 'D', 'E', 'H', 'L', 'IX', 'IY'] + [None] * 7 + ['S', 'Z', 'Hf', 'PV', 'N', 'Cf']
def diff_set(m):
    out = set()
    for k, n in enumerate(DIFFBITS):
        if n and m >> k & 1:
            out |= ({'IXH', 'IXL'} if n == 'IX' else {'IYH', 'IYL'} if n == 'IY' else {n})
    return out

# ---------------------------------------------------------------- labels
LABEL = {}
for a, (n, _h) in A.ROUTINES.items(): LABEL[a] = n
for a, n in A.LABELS.items(): LABEL[a] = n
for a, v in A.VARS.items(): LABEL.setdefault(a, v[0])
for d in A.DATA:
    if d[3]: LABEL[d[0]] = d[3]
for a, n in A.EXTERN.items(): LABEL[a] = n

starts = sorted(A.ROUTINES)
def owner(a):
    k = bisect.bisect_right(starts, a) - 1
    return starts[k] if k >= 0 else None

xref = {}
for a, i in CODE.items():
    if i.target is not None: xref.setdefault(i.target, []).append((a, i.kind))
    if i.imm16 is not None: xref.setdefault(i.imm16[0], []).append((a, 'ref'))

auto_n = {}
for t in sorted(xref):
    if t in CODE and t not in LABEL:
        o = owner(t); base = LABEL.get(o, 'L') if o is not None else 'L'
        auto_n[base] = auto_n.get(base, 0) + 1
        LABEL[t] = '%s_%d' % (base, auto_n[base])

VAR_RANGES = sorted((a, a + max(1, v[1]), v[0]) for a, v in A.VARS.items())
DATA_RANGES = sorted((d[0], d[1], d[3]) for d in A.DATA if d[3])
def var_off(v):
    best = None
    for a, e, n in VAR_RANGES + DATA_RANGES:
        if a <= v < e and (best is None or a > best[0]): best = (a, n)
    if best and best[0] != v: return '%s+%d' % (best[1], v - best[0])
    return None

def opnd_label(v, role='imm'):
    if v in LABEL and not (v in code_bytes and code_bytes[v] != v): return LABEL[v]
    if v in code_bytes and code_bytes[v] != v:          # into the middle of an instruction
        base = code_bytes[v]
        if base in LABEL: return '%s+%d' % (LABEL[base], v - base)
    if v in A.HWNAMES: return A.HWNAMES[v]
    if 0x3C00 < v < 0x4000: return 'VIDEO+%d' % (v - 0x3C00)
    if role == 'imm' and v < 0x3C00: return None
    return var_off(v)

# ---------------------------------------------------------------- routine bodies (for coverage)
def body(entry):
    """Instructions reachable from entry without entering another routine's entry."""
    seen = set(); work = [entry]
    while work:
        a = work.pop()
        while a in CODE and a not in seen:
            if a != entry and a in A.ROUTINES: break
            seen.add(a); i = CODE[a]
            if i.kind in ('jp', 'jr', 'djnz') and i.target in CODE and i.target not in A.ROUTINES: work.append(i.target)
            if (i.kind in ('ret', 'jp', 'jr') and not i.cond) or i.kind in ('jpind', 'halt'): break
            a += i.length
    return seen

def mem_refs(b):
    rd, wr = set(), set()
    for a in b:
        i = CODE[a]
        if i.mem:
            v, how = i.mem
            n = opnd_label(v, 'addr') or h16(v)
            (wr if how == 'w' else rd).add(n)
    return sorted(rd), sorted(wr)

FACTS = {}
problems = []
for a, (name, hdr) in A.ROUTINES.items():
    if a not in CODE: continue
    S = AN.analyze(a)
    b = body(a)
    conds = [x for x in b if CODE[x].cond]
    both = sum(1 for x in conds if prof['taken'].get(x, 0) and prof['exec'].get(x, 0) > prof['taken'].get(x, 0))
    ran = sum(1 for x in b if prof['exec'].get(x, 0))
    calls = prof['dcalls'].get(a, 0)
    measured = diff_set(prof['diff'].get(a, 0))
    leak = [r for r in measured if r not in S.changes]
    if leak: problems.append('%s: emulator saw %s change, static pass says preserved' % (name, leak))
    rd, wr = mem_refs(b)
    FACTS[a] = dict(name=name, changes=RG.fmt_regs(S.changes), preserves=RG.fmt_regs(set(RG.ALL) - S.changes),
                    some=sorted(S.some_paths) if hasattr(S, 'some_paths') else [],
                    stack=S.stack, exits=sorted(S.exits), calls=calls, ncond=len(conds), both=both,
                    insns=len(b), ran=ran, tmin=prof['min'].get(a), tmax=prof['max'].get(a),
                    reads=rd, writes=wr, callees=sorted(S.calls))
if problems:
    for p in problems: print('REGISTER CHECK:', p, file=sys.stderr)
    sys.exit(1)

# ---------------------------------------------------------------- data rendering
def esc_text(bs):
    items = []; cur = ''
    for b in bs:
        if 32 <= b < 127 and b not in (34, 39):
            cur += chr(b)
        else:
            if cur: items.append('"%s"' % cur); cur = ''
            items.append(h8(b))
    if cur: items.append('"%s"' % cur)
    return ','.join(items)

def gfx_comment(bs):
    s = ''
    for v in bs:
        if 0x80 <= v < 0xC0: s += '#' if v == 0xBF else ('.' if v == 0x80 else '+')
        elif v == 0x20: s += '_'
        elif v == 0: s += '|'
        else: s += chr(v) if 32 < v < 127 else '/'
    return s

def data_lines(start, end, kind, opts):
    a = start
    if kind == 'text':
        term = set(opts.get('split', [0x00]))
        while a < end:
            b = a
            while b < end and (b - a) < 48:
                b += 1
                if mem(b - 1) in term: break
            yield a, b, ''
            a = b
    elif kind == 'words':
        w = opts.get('w', 8)
        while a < end:
            b = min(a + w, end); yield a, b, ''; a = b
    else:
        w = opts.get('w', 16)
        while a < end:
            b = min(a + w, end); yield a, b, ''; a = b

lines = []
def emit(addr, bs, text, comment='', kind='code'):
    lines.append({'a': addr, 'b': bs, 't': text, 'c': comment, 'k': kind})

BODY_OWNER = {}
for _r in sorted(A.ROUTINES, reverse=True):
    if _r in CODE:
        for _x in body(_r): BODY_OWNER.setdefault(_x, _r)
def who(x):
    o = BODY_OWNER.get(x, owner(x)); return LABEL.get(o, '?') if o is not None else '%04X' % x

def cyc(f):
    if f['tmin'] is None: return 'not called in the scenarios'
    if f['tmin'] == f['tmax']: return '%d T-states every call (%d calls)  [measured]' % (f['tmin'], f['calls'])
    return 'observed %d-%d T-states (%d calls)  [measured]' % (f['tmin'], f['tmax'], f['calls'])

def emit_header(a):
    if a in A.ROUTINES:
        name, hdr = A.ROUTINES[a]
        f = FACTS.get(a)
        emit(None, [], '', '', 'blank')
        emit(None, [], ';' + '-' * 63, '', 'hdr')
        txt = hdr.strip('\n').split('\n')
        emit(None, [], '; %s — %s' % (name, txt[0].strip().rstrip('.').lower() if txt and txt[0] and not txt[0].startswith(('In:', 'Out:')) else ''), '', 'hdr')
        emit(None, [], ';', '', 'hdr')
        for ln in txt[1:] if txt and txt[0] and not txt[0].startswith(('In:', 'Out:')) else txt:
            if ln.startswith('Exit:'): continue
            emit(None, [], ('; ' + ln.rstrip()) if ln.strip() else ';', '', 'hdr')
        if f:
            tag = '[static + measured: %d calls, %d/%d branches]' % (f['calls'], f['both'], f['ncond']) if f['calls'] else '[static]'
            emit(None, [], '; Changes:   %s' % f['changes'], '', 'hdr')
            if f['some']:
                emit(None, [], ';            (on some paths only: %s)' % RG.fmt_regs(set(f['some'])), '', 'hdr')
            emit(None, [], '; Preserves: %s' % f['preserves'], '', 'hdr')
            emit(None, [], ';            %s' % tag, '', 'hdr')
            if f['reads'] or f['writes']:
                emit(None, [], '; Memory:    %s%s' % (('writes ' + ', '.join(f['writes'])) if f['writes'] else '',
                     (('; ' if f['writes'] else '') + 'reads ' + ', '.join(f['reads'])) if f['reads'] else ''), '', 'hdr')
            emit(None, [], '; Stack:     %d bytes max, callees included  [static]' % f['stack'], '', 'hdr')
            emit(None, [], '; Cycles:    %s' % cyc(f), '', 'hdr')
        ex = [ln for ln in txt if ln.startswith('Exit:')]
        if ex: emit(None, [], '; ' + ex[0], '', 'hdr')
        if f and f['callees']:
            emit(None, [], '; Calls:     ' + ', '.join(LABEL.get(c, h16(c)) for c in f['callees']), '', 'xref')
        calls = sorted(set(x for x, k in xref.get(a, []) if k in ('call', 'rst')))
        jumps = sorted(set(x for x, k in xref.get(a, []) if k in ('jp', 'jr', 'djnz')))
        if calls: emit(None, [], '; Called from: ' + ', '.join('%s (%04Xh)' % (who(x), x) for x in calls), '', 'xref')
        if jumps: emit(None, [], '; Jumped to from: ' + ', '.join('%s (%04Xh)' % (who(x), x) for x in jumps), '', 'xref')
        emit(None, [], ';' + '-' * 63, '', 'hdr')
    if a in A.BLOCKS:
        emit(None, [], '', '', 'blank')
        for ln in A.BLOCKS[a].strip('\n').split('\n'): emit(None, [], '; ' + ln.rstrip(), '', 'hdr')

data_regions = sorted(A.DATA)
def data_at(a):
    for d in data_regions:
        if d[0] <= a < d[1]: return d
    return None

unclassified = []
unreached = []
numeric = []   # operands pointing into the program left as numbers (the shift test would fail)
for seg_s, seg_e in SEG:
    emit(None, [], '', '', 'blank')
    emit(seg_s, [], 'org %s' % h16(seg_s), '', 'org')
    a = seg_s
    while a < seg_e:
        emit_header(a)
        if a in CODE:
            i = CODE[a]
            if a in LABEL: emit(None, [], LABEL[a] + ':', '', 'label')
            txt = i.text
            if i.parts:
                f, v = i.parts
                role = 'addr' if (i.kind or i.mem) else 'imm'
                n = opnd_label(v, role)
                if not n and loaded(v) and v >= GAME_LO:
                    numeric.append((a, v))
                txt = f.format(n if n else h16(v))
            if i.undoc or txt.startswith('db '):
                txt = 'db ' + ','.join(h8(b) for b in i.bytes) + '\t; ' + txt
            com = A.COMMENTS.get(a, '')
            if not com and i.kind in ('call', 'jp') and i.target in A.SHORT and i.target != a:
                com = A.SHORT[i.target]
                if i.kind == 'jp': com = 'go to: ' + com
            if not com and i.kind in ('call', 'jp') and i.target in A.EXTDOC: com = A.EXTDOC[i.target]
            if profs and not prof['exec'].get(a) and a >= GAME_LO and a < 0x5300:
                unreached.append(a)
                if 'UNREACHED' not in com: com = ('UNREACHED  ' + com).rstrip()
            emit(a, i.bytes, txt, com, 'code')
            a += i.length
            continue
        d = data_at(a)
        if d is None:
            b = a + 1
            while b < seg_e and b not in CODE and data_at(b) is None and b not in A.ROUTINES: b += 1
            d = (a, b, 'bytes', None, {})
            unclassified.append((a, b))
        start, end, kind, name, opts = d
        end = min(end, seg_e)
        if a in LABEL: emit(None, [], LABEL[a] + ':', '', 'label')
        for s, e, note in data_lines(a, end, kind, dict(opts, _start=start)):
            for k in range(s + 1, e):
                if k in LABEL and k not in A.VARS: e = k; break
            if s != a and s in LABEL: emit(None, [], LABEL[s] + ':', '', 'label')
            bs = [mem(k) for k in range(s, e)]
            if kind == 'words':
                items = []
                for k in range(0, len(bs) - 1, 2):
                    v = bs[k] | bs[k + 1] << 8
                    n = opnd_label(v, 'addr') if opts.get('ptr', True) else None
                    items.append(n if n else h16(v))
                txt = 'dw ' + ','.join(items)
            elif kind == 'text':
                txt = 'db ' + esc_text(bs)
            else:
                txt = 'db ' + ','.join(h8(b) for b in bs)
            com = A.COMMENTS.get(s, '') or note
            if kind == 'gfx' and not com: com = gfx_comment(bs)
            emit(s, bs, txt, com, 'data')
            a = e
            if a < end and a in LABEL: break
for s, e in unclassified: print('UNCLASSIFIED %04X-%04X' % (s, e - 1), file=sys.stderr)
for a, v in numeric: print('NUMERIC ADDRESS at %04X: %04X' % (a, v), file=sys.stderr)

# ---------------------------------------------------------------- write .asm
asm = []
for ln in A.FILE_HEADER.strip('\n').split('\n'): asm.append('; ' + ln if ln.strip() else ';')
asm.append('')
asm.append('; ---- addresses outside the program ----')
for v, n in sorted(list(A.EXTERN.items()) + list(A.HWNAMES.items())):
    asm.append('%-16s equ %s' % (n, h16(v)) + ('\t; ' + A.EXTDOC[v] if v in A.EXTDOC else ''))
asm.append('')
asm.append('; ---- names for bytes inside the tables ----')
emitted_labels = set(L['t'][:-1] for L in lines if L['k'] == 'label')
for v, (n, sz, d) in sorted(A.VARS.items()):
    if loaded(v) and n not in emitted_labels:
        cont = None
        for s, e, nm in DATA_RANGES:
            if s <= v < e and nm != n: cont = (s, nm)
        if cont: asm.append('%-16s equ %s+%d' % (n, cont[1], v - cont[0]) + ('\t; ' + d if d else ''))
        else: print('VAR NOT PLACED %04X %s' % (v, n), file=sys.stderr)
for L in lines:
    if L['k'] in ('hdr', 'xref'): asm.append(L['t'])
    elif L['k'] == 'blank': asm.append('')
    elif L['k'] == 'label': asm.append(L['t'])
    elif L['k'] == 'org': asm.append('\t' + L['t'])
    else:
        c = ('\t; ' + L['c']) if L['c'] else ''
        asm.append('\t' + L['t'] + c)
asm.append('')
asm.append('\tend\tLOADER')
open(os.path.join(BUILD, 'flysaucers.asm'), 'w').write('\n'.join(asm) + '\n')

lst = []
for L in lines:
    if L['k'] in ('hdr', 'xref', 'blank', 'label'): lst.append(L['t'])
    elif L['k'] == 'org': lst.append('\t\t\t' + L['t'])
    else:
        bs = ' '.join('%02X' % b for b in L['b'][:4]) + ('+' if len(L['b']) > 4 else '')
        lst.append('%04X  %-12s  %-34s%s' % (L['a'], bs, L['t'], ('; ' + L['c']) if L['c'] else ''))
open(os.path.join(BUILD, 'flysaucers.lst'), 'w').write('\n'.join(lst) + '\n')
json.dump({'lines': lines, 'labels': {('%04X' % k): v for k, v in LABEL.items()},
           'routines': {('%04X' % k): v[0] for k, v in A.ROUTINES.items()}},
          open(os.path.join(BUILD, 'listing.json'), 'w'))
json.dump({('%04X' % k): v for k, v in FACTS.items()}, open(os.path.join(BUILD, 'routines.json'), 'w'), indent=1)
# unreached regions
reg = []
for a in sorted(unreached):
    if reg and CODE[reg[-1][1]].length + reg[-1][1] == a: reg[-1][1] = a
    else: reg.append([a, a])
print('lines %d, code insns %d, labels %d, routines %d, profiles %d, unreached regions %s' % (
    len(lines), len(CODE), len(LABEL), len(FACTS), len(profs),
    ' '.join('%04X-%04X' % (s, e + CODE[e].length - 1) for s, e in reg) or 'none'), file=sys.stderr)
