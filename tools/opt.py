"""Build the 'Could it have run faster?' variants from patched source and check them.

  python3 tools/opt.py

Variants (each a text patch of src/flysaucers.asm, assembled with pasmo and
packaged like the original; every other byte keeps its address, padding with
DS where code got shorter, so RAM can be compared pass for pass):

  P1 random   RANDOM's 23-step loop with RR H / RR L / RES 0,L and DJNZ:
              the same numbers (10,000 random seeds through callrun), fewer T-states
  P2 counts   SHOW_COUNTS only when SCORE, HITS or SHOTS has changed (or the
              screen has been cleared); room from the dead -20 branch at 4A7Eh
              and the always-false row-0 test in front of it
  P12         both

Checks: the demonstration, started with D and left alone, runs pass for pass
with the same screen and the same data (4C4Bh-4FE6h) as the original; P1 and
P2 change nothing a player sees except the time each pass takes. Results go to
book/opt.json.
"""
import os, sys, json, subprocess, random, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ROOT, BUILD, RUN, EMU, ORIGINAL, BOOK

SRC = open(os.path.join(ROOT, 'src', 'flysaucers.asm')).read()
FS = os.path.join(EMU, 'fsrun')

P1 = [("""	ld b,17h	; 23 steps
	ld a,l
RANDOM_STEP:
	and 06h	; bits 1 and 2 of the low byte
	scf	; carry = 1 ...
	jp pe,RANDOM_SHIFT	; ... if they are equal (parity even) ...
	ccf	; ... else 0: the XOR of the two, inverted
RANDOM_SHIFT:
	ld a,h	; shift the 16 bits right, carry into bit 15
	rra
	ld h,a
	ld a,l
	rra
	and 0FEh	; bit 0 dropped
	ld l,a
	dec b
	jp nz,RANDOM_STEP
""", """	ld b,17h	; 23 steps
RANDOM_STEP:
	ld a,l	; P1: the same step, shorter
	and 06h
	scf
	jp pe,RANDOM_SHIFT
	ccf
RANDOM_SHIFT:
	rr h	; P1: RR on the registers themselves
	rr l
	res 0,l
	djnz RANDOM_STEP
"""), ("""	pop bc
	ret
SPARE:""", """	pop bc
	ret
	ds 4	; P1: the 4 bytes saved, so nothing after moves
SPARE:""")]

P2 = [("""	call SHOW_COUNTS	; print the counts""", """	call SHOW_IF	; P2: print the counts only if they changed"""),
      ("""	cp 20h	;   blank? yes: just move
""", None)]  # placeholder, replaced below


def p2_patches(src):
    # LOOK_AHEAD jumps to MOVE_INTO on a blank; go straight to MOVE_CHECK instead
    a = src.index('MOVE_INTO:\n')
    b = src.index('FRAGMENT_GONE:\n')
    old = src[a:b]
    nbytes = 13 + 35       # the row-0 test (never true) and the dead -20 branch
    new = """MOVE_INTO:	; P2: the never-true row-0 test and the dead -20 branch are gone
	jp MOVE_CHECK
SHOW_IF:	; P2: SHOW_COUNTS only when something changed
	ld a,(VIDEO+957)	; the score's last digit: blank after the screen was cleared?
	cp 20h
	jp z,SHOW_COUNTS
	ld hl,SPARE	; last values shown: SCORE, HIT_COUNT, SHOT_COUNT (low bytes)
	ld a,(SCORE)
	ld c,(hl)
	ld (hl),a
	xor c
	ld b,a
	inc hl
	ld a,(HIT_COUNT)
	ld c,(hl)
	ld (hl),a
	xor c
	or b
	ld b,a
	inc hl
	ld a,(SHOT_COUNT)
	ld c,(hl)
	ld (hl),a
	xor c
	or b
	ret z	; nothing changed: nothing to print
	jp SHOW_COUNTS
	ds %d	; P2: padding, so nothing after moves
""" % (nbytes - 3 - 40)
    return src[:a] + new + src[b:]


def build(name, src):
    asm = os.path.join(BUILD, 'opt_%s.asm' % name); binf = asm[:-4] + '.bin'; cmd = os.path.join(BUILD, 'OPT_%s.CMD' % name.upper())
    open(asm, 'w').write(src)
    r = subprocess.run(['pasmo', '--bin', asm, binf], capture_output=True, text=True)
    if r.returncode: sys.exit('pasmo %s: %s %s' % (name, r.stdout, r.stderr))
    subprocess.run([sys.executable, '-I', os.path.join(ROOT, 'tools', 'mkcmd.py'), binf, cmd], check=True, capture_output=True)
    return cmd, open(binf, 'rb').read()


def apply(src, patches):
    for a, b in patches:
        assert src.count(a) == 1, a[:40]
        src = src.replace(a, b)
    return src


CHANGED = {}
orig_bin = open(os.path.join(BUILD, 'flysaucers.bin'), 'rb').read()
v = {}
v['random'] = build('random', apply(SRC, P1))
v['counts'] = build('counts', p2_patches(apply(SRC, P2[:1])))
v['both'] = build('both', p2_patches(apply(apply(SRC, P1), P2[:1])))
for k, (cmd, b) in v.items():
    assert len(b) == len(orig_bin), (k, len(b))
    diff = [i for i in range(len(b)) if b[i] != orig_bin[i]]
    print('%-7s %d bytes changed, %04Xh-%04Xh' % (k, len(diff), 0x42EA + diff[0], 0x42EA + diff[-1]))
    CHANGED[k] = len(diff)

# P1: 10,000 random seeds, RANDOM in the original and in P1
rng = random.Random(1979)
cases = []
for _ in range(10000):
    s = rng.randrange(65536)
    cases.append('4C26 00 00 %02X %02X %02X %02X %02X %02X 00 00 00 00 ; 4C4C=%02X%02X ; 4C4C:2' % (
        rng.randrange(256), rng.randrange(256), rng.randrange(256), rng.randrange(256), rng.randrange(256), rng.randrange(256), s & 255, s >> 8))
inp = '\n'.join(cases) + '\n'
res = {}
for k, cmd in (('orig', ORIGINAL), ('random', v['random'][0])):
    out = subprocess.run([os.path.join(EMU, 'callrun'), cmd], input=inp, capture_output=True, text=True, check=True).stdout.splitlines()
    res[k] = out
same = 0; tsave = []
for a, b in zip(res['orig'], res['random']):
    ta, tb = a.split(), b.split()
    # A, B, C, D, E, H, L, IX, IY and the new SEED must agree; F may differ (callers AND it at once)
    if ta[0] == tb[0] and ta[2:10] == tb[2:10] and a.split(';')[1] == b.split(';')[1]: same += 1
    tsave.append(int(ta[10]) - int(tb[10]))
print('P1 port test: %d / 10000 identical; saves %d T-states a call' % (same, st.median(tsave)))

# all: the demonstration, pass for pass
script = os.path.join(RUN, 'opt_demo.txt'); open(script, 'w').write('120 tap D\n')
logs = {}
for k, cmd in [('orig', ORIGINAL)] + [(k, c) for k, (c, _) in v.items()]:
    lg = os.path.join(RUN, 'pass_%s.txt' % k)
    subprocess.run([FS, cmd, '--frames', '7200', '--script', script, '--passlog', lg], check=True, capture_output=True)
    logs[k] = [l.split() for l in open(lg)]
out = {'port_cases': 10000, 'port_identical': same, 'random_saving_t': int(st.median(tsave))}
n = min(len(x) for x in logs.values())
for k in v:
    a, b = logs['orig'][:n], logs[k][:n]
    ident = sum(1 for x, y in zip(a, b) if x[1:] == y[1:])
    first = next((i for i, (x, y) in enumerate(zip(a, b)) if x[1:] != y[1:]), None)
    pa = [int(y[0]) - int(x[0]) for x, y in zip(a, a[1:])]
    pb = [int(y[0]) - int(x[0]) for x, y in zip(b, b[1:])]
    out[k] = {'passes_compared': n, 'identical_passes': ident, 'first_difference': first,
              'pass_t_median_orig': int(st.median(pa)), 'pass_t_median': int(st.median(pb)),
              'saving_t': int(st.median(pa)) - int(st.median(pb)),
              'saving_pct': round(100 * (st.median(pa) - st.median(pb)) / st.median(pa), 1),
              'game_t_orig': int(a[-1][0]) - int(a[0][0]), 'game_t': int(b[-1][0]) - int(b[0][0]),
              'bytes_changed': CHANGED[k]}
    print('%-7s %d/%d passes identical; median pass %d -> %d T (%.1f%%)' % (k, ident, n, out[k]['pass_t_median_orig'], out[k]['pass_t_median'], out[k]['saving_pct']))
json.dump(out, open(os.path.join(BOOK, 'opt.json'), 'w'), indent=1)
