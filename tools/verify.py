#!/usr/bin/env python3
"""Check that the rebuilt /CMD is byte-for-byte flysauc5.cmd (by its recorded SHA-256),
and, if the reader's own copy is at original/flysauc5.cmd, against that too.

usage: verify.py BUILT.CMD [ORIGINAL.CMD]
"""
import hashlib, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from image import ORIGINAL_SHA256, ORIGINAL_SIZE


def main():
    built = open(sys.argv[1], 'rb').read()
    digest = hashlib.sha256(built).hexdigest()
    ok = digest == ORIGINAL_SHA256 and len(built) == ORIGINAL_SIZE
    print('verify: %s  %d bytes  sha256 %s  %s' % (sys.argv[1], len(built), digest, 'OK' if ok else 'MISMATCH'))
    if len(sys.argv) > 2 and os.path.exists(sys.argv[2]):
        orig = open(sys.argv[2], 'rb').read()
        if orig != built:
            diff = next((i for i in range(min(len(orig), len(built))) if orig[i] != built[i]), min(len(orig), len(built)))
            print('verify: differs from %s at offset %04Xh' % (sys.argv[2], diff)); ok = False
        else:
            print('verify: identical to %s' % sys.argv[2])
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
