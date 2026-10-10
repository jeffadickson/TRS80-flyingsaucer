# Flying Saucers (Robert Arnstein, Radio Shack 26-1905, 1979), taken apart.
# Open mode (RIGHTS.md): src/flysaucers.asm is the source of truth and rebuilds
# original/flysauc5.cmd byte for byte (SHA-256 0f27a03a...44d38).
#
#   make listing    program file + tools/annot.py -> build/flysaucers.asm (commented, full)
#   make verify     assemble that listing and check it rebuilds your file byte for byte,
#                   then the shift test (assembled 100h higher, same screens)
#   make exercise   ZEXDOC and ZEXALL on the Rust Z80 core (fetches the .com files)
#   make scenarios  run every emulator scenario (profiles, dumps)
#   make equiv      the browser machine (JS) against the Rust one, every field, zero tolerance
#   make measure    book/figures.json
#   make captures   moment clips and stills in book/fig, expectations in book/moments/
#   make faster     P1/P2 patched builds, checked pass for pass against the original (book/opt.json)
#   make web        the web edition in web/out (all pages)
#   make test       everything above that runs offline
#   make all        exercise + test
#
# Tools: pasmo, python3 (Pillow), cargo, node, ffmpeg.

BIN     ?= original/flysauc5.cmd
PASMO   ?= pasmo
PYTHON  ?= python3 -I
NODE    ?= node
CARGO   ?= cargo
EMU     := emulator/rust/target/release
FSRUN   := $(EMU)/fsrun

.PHONY: all test listing verify shift exercise emulator scenarios equiv measure captures faster web clean

all: exercise test
test: verify scenarios equiv measure captures moments faster web

emulator: $(FSRUN)
$(FSRUN): $(wildcard emulator/rust/src/*.rs emulator/rust/src/bin/*.rs) emulator/rust/Cargo.toml
	cd emulator/rust && $(CARGO) build --release

listing: $(FSRUN)
	@test -f build/run/play5.prof || $(MAKE) scenarios
	$(PYTHON) tools/listing.py $(BIN)

verify: listing
	cmp src/flysaucers.asm build/flysaucers.asm || (echo 'src/flysaucers.asm differs from the generated listing: run make listing and copy it' && exit 1)
	$(PASMO) --bin src/flysaucers.asm build/flysaucers.bin
	$(PYTHON) tools/mkcmd.py build/flysaucers.bin build/FLYSAUC5.CMD
	$(PYTHON) tools/verify.py build/FLYSAUC5.CMD $(BIN)
	$(PYTHON) tests/test_shift.py

exercise: $(FSRUN)
	sh tests/zex.sh

scenarios: $(FSRUN)
	$(PYTHON) tools/scenarios.py

equiv: $(FSRUN)
	for s in edges demo shifted; do \
	  $(FSRUN) $(BIN) --frames 3000 --script tests/scripts/$$s.txt --trace build/run/$$s.trace 2>/dev/null && \
	  $(NODE) tests/test_js.js $(BIN) tests/scripts/$$s.txt build/run/$$s.trace 3000 || exit 1; done

measure: $(FSRUN)
	$(PYTHON) tools/measure.py > /dev/null

captures: listing
	$(PYTHON) tools/captures.py

moments: captures
	$(NODE) tests/test_moments.js $(BIN)

clean:
	rm -rf build

faster: verify
	$(PYTHON) tools/opt.py

web: captures faster measure
	$(PYTHON) tools/web.py
