#!/usr/bin/env python3
"""Package the assembled image as FLYSAUC5.CMD, record for record like flysauc5.cmd.

pasmo --bin writes one image from 42EAh (the game) to 531Ah (the stub), the gap
500Eh-52FFh filled with zeros. The /CMD file holds the stub at 5300h followed
immediately by the game image (42EAh-500Dh, 0D24h bytes), as 256-byte load
records from 5300h (the last one 63 bytes), then the transfer record to 5300h.

usage: mkcmd.py IMAGE.bin OUT.cmd
"""
import sys

BASE, GAME_LO, GAME_HI, STUB_LO, STUB_HI = 0x42EA, 0x42EA, 0x500E, 0x5300, 0x531B


def main():
    img = open(sys.argv[1], 'rb').read()
    body = img[STUB_LO - BASE:STUB_HI - BASE] + img[GAME_LO - BASE:GAME_HI - BASE]
    assert len(body) == 0x0D3F, len(body)
    out = bytearray()
    for k in range(0, len(body), 256):
        part = body[k:k + 256]; a = STUB_LO + k
        out += bytes([0x01, (len(part) + 2) & 0xFF, a & 0xFF, a >> 8]) + part
    out += bytes([0x02, 0x02, STUB_LO & 0xFF, STUB_LO >> 8])
    open(sys.argv[2], 'wb').write(out)
    print('mkcmd: %s  %d bytes' % (sys.argv[2], len(out)))


if __name__ == '__main__':
    main()
