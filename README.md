# Flying Saucers (Radio Shack, 1979), taken apart

*Assembly Demystified* study of **Flying Saucers**, Radio Shack catalog 26-1905,
by Robert Arnstein: a 3,345-byte machine-language game for a 4K TRS-80 Model I.
The game keeps its world on the screen: collisions are read back from video RAM,
and a shot has no direction of its own; it moves the way the cannon points at
that instant, which is how you steer it in flight.

Repository: https://github.com/jeffadickson/TRS80-flyingsaucer

## Layout

| path | what |
|---|---|
| `src/flysaucers.asm` | the commented listing, the source of truth; rebuilds `original/flysauc5.cmd` byte for byte |
| `original/` | the five /CMD copies from Ira Goldklang's TRS-80 archive (open mode, see `RIGHTS.md`) |
| `build/` | generated: the rebuilt CMD, listings, emulator runs (not committed) |
| `tools/` | `annot.py` (annotations) and `listing.py` (listing generator), `regs.py` (static register pass), `mkcmd.py`/`verify.py`, `scenarios.py`, `measure.py`, `captures.py`, `opt.py` (the "faster" variants), `web.py` |
| `tests/` | input scripts, ZEX runner, shift test, JS-vs-Rust equivalence, moment tests |
| `emulator/rust/` | the study emulator (series Z80 core, ZEXDOC/ZEXALL 67/67); `fsrun` runs scenarios headless |
| `emulator/js/` | the browser machine, checked against Rust every field (T-states, registers, R, screen, RAM) |
| `book/` | thesis, `figures.json`, `opt.json`, `history.json`, `fig/` (captures), `moments/` (live examples) |
| `web/` | the web edition's sources (`web/src`); `make web` builds `web/out` |
| `Makefile` | `make verify` checks the rebuilt binary against the original |

## Build

```
make verify     # src/flysaucers.asm -> FLYSAUC5.CMD, byte for byte (SHA-256 0f27a03a...44d38); shift test
make test       # + scenarios, JS/Rust equivalence, figures, captures, moments, faster variants, web
make exercise   # ZEXDOC and ZEXALL on the Z80 core
```

Needs pasmo, python3 with Pillow, cargo, node and ffmpeg.

## Licences

Code (tools, emulators, tests, Makefile, page code): MIT (`LICENSE`). Book text,
annotations, moments, figures and art: all rights reserved (`LICENSE-BOOK.md`).
Flying Saucers © 1979 Robert Arnstein, licensed to Tandy Corporation; included
for study, no licence is granted to it (`RIGHTS.md`).
