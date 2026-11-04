"""Generate the 1024x1024 source icon (status ring over the hull color).

Run: python3 scripts/make-icon.py src-tauri/icons/source.png
then: npx tauri icon src-tauri/icons/source.png
"""
import math, struct, sys, zlib

N = 1024
HULL = (0x22, 0x34, 0x3D)
WATER = (0x3A, 0xA0, 0xB5)
INK = (0xEA, 0xF1, 0xF2)

def inside_rounded(x, y, pad=64, r=200):
    lo, hi = pad, N - pad
    cx = min(max(x, lo + r), hi - r)
    cy = min(max(y, lo + r), hi - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r and lo <= x <= hi and lo <= y <= hi

def pixel(x, y):
    if not inside_rounded(x, y):
        return None
    c = N / 2
    dx, dy = x - c, y - c
    d = math.hypot(dx, dy)
    # Ring: 70% arc in ink, remainder faint.
    if abs(d - 300) <= 52:
        ang = (math.atan2(dx, -dy) / (2 * math.pi)) % 1.0
        return INK if ang <= 0.70 else tuple(int(h * 0.7 + i * 0.3) for h, i in zip(HULL, INK))
    # Water inside the ring below the waterline.
    if d < 230 and y > c + 40:
        return WATER
    return HULL

def main(out):
    ss = 3
    rows = []
    for y in range(N):
        row = bytearray([0])
        for x in range(N):
            acc = [0, 0, 0, 0]
            for sy in range(ss):
                for sx in range(ss):
                    p = pixel(x + (sx + 0.5) / ss, y + (sy + 0.5) / ss)
                    if p:
                        acc[0] += p[0]; acc[1] += p[1]; acc[2] += p[2]; acc[3] += 1
            n = acc[3]
            if n:
                row += bytes([acc[0] // n, acc[1] // n, acc[2] // n, 255 * n // (ss * ss)])
            else:
                row += b"\0\0\0\0"
        rows.append(bytes(row))
    raw = b"".join(rows)
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(out, "wb").write(png)

if __name__ == "__main__":
    main(sys.argv[1])
