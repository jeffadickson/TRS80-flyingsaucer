"""Parse TRS-80 /CMD load-module files: records, load map, transfer address."""
import sys, hashlib

def parse(data):
    i, recs, xfer, notes = 0, [], None, []
    while i < len(data):
        t = data[i]
        if i + 1 >= len(data):
            notes.append(f"trailing byte {t:02X} at {i}"); break
        n = data[i + 1]
        if t == 0x01:
            ln = n if n > 2 else n + 256
            addr = data[i + 2] | data[i + 3] << 8
            body = data[i + 4:i + 2 + ln]
            recs.append((addr, bytes(body), i))
            i += 2 + ln
        elif t == 0x02:
            xfer = data[i + 2] | data[i + 3] << 8
            i += 2 + n
            if i < len(data):
                notes.append(f"{len(data) - i} bytes after transfer record (padding?)")
            break
        elif t == 0x05:
            notes.append(f"header '{data[i+2:i+2+n].decode('latin1')}'"); i += 2 + n
        elif t == 0x1F:
            notes.append(f"copyright block len {n}"); i += 2 + n
        else:
            notes.append(f"unknown record {t:02X} at {i}"); break
    return recs, xfer, notes

def image(recs):
    mem = {}
    for a, b, _ in recs:
        for k, v in enumerate(b):
            mem[(a + k) & 0xFFFF] = v
    return mem

def ranges(mem):
    ks = sorted(mem); out = []; s = p = None
    for k in ks:
        if s is None: s = p = k
        elif k == p + 1: p = k
        else: out.append((s, p)); s = p = k
    if s is not None: out.append((s, p))
    return out

if __name__ == "__main__":
    for f in sys.argv[1:]:
        d = open(f, "rb").read()
        recs, x, notes = parse(d)
        mem = image(recs)
        lo, hi = min(mem), max(mem)
        blob = bytes(mem.get(a, 0) for a in range(lo, hi + 1))
        print(f"{f.split('/')[-1]}: {len(recs)} recs, xfer={x:04X}h, "
              f"ranges={[f'{a:04X}-{b:04X}' for a, b in ranges(mem)]}, "
              f"{len(mem)} bytes, img sha={hashlib.sha256(blob).hexdigest()[:16]}")
        for n in notes: print("   ", n)
