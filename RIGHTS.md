# Rights

## The program

*Flying Saucers*, Radio Shack catalog number 26-1905. The manual's credit page
reads: program (c) 1979 Robert Arnstein, licensed to Tandy Corporation; manual
(c) 1979, 1981 Tandy Corporation. ([Flying Saucers manual, Internet Archive](https://archive.org/details/Flying_Saucers_1981_Tandy))

| | |
|---|---|
| Owner | Robert Arnstein (author), licensed to Tandy Corporation |
| Public-domain release | none found |
| Permission | none on record |
| **Mode** | **overlay**: no program bytes in this repository or its web edition |

In overlay mode the repository ships the annotations (`annotations/annot.py`:
labels, comments, routine headers, the code/data map, keyed by address).
`make listing BIN=your/flysauc5.cmd` checks your file against the recorded
SHA-256, disassembles it and merges the annotations into the full commented
listing. The book quotes only the routines it discusses. Its moments show
captures; the Run buttons ask for your copy of the file and replay each
moment's input. No save state is published.

If a public-domain release or permission is found, record it here (link,
date, who made it) and the project can move to open mode.

## The copy studied

`flysauc5.cmd`, one of five /CMD files for Flying Saucers in Ira Goldklang's
TRS-80 archive (zip "Flying Saucers (19xx)(Radio Shack)[CMD]", note dated
2001-05-30: "They all seem to be the same"). SHA-256s:

| file | SHA-256 | what it is |
|---|---|---|
| flysauc1.cmd | bae1cec8...879dca | game relocated to run at 5000h |
| flysauc2.cmd | 53ea84ce...d0d6 | image of 4000h-4FFFh, moved down by a stub at 7000h |
| flysauc3.cmd | 3d398d2a...272b | image of 4000h-51FFh, header "MODIFIED USING OFFSET (C) 1983 ZETA MICROCOMPUTER SOFTWARE" |
| flysauc4.cmd | 0fb247cd...a182 | 10 bytes patched (stack moved to 7FFFh, a hook at 43CAh) |
| **flysauc5.cmd** | **0f27a03a...44d38** | **ROM table copy + move to 42EAh: studied here** |

The game bytes at 42EAh-4FFAh are identical in flysauc2, 3 and 5.

## The ROM

The Model I Level II ROM is not shipped and not used. The program calls the ROM
once (002Bh, the keyboard scan); the emulators replace that one routine with
code of their own (`emulator/rust/src/trs80.rs`, `emulator/js/saucsys.js`).

## This repository

Tools, emulators and build code: MIT (`LICENSE`). Book text, annotations and
original art: all rights reserved (`LICENSE-BOOK.md`). The program keeps its own
status, above; no licence is granted to it. The excerpts in the book are quoted
for commentary.
