"""Where everything lives, relative to the repository root."""
import os
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
ORIGINAL = os.path.join(ROOT, 'original', 'flysauc5.cmd')   # the reader's copy (not committed)
SRC = os.path.join(ROOT, 'src')
BUILD = os.path.join(ROOT, 'build')
RUN = os.path.join(BUILD, 'run')
BOOK = os.path.join(ROOT, 'book')
FIG = os.path.join(BOOK, 'fig')
EMU = os.path.join(ROOT, 'emulator', 'rust', 'target', 'release')
GAME_LO, GAME_HI = 0x42EA, 0x500E     # what the loader moves into place (42EAh-500Dh)
LOADER_LO, LOADER_HI = 0x5300, 0x531B  # the relocating stub
IMAGE_OFFSET = 0x531B - 0x42EA         # the game sits in the file this far above where it runs
for d in (BUILD, RUN, FIG):
    os.makedirs(d, exist_ok=True)
