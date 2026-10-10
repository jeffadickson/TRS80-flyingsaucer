"""Flatten a /CMD file into a binary image: cmd2bin.py IN.CMD OUT.BIN -> prints origin and xfer."""
import sys
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from cmdparse import parse, image

d = open(sys.argv[1], 'rb').read()
recs, x, _ = parse(d)
mem = image(recs)
lo, hi = min(mem), max(mem)
open(sys.argv[2], 'wb').write(bytes(mem.get(a, 0) for a in range(lo, hi + 1)))
print(f"{lo:04X} {hi:04X} {x:04X}")
