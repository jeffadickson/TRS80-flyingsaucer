"""Measure the program on the emulator and write book/figures.json.

Every number the book quotes comes from this file; the prose never retypes one.
Times are T-states of the Model I's 1.77408 MHz clock (the stock machine has no
wait states on memory the CPU uses: video RAM, RAM and ROM all run at full
speed, so elapsed time is the instruction-table count; the one exception is the
substitute ROM keyboard routine, charged a flat 410 T-states a call, which is
an estimate: see the Tools chapter).
"""
import os, sys, json, subprocess, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'annotations'))
from paths import ROOT, RUN, EMU, ORIGINAL, BOOK
from image import load
import regs as RG

HZ = 1774080
FS = os.path.join(EMU, 'fsrun')
S = os.path.join(ROOT, 'tests', 'scripts')
fig = {}


def fsrun(script, frames, extra):
    r = subprocess.run([FS, ORIGINAL, '--frames', str(frames), '--script', os.path.join(S, script)] + extra,
                       capture_output=True, text=True)
    if r.returncode: sys.exit(r.stderr)


def hits(script, frames, pcs, robot=None):
    out = os.path.join(RUN, 'm_hits.txt')
    extra = ['--hits', ','.join('%04X' % p for p in pcs) + ':' + out]
    if robot: extra += ['--robot', robot]
    fsrun(script, frames, extra)
    ev = []
    for ln in open(out):
        t = ln.split(); ev.append((int(t[0]), int(t[1]), int(t[2], 16), t[3:]))
    return ev


def ms(t): return round(t * 1000 / HZ, 2)
def sec(t): return round(t / HZ, 2)

# ---- the program
run, filemem, xfer = load()
fig['file_bytes'] = 3451
fig['game_bytes'] = 0x4FFB - 0x42EA
fig['code_bytes'] = 0x4C48 - 0x42EA
fig['data_bytes'] = 0x4FFB - 0x4C48
fig['ram_top'] = '4FFFh'
fig['stack_bytes'] = 0x4FF9 - 0x4FE7
an = RG.Analyzer(lambda a: run.get(a & 0xFFFF, 0xFF), {0x002B: {'A', 'D', 'E'} | set(RG.FLAGS)})
fig['stack_max_static'] = an.analyze(0x42F9).stack     # TITLE onwards: the whole game

# ---- profile facts (from the scenario runs)
R = {}
for ln in open(os.path.join(RUN, 'play5.prof')):
    t = ln.split()
    if t and t[0] == 'R': R[int(t[1], 16)] = dict(excl=int(t[3]), calls=int(t[5]), incl=int(t[7]), mx=int(t[9]), mn=int(t[11]))
fig['delay_t'] = R[0x4C10]['mn']
assert R[0x4C10]['mn'] == R[0x4C10]['mx']
fig['delay_ms'] = ms(fig['delay_t'])
fig['random_t'] = R[0x4C26]['mn']

# ---- one pass of the main loop, by difficulty (game time only)
MAIN = 0x4404
for d, script, frames, robot in ((1, 'hard.txt', 14000, '130:7'), (5, 'play5.txt', 8400, '130:1'), (9, 'easy.txt', 24000, '130:3')):
    ev = hits(script, frames, [MAIN, 0x4BB9, 0x464A, 0x47D0, 0x457D, 0x4626, 0x4935, 0x497D, 0x49D2, 0x4451, 0x447D], robot)
    tops = [c for f, c, pc, _ in ev if pc == MAIN]
    passes = [b - a for a, b in zip(tops, tops[1:])]
    over = [c for f, c, pc, _ in ev if pc == 0x4451]
    if over: passes = [b - a for a, b in zip(tops, tops[1:]) if b < over[0]]
    plain = sorted(passes)
    clock = [c for f, c, pc, _ in ev if pc == 0x4BB9]
    sauc = [c for f, c, pc, _ in ev if pc == 0x464A]
    shot = [c for f, c, pc, _ in ev if pc == 0x47D0]
    spawn = sum(1 for f, c, pc, _ in ev if pc in (0x457D, 0x4626) and (not over or c < over[0]))
    k = 'd%d' % d
    fig[k] = {
        'pace': 2 * d + 4, 'clock_passes': 2 * d + 4 + 204, 'shot_passes': d + 1,
        'pass_t_median': int(st.median(plain)), 'pass_t_min': plain[0],
        'pass_ms_median': ms(st.median(plain)),
        'time_unit_s': sec(st.median([b - a for a, b in zip(clock, clock[1:])])),
        'saucer_step_ms': ms(st.median([b - a for a, b in zip(sauc, sauc[1:])])),
        'shot_step_ms': ms(st.median([b - a for a, b in zip(shot, shot[1:])])),
        'game_s': sec(over[0] - tops[0]) if over else None,
        'extra_time_given': any(pc == 0x447D for f, c, pc, _ in ev),
        'saucers_started': spawn,
        'passes': len(passes),
    }
    fig[k]['shot_cells_per_s'] = round(1000 / (2 * fig[k]['shot_step_ms']), 1)
    fig[k]['small_cells_per_s'] = round(1000 / fig[k]['saucer_step_ms'], 1)
    fig[k]['big_cells_per_s'] = round(1000 / (2 * fig[k]['saucer_step_ms']), 1)
    booms = [(c, pc) for f, c, pc, _ in ev if pc in (0x4935, 0x497D, 0x49D2)]
    d_boom = [b[0] - a[0] for a, b in zip(booms, booms[1:]) if a[1] == 0x4935 and b[1] in (0x497D, 0x49D2)]
    if d_boom: fig[k]['boom_ms'] = ms(st.median(d_boom))
    fig[k]['delay_share_pct'] = round(100 * fig['delay_t'] / st.median(plain), 1)

# ---- where a typical pass goes (difficulty 5)
tot = sum(v['excl'] for v in R.values())
def share(entry): return round(100 * R[entry]['incl'] / tot, 1) if entry in R else 0
fig['share_pct'] = {'DELAY': share(0x4C10), 'UPDATE': share(0x4510), 'SHOW_COUNTS': share(0x4B5E),
                    'TICK_CLOCK': share(0x4B80), 'GET_KEY': share(0x4C1F)}
fig['show_counts_t'] = R[0x4B5E]['mn']
Lp = sorted(int(l.split()[1]) for l in open(os.path.join(RUN, 'play5.prof')) if l.startswith('L '))
fig['pass_pcts'] = {'p50': Lp[len(Lp) // 2], 'p99': Lp[int(.99 * (len(Lp) - 1))], 'max': Lp[-1]}
fig['pass_pcts_ms'] = {k: ms(v) for k, v in fig['pass_pcts'].items()}
fig['update_max_t'] = R[0x4510]['mx']; fig['update_max_ms'] = ms(R[0x4510]['mx'])
fig['random_per_call_t'] = round(R[0x4C26]['incl'] / R[0x4C26]['calls'])
fig['random_share_pct'] = round(100 * fig['random_per_call_t'] / fig['d5']['pass_t_median'], 1)
fig['show_counts_share_pct'] = round(100 * R[0x4B5E]['mn'] / fig['d5']['pass_t_median'], 1)
fig['print_bcd_t_max'] = R[0x4BCE]['mx']


# ---- explosions that find no saucer (difficulty 5, the robot's game)
out = os.path.join(RUN, 'm_boom.txt'); dump = os.path.join(RUN, 'm_boom.dump')
fsrun('play5.txt', 8400, ['--robot', '130:1', '--hits', '48E6,4935,4894,4741,4794:' + out, '--dump', dump])
ev = [l.split() for l in open(out)]
scr = open(dump, 'rb').read()
found = 0; none = 0; dome = 0
for i, e in enumerate(ev):
    if e[2] != '4935': continue
    prev = ev[max(0, i - 3):i]
    if any(x[2] == '48E6' for x in prev): found += 1; continue
    none += 1
    look = [x for x in prev if x[2] == '4894']
    de = int(look[-1][5], 16)
    r = (de - 0x3C00) // 64; f = int(e[0])
    cell = scr[(f - 1) * 1024 + de - 0x3C00]
    if r in (2, 4, 6, 8) and 0x80 <= cell < 0xC0: dome += 1
fig['explosions'] = {'total': found + none, 'saucer_found': found, 'none_found': none, 'dome_row': dome,
                     'saucer_crashes': sum(1 for e in ev if e[2] in ('4741', '4794'))}

# ---- the escape penalty at a score of 0100 (OR L tests only the low byte)
for start, key in (('00', 'escape_from_0100'), ('05', 'escape_from_0105')):
    md = os.path.join(RUN, 'm_esc.bin'); hf = os.path.join(RUN, 'm_esc.txt')
    fsrun('escape.txt', 2200, ['--pokeat', '125:4CF6=%s,4CF7=01' % start, '--memdump', '2199:' + md, '--hits', '470F:' + hf])
    m = open(md, 'rb').read()
    fig[key] = {'escapes': sum(1 for _ in open(hf)), 'score_after': '%02X%02X' % (m[0x4CF7], m[0x4CF6])}

# ---- how full the shot table gets under rapid fire (LAUNCH's search has no end test)
hf = os.path.join(RUN, 'm_mash.txt')
fsrun('mash.txt', 10200, ['--hits', '4B47,49C1:' + hf])
recs = [(int(l.split()[6], 16) - 0x4C9C) // 3 for l in open(hf)]
fig['shot_table'] = {'records': 20, 'highest_used': max(recs) + 1, 'new_records': len(recs), 'fields': 10200}

# ---- spawn odds (from the code: 1 in 256 a pass; 2 in 256 in the demonstration)
fig['spawn_per_pass'] = '1/256'
fig['super_share'] = '1/33'

os.makedirs(BOOK, exist_ok=True)
json.dump(fig, open(os.path.join(BOOK, 'figures.json'), 'w'), indent=1)
print(json.dumps(fig, indent=1))
