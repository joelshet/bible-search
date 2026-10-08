#!/usr/bin/env python3
"""Draw the home-screen icons in web/icons/: the favicon's four tiles on paper, with
enough margin that a round or rounded mask doesn't clip them. Standard library only."""
import struct
import zlib
from pathlib import Path

PAPER, TILE, RUBRIC = (0xFB, 0xFA, 0xF7), (0xD9, 0xD5, 0xCC), (0xB3, 0x26, 0x1E)
# x, y, width, height on the favicon's 32-unit grid (the icon link in web/index.html)
TILES = [(4, 4, 11, 9, TILE), (17, 4, 11, 14, TILE), (4, 15, 11, 13, TILE), (17, 20, 11, 8, RUBRIC)]
MARGIN = 0.22  # of the icon's width, on every side


def png(size):
    rows = [[PAPER] * size for _ in range(size)]
    scale = size * (1 - 2 * MARGIN) / 24  # the tiles span 24 units, from 4 to 28
    at = lambda units: round(size * MARGIN + (units - 4) * scale)
    for x, y, w, h, color in TILES:
        for row in rows[at(y):at(y + h)]:
            row[at(x):at(x + w)] = [color] * (at(x + w) - at(x))
    raw = b"".join(b"\0" + bytes(c for pixel in row for c in pixel) for row in rows)
    chunk = lambda kind, data: struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    header = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)  # 8-bit RGB
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


out = Path(__file__).resolve().parent.parent / "web" / "icons"
out.mkdir(exist_ok=True)
for size in (180, 192, 512):
    (out / f"icon-{size}.png").write_bytes(png(size))
