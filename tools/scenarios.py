"""Run every emulator scenario behind the listing's measurements and the book's figures.

Each scenario is (name, script, frames, extra fsrun options). Outputs in build/run/:
  NAME.prof   per-routine profile, register entry/exit differences, coverage
  NAME.dump   video RAM after every field (when 'dump' is set)
  NAME.keys   every key the program read
Deterministic: the same file and scripts give the same bytes (tests/test_scenarios.py).
"""
import os, subprocess, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ROOT, RUN, EMU, ORIGINAL

S = os.path.join(ROOT, 'tests', 'scripts')
SCENARIOS = [
    # name, script, fields, options
    ('play5', 'play5.txt', 9000, ['--robot', '130:1', '--dump']),
    ('demo', 'demo.txt', 3000, ['--dump']),
    ('demolong', 'demolong.txt', 16000, []),
    ('edges', 'edges.txt', 700, ['--dump']),
    ('shifted', 'shifted.txt', 420, []),
    ('hard', 'hard.txt', 14000, ['--robot', '130:7']),
    ('easy', 'easy.txt', 24000, ['--robot', '130:3']),
    ('miss', 'miss.txt', 400, ['--pokeat', '125:4CF6=00,4CF7=01', '--dump']),
]


def run(name, script, frames, opts, cmd=ORIGINAL):
    exe = os.path.join(EMU, 'fsrun')
    args = [exe, cmd, '--frames', str(frames), '--script', os.path.join(S, script),
            '--profile', os.path.join(RUN, name + '.prof'), '--keylog', os.path.join(RUN, name + '.keys')]
    o = list(opts)
    if '--dump' in o:
        o.remove('--dump'); args += ['--dump', os.path.join(RUN, name + '.dump')]
    args += o
    r = subprocess.run(args, capture_output=True, text=True)
    if r.returncode: sys.exit('scenario %s failed: %s' % (name, r.stderr))
    print('%-9s %s' % (name, r.stderr.strip()))


if __name__ == '__main__':
    only = sys.argv[1:]
    for sc in SCENARIOS:
        if not only or sc[0] in only:
            run(*sc)
