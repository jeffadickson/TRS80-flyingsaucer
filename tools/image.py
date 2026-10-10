"""The program as it runs: the /CMD file's load image, with the game moved
down from 531Bh to 42EAh the way the loader stub does it.

run_mem(a)  byte at run address a (42EAh-500Dh: the game; 5300h-531Ah: the stub)
"""
import hashlib, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ORIGINAL, GAME_LO, GAME_HI, LOADER_LO, LOADER_HI, IMAGE_OFFSET

# SHA-256 of the copy this study annotates (Ira Goldklang's archive, file
# flysauc5.cmd); `make listing` refuses any other file.
ORIGINAL_SHA256 = '0f27a03a19960b9407a059e031d289234feaecbeb33f1341c080db749e044d38'
ORIGINAL_SIZE = 3451


def parse(data):
    """Return (list of (addr, bytes) load records, transfer address)."""
    i, recs, xfer = 0, [], None
    while i < len(data):
        t, n = data[i], data[i + 1]
        if t == 1:
            ln = n if n > 2 else n + 256
            recs.append((data[i + 2] | data[i + 3] << 8, bytes(data[i + 4:i + 2 + ln])))
            i += 2 + ln
        elif t == 2:
            xfer = data[i + 2] | data[i + 3] << 8
            break
        else:
            i += 2 + n
    return recs, xfer


def load(path=ORIGINAL, check=True):
    data = open(path, 'rb').read()
    if check:
        h = hashlib.sha256(data).hexdigest()
        if h != ORIGINAL_SHA256:
            sys.exit('image: %s is not the expected flysauc5.cmd (sha256 %s)' % (path, h))
    recs, xfer = parse(data)
    file_mem = {}
    for a, b in recs:
        for k, v in enumerate(b):
            file_mem[a + k] = v
    run = {}
    for a in range(GAME_LO, GAME_HI):
        run[a] = file_mem[a + IMAGE_OFFSET]
    for a in range(LOADER_LO, LOADER_HI):
        run[a] = file_mem[a]
    return run, file_mem, xfer


if __name__ == '__main__':
    run, fm, x = load()
    print('game %04X-%04X, stub %04X-%04X, transfer %04X, %d file bytes' % (GAME_LO, GAME_HI - 1, LOADER_LO, LOADER_HI - 1, x, len(fm)))
