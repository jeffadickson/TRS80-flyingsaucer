# z80emu — a study emulator for Frogger (and Galaxy Invasion Plus)

A Z80 core and just enough of a TRS-80 Model I (with Expansion Interface and
one disk drive) to run `FROGGER1/CMD`, Robert Pappas's 1981 Cornsoft Frogger,
headless. The machine model began in the Galaxy Invasion Plus study and keeps
that program's ROM and DOS substitutes. It is for study: scripted input,
screen dumps, sound logs, traces and timing. It has no window and cannot be
played live.

## What Frogger needs from it

- ROM entry points, done in Rust (high-level emulation): 0033h (display a
  character), 0049h (wait for a key: the CPU stays at 0049h, 200 T-states per
  check, until a key is down), 01C9h (clear the screen, 21 T-states a cell),
  0630h (the backspace handler of the ROM line input: give back one character).
  402Dh (return to TRSDOS) is a HALT.
- TRSDOS @OPEN, @WRITE and @CLOSE: @WRITE copies the 256-byte record into the
  first record of the program file's image (Frogger keeps its high scores in
  the first sector of its own file). `--hiscore FILE` loads and saves that image.
- The keyboard matrix at 3800h-3BFFh, including multi-row reads such as 380Bh.
- Video RAM with the stock Model I bit-6 rule (bit 6 = NOT(bit 5 OR bit 7));
  `--lowercase` keeps bit 6.
- The cassette port, two bits, logged for `tools/sound.py`.

## Frogger options of trs80run

| option | meaning |
| --- | --- |
| `--autoplay F` / `--autoplay-end F` / `--seed S` | the capture robot plays from field F: it reads the frog's lane and column, the road and river object tables and the homes, and presses one arrow at a time |
| `--park` | the robot stops once the frog is on the river's near bank |
| `--budget FILE` | per-routine T-states of MainLoop's calls (808Ah-80EBh) and one line per pass |
| `--budget-screen 1|2` | count only ordinary passes of the road (1) or river (2) half |
| `--loop LO-HI:TOP` | the loop to budget (default 808A-80EC:808C) |

The rest of this file describes the options common to both studies.

## Build

Needs Rust 1.85 or later (edition 2024). No dependencies.

```
cd emulator/rust
cargo build --release      # or, from the repository root: make emulator
```

Binaries in `target/release/`:

| binary | what it does |
| --- | --- |
| `trs80run` | runs a CMD file on the emulated Model I, headless |
| `callrun` | calls one routine of the program in a fresh machine, many times, and reports registers, cycles and memory |
| `zex` | runs the ZEXDOC / ZEXALL CP/M exercisers on the Z80 core |
| `timing` (`make test`) | checks T-state counts and R increments against tables typed in from the Zilog manual |

The crate is called `z80emu`; the TRS-80 runner is `trs80run`.

## trs80run

```
trs80run <file.cmd> [options]
```

Time is counted in **video fields** (1/60 s = 29,568 T-states at 1.77408 MHz).
The book calls one pass of the game's main loop a "pass"; `--frames` counts
fields, not passes.

| option | meaning |
| --- | --- |
| `--frames N` | run N fields (default 600) |
| `--script FILE` | scripted keys (format below) |
| `--dump FILE` | append each field's video RAM (1,024 bytes + 1 byte wide-mode flag) to FILE (`.scr`) |
| `--every K` | dump only every K-th field |
| `--trace FILE` | one line per field: `frame cycles pc af bc de hl ix iy sp ramhash` (FNV-1a 64 of 3C00h-FFFFh) |
| `--sound FILE` | log of every change on cassette port FFh: `T-state value`; `tools/sound.py` turns it into WAV/MP3 |
| `--hiscore FILE` | host file used as HISCORES/GAL: loaded if present, saved at exit |
| `--lowercase` | Model I with the lower-case modification (video bit 6 kept) |
| `--joystick HEX` | value returned by port 00h (default FF = no joystick) |
| `--profile FILE` | T-states spent at each PC |
| `--writes FILE` | log every write to program memory: `cycles pc addr value` |
| `--watch A,B-C` | log writes to these addresses (hex, ranges allowed) |
| `--poke ADDR=VAL` | patch memory after loading (hex; may be repeated) |
| `--memdump F:FILE` | write all 64K at field F |
| `--memdump-pc ADDR:N:FILE` | write all 64K the N-th time the PC reaches ADDR |
| `--budget FILE` | per-routine T-states for the main loop's calls (used for chapter 6) |
| `--autoplay F` / `--autoplay-end F` / `--seed S` | the capture robot: a simple player that takes over the keys from field F |
| `--dos-log` | print the emulated TRSDOS calls |

**Script format:** one line per event, `<field> <down|up|tap> KEY [fields]`.
`tap` holds the key for the given number of fields (default 3). Example:

```
400 tap CLEAR 5
500 tap 1
600 down RIGHT
640 up RIGHT
```

Key names: `@ A`–`Z`, `0`–`9`, `: ; , - . /`, `ENTER CLEAR BREAK UP DOWN LEFT
RIGHT SPACE SHIFT`. The game's own keys are LEFT/RIGHT (or `,` `.` with SHIFT
for `<` `>`), F to fire, P to pause, S for sound, BREAK+CLEAR to abort.

A complete run, from the repository root:

```
emulator/rust/target/release/trs80run original/galinvp2.cmd --frames 1500 \
    --script tests/scripts/s1.txt --trace s1.tr --dump s1.scr --every 100 --sound s1.snd
python3 tools/render.py s1.scr shot 6 12   # dumps 6 and 12 = fields 600 and 1200 -> shot_0006.png, shot_0012.png
```

## What is emulated, and what is substituted

- **CPU.** Every documented and undocumented Z80 instruction, undocumented
  flags (X/Y), MEMPTR, the refresh register R (7 bits, +1 per M1 cycle,
  prefixes included) and exact T-state counts.
- **Memory map.** 48K RAM; keyboard matrix at 3800h (row n at 3800h + 2^n);
  video RAM 3C00h-3FFFh with the stock Model I store rule (bit 6 =
  NOT(bit 5 OR bit 7)); FDC status at 37ECh shows an index pulse
  (7,000 T of every 354,816 T, five per second), which is all the game's
  drive check reads.
- **The Tandy Level II ROM is not included.** Its place is taken by:
  - 0060h (delay loop): the real four-instruction loop, byte for byte;
  - 002Bh (keyboard) and 0033h (display): **high-level emulation (HLE)** —
    the emulator catches the call at that address, does the job in Rust,
    charges a fixed 400 or 300 T-states, and returns;
  - ROM bytes 0000h-01FFh read by the game's Random routine: FFh, so random
    sequences differ from a real machine;
  - byte 0125h: not `I`, so the game takes its Model I paths.
- **TRSDOS** calls 4409h (error), 4420h (@INIT), 4424h (@OPEN), 4428h
  (@CLOSE), 4436h (@READ), 4439h (@WRITE) and 443Ch (@VERF) are HLE on one
  host file, each charged 2,000 T-states.

## callrun

```
callrun original/galinvp2.cmd < cases.txt > results.txt
```

Input line: `ENTRY A F B C D E H L IXh IXl IYh IYl ; POKES ; READS`, all hex.
POKES = `addr=hexbytes,...`; READS = `addr:len,...`. The routine is called
with SP = BFF0h and returns to a HALT at FFF0h. Output line:
`A F B C D E H L IX IY cycles ; bytes...`.

## Validation

| test | result |
| --- | --- |
| ZEXDOC (`make zex`) | all 67 groups OK |
| ZEXALL (`make zex`) | all 67 groups OK (undocumented flags included) |
| `timing` (`make test`) | 749 T-state and R checks against the Zilog UM0080 tables, 0 mismatches |
| second implementation (`../python`, pure Python, written separately) | per-field traces (cycles, all registers, RAM hash) identical for 1,500 fields under two input scripts; `make test` repeats this for 900 fields |
| byte-identical reassembly of the commented listing (`make verify`) | 25,512 program bytes match; the 26,112-byte CMD file matches with its 188 bytes of sector slack carried over |

ZEXDOC/ZEXALL are Frank Cringle's exercisers (GPL) and are not part of this
repository; `make zex` downloads them into `build/` and runs both.

## Playing the game

Use a full TRS-80 emulator with a real ROM and TRSDOS: **trs80gp** (Model I
and III; ROMs built in), or **sdltrs / sdl2trs** or **xtrs** (need a ROM image).
Import `galinvp2.cmd` onto a TRSDOS 2.3 (Model I) or TRSDOS 1.3 (Model III)
disk image with the emulator's import feature, boot TRSDOS, and type
`GALINVP2` at the prompt. (On the TRSDOS disk the file is `GALINVP2/CMD`;
TRSDOS writes NAME/EXT with a slash.) The game makes DOS calls for its
high-score file, so it must be run from a booted DOS, not loaded bare. Keep
the disk writable if you want HISCORES/GAL saved.
