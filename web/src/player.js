/* player.js -- the chapter's "Run this moment" player: Flying Saucers on the
 * series' JavaScript TRS-80 (trs80.js) with the study's substitute ROM
 * (saucsys.js). Open mode: flysauc5.cmd is published with the page, as
 * base64 in flysauc5.js, and checked by SHA-256 on the first Run. If it cannot be fetched, the
 * reader can load their own copy. Each moment replays its recorded input from
 * power-on to the moment's field and stops on the moment's breakpoint. MOMENTS and FONT are injected
 * by tools/web.py. */
(function () {
  'use strict';
  const { SaucerMachine } = window.SAUCSYS;
  const FRAME_T = SaucerMachine.FRAME_T;
  const SHA = '0f27a03a19960b9407a059e031d289234feaecbeb33f1341c080db749e044d38';
  const KEY = 'flysauc5-cmd';
  const M = window.MOMENTS, FONT = window.FONT;

  const panel = document.getElementById('emu');
  const canvas = document.getElementById('emu-crt');
  const ctx = canvas.getContext('2d');
  const titleEl = document.getElementById('emu-title');
  const statusEl = document.getElementById('emu-status');
  const loadBox = document.getElementById('emu-load');
  const fileIn = document.getElementById('emu-file');
  const loadMsg = document.getElementById('emu-load-msg');
  const runBtn = document.getElementById('emu-run');
  const stepBtn = document.getElementById('emu-step');
  const againBtn = document.getElementById('emu-again');
  const closeBtn = document.getElementById('emu-close');
  const screenWrap = document.getElementById('emu-screen');
  const DX = 2, DY = 3;
  canvas.width = 384 * DX; canvas.height = 192 * DY;

  let program = null, machine = null, current = null, running = false, field = 0, raf = 0, lastTs = 0, acc = 0;
  let last = new Int16Array(1024).fill(-1);
  let opener = null;

  // ---- the reader's file
  function fromStore() {
    try { const s = localStorage.getItem(KEY); if (s) return Uint8Array.from(atob(s), c => c.charCodeAt(0)); } catch (e) { /* no storage */ }
    return null;
  }
  function toStore(b) {
    try { let s = ''; for (const x of b) s += String.fromCharCode(x); localStorage.setItem(KEY, btoa(s)); } catch (e) { /* no storage */ }
  }
  async function sha256(b) {
    const d = await crypto.subtle.digest('SHA-256', b);
    return Array.from(new Uint8Array(d), x => x.toString(16).padStart(2, '0')).join('');
  }
  async function accept(bytes) {
    const h = await sha256(bytes);
    if (h !== SHA) {
      loadMsg.textContent = 'That file is not flysauc5.cmd: its SHA-256 starts ' + h.slice(0, 12) +
        ', the copy this book studies starts 0f27a03a1996. Choose the file from Ira Goldklang\'s archive.';
      return false;
    }
    program = bytes; toStore(bytes); loadBox.hidden = true; screenWrap.hidden = false;
    document.querySelectorAll('[data-moment]').forEach(b => { b.textContent = 'Run this moment'; });
    return true;
  }
  fileIn.addEventListener('change', async () => {
    const f = fileIn.files && fileIn.files[0]; if (!f) return;
    if (await accept(new Uint8Array(await f.arrayBuffer())) && current) start(current);
  });
  loadBox.addEventListener('dragover', e => e.preventDefault());
  loadBox.addEventListener('drop', async e => {
    e.preventDefault();
    const f = e.dataTransfer.files && e.dataTransfer.files[0]; if (!f) return;
    if (await accept(new Uint8Array(await f.arrayBuffer())) && current) start(current);
  });

  // ---- screen
  function colours() {
    const cs = getComputedStyle(document.documentElement);
    return { bg: cs.getPropertyValue('--crt-bg').trim() || '#06080c', fg: cs.getPropertyValue('--crt-fg').trim() || '#e4ecff' };
  }
  let col = colours();
  function drawCell(i, b) {
    const cx = (i & 63) * 6 * DX, cy = (i >> 6) * 12 * DY;
    ctx.fillStyle = col.bg; ctx.fillRect(cx, cy, 6 * DX, 12 * DY);
    ctx.fillStyle = col.fg;
    if (b & 0x80) {
      for (let r = 0; r < 3; r++) for (let c = 0; c < 2; c++)
        if (b & (1 << (r * 2 + c))) ctx.fillRect(cx + c * 3 * DX, cy + r * 4 * DY, 3 * DX, 4 * DY);
      return;
    }
    let code = b & 0x7F;
    if (code < 0x20) code += 0x40;
    if (code >= 0x60) code -= 0x20;                       // a stock Model I shows no lower case
    const g = FONT[code - 0x20];
    for (let r = 0; r < 9; r++) {
      const row = g[r];
      for (let c = 0; c < 5; c++) if (row & (16 >> c)) ctx.fillRect(cx + c * DX, cy + (r + 2) * DY, DX, DY);
    }
  }
  function draw(force) {
    const v = machine.shown();
    for (let i = 0; i < 1024; i++) if (force || v[i] !== last[i]) { drawCell(i, v[i]); last[i] = v[i]; }
  }

  // ---- input: the moment's recorded keys, then the reader's
  let events = [];
  function applyEvents(f) {
    for (const [ef, down, k] of events) if (ef === f) { if (down) machine.pressKey(k); else machine.releaseKey(k); }
  }
  const held = {};
  function tap(k) {
    if (!machine) return;
    machine.pressKey(k); held[k] = field + 3;
  }
  function settle() {
    for (const k in held) if (field >= held[k]) { machine.releaseKey(k); delete held[k]; }
  }

  function setStatus(t) { statusEl.textContent = t; }
  function label(m) { return m.breakpoint + ' (' + m.breakpoint_addr + 'h)'; }

  function start(id) {
    current = id; const m = M[id];
    titleEl.textContent = m.title;
    if (!program) { loadBox.hidden = false; screenWrap.hidden = true; setStatus(''); return; }
    loadBox.hidden = true; screenWrap.hidden = false;
    stop();
    events = m.events;
    machine = new SaucerMachine(); machine.boot(program);
    field = 0; last.fill(-1);
    setStatus('Replaying the recorded keys to field ' + m.frame + '…');
    const target = m.frame, bp = m.breakpoint ? parseInt(m.breakpoint_addr, 16) : -1;
    function chunk() {
      const t0 = performance.now();
      while (field <= target && performance.now() - t0 < 30) {
        applyEvents(field); machine.runTo((field + 1) * FRAME_T); field++;
      }
      if (field <= target) { setStatus('Replaying the recorded keys: field ' + field + ' of ' + target); draw(); raf = requestAnimationFrame(chunk); return; }
      field = target;
      if (!m.breakpoint) {                  // free play: straight to the title page, running
        field = target + 1; draw(true); canvas.focus(); go(); return;
      }
      for (let n = 0; machine.cpu.pc !== bp && n < 5e6; n++) machine.step();
      field = target + 1;
      draw(true);
      setStatus('Stopped at ' + label(m) + ' in field ' + (target + 1) + '. Run to go on; the keys below play.');
      runBtn.textContent = 'Run'; canvas.focus();
    }
    chunk();
  }

  function loop(ts) {
    if (!running) return;
    if (!lastTs) lastTs = ts;
    acc += Math.min(100, ts - lastTs); lastTs = ts;
    while (acc >= 1000 / 60) {
      applyEvents(field); settle();
      machine.runTo((field + 1) * FRAME_T); field++; acc -= 1000 / 60;
    }
    draw();
    raf = requestAnimationFrame(loop);
  }
  function go() { if (!machine) return; running = true; lastTs = 0; acc = 0; runBtn.textContent = 'Pause'; setStatus('Running. Keys: , and . turn the cannon, Z fires, 1-9 difficulty, R restarts.'); raf = requestAnimationFrame(loop); }
  function stop() { running = false; cancelAnimationFrame(raf); runBtn.textContent = 'Run'; }

  runBtn.addEventListener('click', () => { running ? (stop(), setStatus('Paused at field ' + field + '.')) : go(); });
  stepBtn.addEventListener('click', () => {
    if (!machine) return; stop();
    applyEvents(field); settle(); machine.runTo((field + 1) * FRAME_T); field++; draw();
    setStatus('Paused at field ' + field + '.');
  });
  againBtn.addEventListener('click', () => { if (current) start(current); });
  closeBtn.addEventListener('click', close);
  document.querySelectorAll('[data-tap]').forEach(b => b.addEventListener('click', () => { tap(b.dataset.tap); if (!running) go(); }));

  // keyboard, only while the screen has focus; Esc gives the keyboard back
  const KEYS = { ' ': 'SPACE', Enter: 'ENTER', Shift: 'SHIFT', '.': '.', ',': ',', '<': ',', '>': '.' };
  canvas.addEventListener('keydown', e => {
    if (e.key === 'Escape') { canvas.blur(); return; }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const k = KEYS[e.key] || (/^[a-z0-9]$/i.test(e.key) ? e.key.toUpperCase() : null);
    if (!k || !machine) return;
    e.preventDefault(); if (!e.repeat) { tap(k); if (!running) go(); }
  });

  async function fetchProgram() {
    try {
      if (!window.FLYSAUC5_B64) return false;
      const b = Uint8Array.from(atob(window.FLYSAUC5_B64), c => c.charCodeAt(0));
      if (await sha256(b) !== SHA) return false;
      program = b; return true;
    } catch (e) { return false; }
  }
  async function open(id, btn) {
    opener = btn; col = colours();
    panel.hidden = false; document.body.classList.add('emu-open');
    if (!program) { setStatus('Loading Flying Saucers…'); await fetchProgram(); }
    start(id);
    (program ? canvas : fileIn).focus();
  }
  function close() {
    stop(); panel.hidden = true; document.body.classList.remove('emu-open');
    if (opener) opener.focus();
  }
  panel.addEventListener('keydown', e => { if (e.key === 'Escape' && document.activeElement !== canvas) close(); });

  program = fromStore();
  document.querySelectorAll('[data-moment]').forEach(b => b.addEventListener('click', () => open(b.dataset.moment, b)));
  window.addEventListener('storage', () => { program = program || fromStore(); });
})();
