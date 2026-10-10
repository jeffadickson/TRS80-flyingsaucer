"""Build the chapter's web edition: web/out/index.html plus its files.

Numbers come from book/figures.json, code excerpts from build/listing.json (the
listing generated from the reader's file), moments from moments/*.json, clips
and stills from book/fig. Nothing is retyped. Open mode (RIGHTS.md): the
program file is published next to the page for the moment player.
"""
import os, sys, json, re, shutil, subprocess, html, datetime
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import ROOT, BUILD, BOOK, FIG, RUN

OUT = os.path.join(ROOT, 'web', 'out')
F = json.load(open(os.path.join(BOOK, 'figures.json')))
L = json.load(open(os.path.join(BUILD, 'listing.json')))
LINES = L['lines']
MOM = {}
for n in sorted(os.listdir(os.path.join(ROOT, 'moments'))):
    if n.endswith('.json'):
        m = json.load(open(os.path.join(ROOT, 'moments', n))); MOM[m['id']] = m


def fig(path):
    v = F
    for k in path.split('.'): v = v[k]
    return v


def num(path, fmt='{:,}'):
    v = fig(path)
    return fmt.format(v)


def excerpt(lo, hi, note=''):
    """Lines of the listing from lo to hi (inclusive), labels kept, headers dropped."""
    out = []
    started = False
    for i, ln in enumerate(LINES):
        a = ln['a']
        if ln['k'] == 'label':
            nxt = next((x for x in LINES[i + 1:] if x['a'] is not None), None)
            if nxt and lo <= nxt['a'] <= hi: out.append(('label', ln['t']))
            continue
        if ln['k'] not in ('code', 'data') or a is None: continue
        if lo <= a <= hi:
            out.append(('code', a, ' '.join('%02X' % b for b in ln['b']), ln['t'], ln['c']))
    assert 0 < len(out) <= 25, (hex(lo), len(out))
    rows = []
    for r in out:
        if r[0] == 'label':
            rows.append('<span class="lb">%s</span>' % html.escape(r[1]))
        else:
            _, a, bs, t, c = r
            t = html.escape(t)
            c = (' <span class="cm">; %s</span>' % html.escape(c)) if c else ''
            rows.append('<span class="ad">%04X</span>  <span class="by">%-9s</span> <span class="op">%s</span>%s' % (a, bs, t, c))
    cap = ('<figcaption>%s</figcaption>' % note) if note else ''
    return '<figure class="code"><pre><code>%s</code></pre>%s</figure>' % ('\n'.join(rows), cap)


def clip(mid, extra=''):
    m = MOM[mid]
    alt = html.escape(m['caption'])
    return '''<figure class="moment" id="m-%(id)s">
  <div class="crt"><video muted loop playsinline preload="metadata" poster="fig/%(id)s.png" aria-label="%(alt)s" data-clip>
    <source src="fig/%(id)s.webm" type="video/webm"><source src="fig/%(id)s.mp4" type="video/mp4"></video></div>
  <figcaption><span class="mt">%(title)s</span> %(cap)s%(extra)s</figcaption>
  <div class="mrow"><button type="button" class="run" data-moment="%(id)s">Run this moment</button>
  <button type="button" class="pz" data-pause aria-pressed="false">Pause clip</button>
  <span class="mloc">stops at %(bp)s in field %(frame)d</span></div>
</figure>''' % dict(id=mid, alt=alt, title=html.escape(m['title']), cap=html.escape(m['caption']), extra=extra,
                    frame=m['frame'] + 1, bp=m['breakpoint'])


def steer_trace():
    """The steered shot, cell by cell, from the capture of the steer moment."""
    d = open(os.path.join(RUN, 'mom_steer.dump'), 'rb').read()
    fr = [d[i:i + 1024] for i in range(0, len(d), 1024)]
    cells = []
    for f in range(MOM['steer']['clip'][0], MOM['steer']['clip'][1] + 1):
        for i in range(64, 13 * 64):
            if fr[f][i] == 0x30 and (not cells or cells[-1][1] != i): cells.append((f, i))
    rows = []
    for k, (f, i) in enumerate(cells):
        r, c = divmod(i, 64)
        if k:
            pr, pc = divmod(cells[k - 1][1], 64)
            step = i - cells[k - 1][1]
            aim = 3 + (c - pc)
            rows.append('<tr><td>%d</td><td>%d, %d</td><td>%+d</td><td>%d</td></tr>' % (f, r, c, step, aim))
        else:
            rows.append('<tr><td>%d</td><td>%d, %d</td><td>leaves the barrel</td><td>3</td></tr>' % (f, r, c))
    return ''.join(rows), len(cells)


trace_rows, trace_n = steer_trace()
built = datetime.date.today().isoformat()
try:
    commit = subprocess.run(['git', '-C', ROOT, 'rev-parse', '--short', 'HEAD'], capture_output=True, text=True).stdout.strip() or 'uncommitted'
except Exception:
    commit = 'uncommitted'

speed_rows = ''
for d in (1, 5, 9):
    k = 'd%d' % d
    speed_rows += ('<tr><th scope="row">%d</th><td>%d</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>' % (
        d, fig(k + '.pace'), num(k + '.saucer_step_ms', '{:.0f} ms'), num(k + '.small_cells_per_s', '{:.1f}'),
        num(k + '.big_cells_per_s', '{:.1f}'), num(k + '.shot_cells_per_s', '{:.1f}'), num(k + '.time_unit_s', '{:.2f} s')))

T = open(os.path.join(ROOT, 'web', 'src', 'chapter.html')).read()

def sub(m):
    key = m.group(1)
    if key.startswith('fig:'):
        p, _, f = key[4:].partition('|')
        return num(p, '{}' if f == 's' else ('{:' + f + '}' if f else '{:,}'))
    if key.startswith('clip:'): return clip(key[5:])
    if key.startswith('code:'):
        lo, hi, *note = key[5:].split(':', 2)
        return excerpt(int(lo, 16), int(hi, 16), note[0] if note else '')
    return {'speed_rows': speed_rows, 'trace_rows': trace_rows, 'trace_n': str(trace_n), 'built': built, 'commit': commit,
            'moments_json': json.dumps({k: dict(title=v['title'], frame=v['frame'], breakpoint=v['breakpoint'],
                                                breakpoint_addr=v['breakpoint_addr'], events=events(v)) for k, v in MOM.items()},
                                       separators=(',', ':')),
            'font_json': open(os.path.join(ROOT, 'web', 'font.json')).read().strip()}[key]


def events(m):
    ev = []
    for line in open(os.path.join(ROOT, m['input_script'])):
        t = line.split()
        if not t or t[0].startswith('#'): continue
        f = int(t[0])
        if t[1] == 'down': ev.append([f, 1, t[2]])
        elif t[1] == 'up': ev.append([f, 0, t[2]])
        else:
            dur = int(t[3]) if len(t) > 3 else 3
            ev += [[f, 1, t[2]], [f + dur, 0, t[2]]]
    return ev

page = re.sub(r'\{\{([^}]+)\}\}', sub, T)
shutil.rmtree(OUT, ignore_errors=True); os.makedirs(os.path.join(OUT, 'fig'))
open(os.path.join(OUT, 'index.html'), 'w').write(page)
for f in os.listdir(FIG): shutil.copy(os.path.join(FIG, f), os.path.join(OUT, 'fig', f))
for f in ('trs80.js', 'saucsys.js'): shutil.copy(os.path.join(ROOT, 'emulator', 'js', f), os.path.join(OUT, f))
shutil.copy(os.path.join(ROOT, 'web', 'src', 'player.js'), os.path.join(OUT, 'player.js'))
shutil.copy(os.path.join(ROOT, 'original', 'flysauc5.cmd'), os.path.join(OUT, 'flysauc5.cmd'))
tot = sum(os.path.getsize(os.path.join(dp, f)) for dp, _, fs in os.walk(OUT) for f in fs)
print('web: %s  %d bytes in %d files' % (OUT, tot, sum(len(fs) for _, _, fs in os.walk(OUT))))
