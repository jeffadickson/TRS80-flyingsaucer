"""Render every moment (moments/*.json) and the chapter's still figures.

For each moment:
  build/run/mom_ID.txt     its full input script (prefix + script), the source of truth
  book/fig/ID.webm, .mp4   the clip, real timing (60 fields a second), muted
  book/fig/ID.png          poster: the field after the moment
  moments/ID.json          gains "expect": pc, frame_hash, ram_hash at the breakpoint
                           (tests/test_moments.js checks the JS machine against it)
Stills: book/fig/steer_path.png, book/fig/saucers.png.
Deterministic: same file, same scripts, same bytes (the hashes are recorded).
"""
import os, sys, json, subprocess, shutil, hashlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ROOT, RUN, EMU, ORIGINAL, FIG, BUILD
from render import render
from PIL import Image, ImageDraw

FS = os.path.join(EMU, 'fsrun')
MOM = os.path.join(ROOT, 'moments')
SCR = os.path.join(ROOT, 'tests', 'scripts')
LABELS = {v: int(k, 16) for k, v in json.load(open(os.path.join(BUILD, 'listing.json')))['labels'].items()}
CW, CH = 12, 36          # one character cell after render(): 6 dots x 2, 12 lines x 3


def script_for(m):
    lines = []
    if m.get('prefix'): lines.append(m['prefix'])
    lines += [l for l in open(os.path.join(SCR, m['script'])).read().splitlines() if l.strip() and not l.startswith('#')]
    lines.sort(key=lambda l: int(l.split()[0]))
    p = os.path.join(RUN, 'mom_%s.txt' % m['id'])
    open(p, 'w').write('\n'.join(lines) + '\n')
    return p


def frames_of(dump):
    d = open(dump, 'rb').read()
    return [d[i:i + 1024] for i in range(0, len(d), 1024)]


def crop(img, c):
    c0, r0, c1, r1 = c
    return img.crop((c0 * CW, r0 * CH, c1 * CW, r1 * CH))


def encode(pngdir, out_base):
    common = ['ffmpeg', '-y', '-loglevel', 'error', '-framerate', '60', '-i', os.path.join(pngdir, '%05d.png')]
    subprocess.run(common + ['-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', '40', '-pix_fmt', 'yuv420p', '-an', out_base + '.webm'], check=True)
    subprocess.run(common + ['-c:v', 'libx264', '-preset', 'slow', '-crf', '28', '-pix_fmt', 'yuv420p',
                             '-vf', 'pad=ceil(iw/2)*2:ceil(ih/2)*2', '-movflags', '+faststart', '-an', out_base + '.mp4'], check=True)


def moment(path):
    m = json.load(open(path))
    sc = script_for(m)
    a, b = m['clip']
    dump = os.path.join(RUN, 'mom_%s.dump' % m['id'])
    subprocess.run([FS, ORIGINAL, '--frames', str(b + 1), '--script', sc, '--dump', dump], check=True, capture_output=True)
    fr = frames_of(dump)
    tmp = os.path.join(RUN, 'png_' + m['id'])
    shutil.rmtree(tmp, ignore_errors=True); os.makedirs(tmp)
    for k, f in enumerate(range(a, b + 1)):
        crop(render(fr[f]), m['crop']).save(os.path.join(tmp, '%05d.png' % k))
    encode(tmp, os.path.join(FIG, m['id']))
    crop(render(fr[min(m['frame'] + 1, b)]), m['crop']).save(os.path.join(FIG, m['id'] + '.png'))
    bp = LABELS[m['breakpoint']]
    r = subprocess.run([FS, ORIGINAL, '--frames', str(m['frame'] + 1), '--script', sc, '--moment', '%d:%04X' % (m['frame'], bp)],
                       check=True, capture_output=True, text=True)
    exp = json.loads(r.stdout.strip().splitlines()[-1])
    assert int(exp['pc'], 16) == bp, (m['id'], exp)
    m['breakpoint_addr'] = '%04X' % bp
    m['expect'] = exp
    m['input_script'] = os.path.relpath(sc, ROOT)
    m['clip_bytes'] = {ext: os.path.getsize(os.path.join(FIG, m['id'] + '.' + ext)) for ext in ('webm', 'mp4', 'png')}
    json.dump(m, open(path, 'w'), indent=1)
    print('%-7s fields %d-%d  bp %s at field %d  webm %d B  mp4 %d B' % (m['id'], a, b, m['breakpoint'], m['frame'],
          m['clip_bytes']['webm'], m['clip_bytes']['mp4']))
    return fr


def steer_path(fr, m):
    """Every cell the steered shot occupied, drawn over the field it reached the top."""
    cells = []
    for f in range(m['clip'][0], m['clip'][1] + 1):
        s = fr[f]
        for i in range(64, 13 * 64):
            if s[i] == 0x30 and (not cells or cells[-1][1] != i): cells.append((f, i))
    last = max(f for f, _ in cells)
    base = render(fr[last]).convert('RGB')
    dr = ImageDraw.Draw(base)
    for k, (f, i) in enumerate(cells):
        r, c = divmod(i, 64)
        x, y = c * CW + CW // 2, r * CH + CH // 2
        dr.ellipse((x - 5, y - 5, x + 5, y + 5), outline=(255, 190, 60), width=2)
        if k:
            pr, pc = divmod(cells[k - 1][1], 64)
            dr.line((pc * CW + CW // 2, pr * CH + CH // 2, x, y), fill=(255, 190, 60), width=2)
    base.save(os.path.join(FIG, 'steer_path.png'))
    return [(f, divmod(i, 64)) for f, i in cells]


def saucer_sheet():
    """The three saucers and the explosion, drawn from the program's own tables."""
    from image import load
    run, _, _ = load()
    rows = []
    for t in range(3):
        top = [run[0x4DE1 + 7 * t + k] for k in range(6)]
        la = [run[0x4DF6 + 7 * t + k] for k in range(6)]
        lb = [run[0x4E0B + 7 * t + k] for k in range(6)]
        rows.append((top, la, lb))
    scr = bytearray([0x20] * 1024)
    for t, (top, la, lb) in enumerate(rows):
        for k in range(6):
            scr[(1 + 3 * t) * 64 + 2 + k] = top[k]; scr[(2 + 3 * t) * 64 + 2 + k] = la[k]
            scr[(1 + 3 * t) * 64 + 12 + k] = top[k]; scr[(2 + 3 * t) * 64 + 12 + k] = lb[k]
    for p in range(3):          # explosion pictures, BOOM_GFX: 3 x 3 strings of 5
        for row in range(3):
            for k in range(5):
                scr[(1 + row) * 64 + 24 + 8 * p + k] = run[0x4E3E + 18 * p + 6 * row + k]
    img = render(bytes(scr)).crop((0, 0, 48 * CW, 9 * CH))
    img.save(os.path.join(FIG, 'saucers.png'))


if __name__ == '__main__':
    os.makedirs(FIG, exist_ok=True)
    only = sys.argv[1:]
    for name in sorted(os.listdir(MOM)):
        if not name.endswith('.json'): continue
        mid = name[:-5]
        if only and mid not in only: continue
        fr = moment(os.path.join(MOM, name))
        if mid == 'steer':
            path = steer_path(fr, json.load(open(os.path.join(MOM, name))))
            print('steer path', path)
    saucer_sheet()
    print('stills: steer_path.png saucers.png')
