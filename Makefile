# Flying Saucers (Robert Arnstein, Radio Shack 26-1905, 1979), taken apart.
# Overlay mode: this repository holds no program bytes. Put your own copy of
# flysauc5.cmd at original/flysauc5.cmd (or pass BIN=path); everything is
# checked against its SHA-256 (0f27a03a...44d38).
#
#   make listing    your file + annotations/ -> build/flysaucers.asm (commented, full)
#   make verify     assemble that listing and check it rebuilds your file byte for byte,
#                   then the shift test (assembled 100h higher, same screens)
#   make exercise   ZEXDOC and ZEXALL on the Rust Z80 core (fetches the .com files)
#   make scenarios  run every emulator scenario (profiles, dumps)
#   make equiv      the browser machine (JS) against the Rust one, every field, zero tolerance
#   make measure    book/figures.json
#   make captures   moment clips and stills in book/fig, expectations in moments/
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

.PHONY: all test listing verify shift exercise emulator scenarios equiv measure captures clean

all: exercise test
test: verify scenarios equiv measure captures moments

emulator: $(FSRUN)
$(FSRUN): $(wildcard emulator/rust/src/*.rs emulator/rust/src/bin/*.rs) emulator/rust/Cargo.toml
	cd emulator/rust && $(CARGO) build --release

listing: $(FSRUN)
	@test -f build/run/play5.prof || $(MAKE) scenarios
	$(PYTHON) tools/listing.py $(BIN)

verify: listing
	$(PASMO) --bin build/flysaucers.asm build/flysaucers.bin
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
