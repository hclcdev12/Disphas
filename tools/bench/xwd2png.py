#!/usr/bin/env python3
"""Converts a 24/32-bit ZPixmap XWD screenshot to PNG (stdlib only).
Usage: xwd2png.py in.xwd out.png [scale]   (scale: keep 1 pixel in N)"""
import struct
import sys
import zlib


def main():
    src, dst = sys.argv[1], sys.argv[2]
    step = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    data = open(src, "rb").read()
    fields = struct.unpack(">25I", data[:100])
    header_size, _, fmt, _, width, height = fields[:6]
    byte_order, bpp, bytes_per_line = fields[7], fields[11], fields[12]
    ncolors = fields[19]
    if fmt != 2 or bpp not in (24, 32):
        sys.exit(f"unsupported XWD: format {fmt}, {bpp} bpp")
    pixels = data[header_size + 12 * ncolors:]
    size = bpp // 8
    rows = []
    for y in range(0, height, step):
        line = pixels[y * bytes_per_line:(y + 1) * bytes_per_line]
        row = bytearray(b"\x00")
        for x in range(0, width, step):
            p = line[x * size:x * size + size]
            if byte_order == 0:  # LSBFirst: B G R (X)
                row += bytes((p[2], p[1], p[0]))
            else:  # MSBFirst: (X) R G B
                row += bytes(p[-3:])
        rows.append(bytes(row))
    out_w, out_h = len(range(0, width, step)), len(rows)

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", out_w, out_h, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(b"".join(rows), 6))
    png += chunk(b"IEND", b"")
    open(dst, "wb").write(png)
    print(f"{dst}: {out_w}x{out_h}")


main()
