"""Puw Player app icon (run: python make_icon.py <assets dir> <preview dir>; needs Pillow): a black
play triangle on white inside a dark, lightly rounded square border. Same pipeline as PUW Notch.
Drawn at 1024 px, downsampled."""
import sys
import io, struct
from PIL import Image, ImageDraw
N = 1024
icon = Image.new('RGBA', (N, N), (0, 0, 0, 0))
d = ImageDraw.Draw(icon)
# white rounded square inside a dark border
border, radius = N * 0.065, N * 0.16
d.rounded_rectangle((0, 0, N - 1, N - 1), radius=radius, fill=(28, 28, 34, 255))
d.rounded_rectangle((border, border, N - 1 - border, N - 1 - border), radius=radius - border, fill=(250, 250, 252, 255))
# play triangle with softly rounded corners, nudged right so its centroid sits at the center.
# Each corner becomes an arc of radius r around the triangle shrunk toward its incenter by r,
# so the rounded outline stays inside the original triangle; filled as one polygon (no seams).
import math
h, w, r = N * 0.46, N * 0.40, N * 0.045
cy = N * 0.5
x0 = N * 0.5 - w / 3
pts = [(x0, cy - h / 2), (x0 + w, cy), (x0, cy + h / 2)]
side = lambda p, q: math.dist(p, q)
a_, b_, c_ = side(pts[1], pts[2]), side(pts[0], pts[2]), side(pts[0], pts[1])
per = a_ + b_ + c_
ix = (a_ * pts[0][0] + b_ * pts[1][0] + c_ * pts[2][0]) / per
iy = (a_ * pts[0][1] + b_ * pts[1][1] + c_ * pts[2][1]) / per
k = 1 - r / (w * h / per)  # w * h / per = inradius
inner = [(ix + (x - ix) * k, iy + (y - iy) * k) for x, y in pts]
def normal(p, q):  # outward normal of edge p->q (vertices are clockwise on screen)
    dx, dy = q[0] - p[0], q[1] - p[1]
    n = math.hypot(dx, dy)
    return math.atan2(-dx / n, dy / n)
outline = []
for i, v in enumerate(inner):
    start = normal(inner[i - 1], v)
    end = normal(v, inner[(i + 1) % 3])
    while end < start:
        end += 2 * math.pi
    for j in range(25):
        t = start + (end - start) * j / 24
        outline.append((v[0] + r * math.cos(t), v[1] + r * math.sin(t)))
d.polygon(outline, fill=(20, 20, 24, 255))
out = sys.argv[1]
icon.save(out + '/puw_1024.png')
sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
frames = {s: icon.resize((s, s), Image.LANCZOS) for s in sizes}
pngs = {}
for s, im in frames.items():
    b = io.BytesIO(); im.save(b, 'PNG'); pngs[s] = b.getvalue()
# .ico: PNG-compressed entries
ico = struct.pack('<HHH', 0, 1, len(sizes)); off = 6 + 16 * len(sizes); body = b''
for s in sizes:
    data = pngs[s]
    ico += struct.pack('<BBBBHHII', s % 256, s % 256, 0, 0, 1, 32, len(data), off + len(body)); body += data
open(out + '/puw.ico', 'wb').write(ico + body)
# .res for the MSVC linker: RT_ICON per size (ids 1..n) + RT_GROUP_ICON id 1
def entry(rtype, name, data):
    hdr = struct.pack('<HH', 0xFFFF, rtype) + struct.pack('<HH', 0xFFFF, name)
    hdr += struct.pack('<IHHII', 0, 0x1030, 0x0409, 0, 0)
    head = struct.pack('<II', len(data), 8 + len(hdr)) + hdr
    pad = (4 - len(data) % 4) % 4
    return head + data + b'\0' * pad
# leading empty resource header
res = struct.pack('<II', 0, 32) + struct.pack('<HHHH', 0xFFFF, 0, 0xFFFF, 0) + struct.pack('<IHHII', 0, 0, 0, 0, 0)
group = struct.pack('<HHH', 0, 1, len(sizes))
for n, s in enumerate(sizes, 1):
    res += entry(3, n, pngs[s])
    group += struct.pack('<BBBBHHIH', s % 256, s % 256, 0, 0, 1, 32, len(pngs[s]), n)
res += entry(14, 1, group)
open(out + '/puw.res', 'wb').write(res)
prev = Image.new('RGBA', (sum(sizes) + 12 * len(sizes), 270), (40, 40, 40, 255)); x = 6
for s in sizes:
    prev.alpha_composite(frames[s], (x, 270 - s - 6)); x += s + 12
prev.save(sys.argv[2] + '/icon_preview.png')
print('ok')
