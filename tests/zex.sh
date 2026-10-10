#!/bin/sh
# Run Frank Cringle's ZEXDOC and ZEXALL Z80 exercisers on the Rust CPU core.
# The .com files are GPL and not part of this repository; they are fetched into build/ on first use.
set -e
ZEX=emulator/rust/target/release/zex
URL=https://raw.githubusercontent.com/anotherlin/z80emu/master/testfiles
mkdir -p build
for t in zexdoc zexall; do
  [ -f build/$t.com ] || curl -fsSL -o build/$t.com $URL/$t.com
  echo "== $t"
  $ZEX build/$t.com | tee build/$t.out | tail -3
  grep -q "ERROR" build/$t.out && exit 1
done
exit 0
