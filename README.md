# Flying Saucers (Radio Shack, 1979), taken apart

*Assembly Demystified* study of **Flying Saucers**, Radio Shack catalog 26-1905,
by Robert Arnstein: a 3,345-byte machine-language game for a 4K TRS-80 Model I.
The thesis, in one line: the game keeps its world on the screen. Collisions are
read back from video RAM, and a shot has no direction of its own; it moves the
way the cannon points at that instant, which is how you steer it in flight.

**Open mode** (see `RIGHTS.md`): `original/` holds the five /CMD copies from
Ira Goldklang's TRS-80 archive; `flysauc5.cmd` (SHA-256 `0f27a03a…44d38`) is the
one studied, and `src/flysaucers.asm` rebuilds it byte for byte.

## Build

```
make verify     # src/flysaucers.asm reassembles to original/flysauc5.cmd byte for byte; shift test
make listing    # regenerate the listing from the file + annotations (must equal src/)
make test       # + scenarios, JS/Rust equivalence, figures, captures, moments
make exercise   # ZEXDOC and ZEXALL on the Z80 core
```

Needs pasmo, python3 with Pillow, cargo, node and ffmpeg.

## What is here

| path | what |
|---|---|
| `annotations/annot.py` | labels, routine headers, line comments, code/data map |
| `tools/listing.py` | merges your file and the annotations; register, stack and cycle lines come from `tools/regs.py` (static, every path) checked against the emulator |
| `emulator/rust/` | the study emulator (series Z80 core: ZEXDOC/ZEXALL 67/67); `fsrun` runs scenarios headless |
| `emulator/js/` | the browser machine; `tests/test_js.js` checks it against Rust every field (T-states, registers, R, screen, RAM) |
| `moments/` | the chapter's live examples: input scripts, breakpoints, expected state |
| `book/` | thesis, `figures.json` (every measured number), captures |
| `web/` | the chapter's web edition (`tools/web.py` builds `web/out/`) |

## Licences

Tools, emulators and build code: MIT (`LICENSE`). Book text, annotations,
moments and figures: all rights reserved (`LICENSE-BOOK.md`). Flying Saucers
© 1979 Robert Arnstein, licensed to Tandy Corporation; no licence is granted
to it here (`RIGHTS.md`).
