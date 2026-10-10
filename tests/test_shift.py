#!/usr/bin/env python3
"""Shift test: assemble the listing with the game 100h higher and check that it
shows exactly the same screens as the original, field for field, for scenarios
that need no help from outside the program (the capture robot reads fixed
addresses, so robot scenarios are not used here). Catches any address left as
a number in the listing."""
import os, subprocess, sys
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
sys.path.insert(0, os.path.join(ROOT, 'tools'))
from paths import BUILD, RUN, EMU, ORIGINAL
src = open(os.path.join(BUILD, 'flysaucers.asm')).read().replace('\torg 42EAh', '\torg 43EAh')
open(os.path.join(BUILD, 'shift.asm'), 'w').write(src)
subprocess.run(['pasmo', '--bin', os.path.join(BUILD, 'shift.asm'), os.path.join(BUILD, 'shift.bin')], check=True)
img = open(os.path.join(BUILD, 'shift.bin'), 'rb').read(); base = 0x43EA
body = img[0x5300 - base:0x531B - base] + img[0:0x500E - 0x42EA]
out = bytearray()
for k in range(0, len(body), 256):
    p = body[k:k + 256]; a = 0x5300 + k; out += bytes([1, (len(p) + 2) & 0xFF, a & 0xFF, a >> 8]) + p
out += bytes([2, 2, 0, 0x53])
open(os.path.join(BUILD, 'SHIFT.CMD'), 'wb').write(out)
fs = os.path.join(EMU, 'fsrun'); bad = 0
for sc in ('demo', 'edges'):
    dumps = []
    for cmd in (os.path.join(BUILD, 'SHIFT.CMD'), ORIGINAL):
        d = os.path.join(RUN, 'shift_%s_%d.dump' % (sc, len(dumps)))
        subprocess.run([fs, cmd, '--frames', '6000', '--script', os.path.join(ROOT, 'tests', 'scripts', sc + '.txt'), '--dump', d],
                       check=True, capture_output=True)
        dumps.append(open(d, 'rb').read())
    same = dumps[0] == dumps[1]
    print('shift: %s, 6000 fields: %s' % (sc, 'identical' if same else 'DIFFERENT'))
    bad += not same
sys.exit(1 if bad else 0)
