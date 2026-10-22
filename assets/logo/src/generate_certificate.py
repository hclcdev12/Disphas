#!/usr/bin/env python3
"""Generates the disphas marks printed on certificates (run from anywhere).

Writes into assets/logo/:
- disphas-wordmark-certificate.svg: "Disphas" in Klaudia Bold, state 0
  ("donnée": every letter full), ink, no ermine (IDENTITE.md §3.2, §6.4).
- disphas-wordmark-os.svg: the same wordmark in bone, for the dark
  interface of the application (24 px high: state 0, IDENTITE.md §3.3 rule 4).
- disphas-stamp.svg: the seal next to the QR code, in ink (no accent
  colour on the certificate, IDENTITE.md §5.2), with the small logo (the
  D of Klaudia Bold, IDENTITE.md §3.4) in the centre.

Every file is pure paths: no font is needed to display them.
Klaudia (assets/fonts/klaudia, SIL OFL 1.1) is read with fontTools and
shaped with HarfBuzz to keep the font's own spacing and kerning (rule 2 of
IDENTITE.md §3.3: original tracking below 96 px). Development tool only:
    python3 -m venv venv && venv/bin/pip install fonttools uharfbuzz
    venv/bin/python assets/logo/src/generate_certificate.py
"""
import math
from pathlib import Path

import uharfbuzz as hb
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

HERE = Path(__file__).resolve().parent
OUT = HERE.parent
FONT = HERE.parent.parent / "fonts" / "klaudia" / "Klaudia-Bold.ttf"

INK = "#111110"  # IDENTITE.md §5.2: certificate text only
BONE = "#ECEAE3"  # IDENTITE.md §5.1: logo on a dark background

font = TTFont(FONT)
glyph_set = font.getGlyphSet()
cmap = font.getBestCmap()
UPEM = font["head"].unitsPerEm
CAP = font["OS/2"].sCapHeight


def outline(name):
    """Glyph outline as an SVG path, y pointing down (baseline at 0)."""
    pen = SVGPathPen(glyph_set, ntos=lambda v: f"{v:g}")
    glyph_set[name].draw(TransformPen(pen, (1, 0, 0, -1, 0, 0)))
    return pen.getCommands()


def bounds(name):
    """xMin, yMin, xMax, yMax of a glyph, y pointing up."""
    glyph = font["glyf"][name]
    glyph.recalcBounds(font["glyf"])
    return glyph.xMin, glyph.yMin, glyph.xMax, glyph.yMax


def svg(view_box, body, title):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}" role="img">'
        f"<title>{title}</title>{body}</svg>\n"
    )


def shape(text):
    """(glyph name, x) pairs of `text` shaped by HarfBuzz, in font units."""
    face = hb.Face(FONT.read_bytes())
    hb_font = hb.Font(face)
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(hb_font, buf, {"kern": True, "liga": False})
    x, out = 0, []
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        out.append((font.getGlyphName(info.codepoint), x + pos.x_offset))
        x += pos.x_advance
    return out


def wordmark(color):
    """'Disphas', state 0: all letters full, no texture, no cursor."""
    paths, x_min, x_max, y_min, y_max = [], math.inf, -math.inf, math.inf, -math.inf
    for name, x in shape("Disphas"):
        paths.append(f'<path transform="translate({x} 0)" d="{outline(name)}"/>')
        g_x_min, g_y_min, g_x_max, g_y_max = bounds(name)
        x_min, x_max = min(x_min, x + g_x_min), max(x_max, x + g_x_max)
        y_min, y_max = min(y_min, g_y_min), max(y_max, g_y_max)
    # The box hugs the ink: the certificate sets the height, not the margins.
    view_box = f"{x_min} {-y_max} {x_max - x_min} {y_max - y_min}"
    return svg(view_box, f'<g fill="{color}">{"".join(paths)}</g>', "Disphas")


def arc_text(text, size, baseline, centre_angle, outward):
    """Glyph paths set along a circle, centred on `centre_angle`. `outward`:
    tops point away from the centre (top of a stamp, read clockwise);
    otherwise towards it (bottom of a stamp, read left to right, upright)."""
    scale = size / UPEM
    spacing = 0.5
    names = [font.getGlyphName(font.getGlyphID(cmap[ord(ch)])) for ch in text]
    advances = [font["hmtx"][name][0] * scale for name in names]
    total = sum(advances) + spacing * (len(text) - 1)
    out, u = [], -total / 2
    for name, advance in zip(names, advances):
        middle = u + advance / 2
        u += advance + spacing
        d = outline(name)
        if not d:
            continue
        if outward:
            angle = centre_angle + middle / baseline
            degrees = math.degrees(angle + math.pi / 2)
        else:
            angle = centre_angle - middle / baseline
            degrees = math.degrees(angle - math.pi / 2)
        px, py = 50 + baseline * math.cos(angle), 50 + baseline * math.sin(angle)
        out.append(
            f'<path transform="translate({px:.2f} {py:.2f}) rotate({degrees:.2f}) '
            f'translate({-advance / 2:.2f} 0) scale({scale:.5f})" d="{d}"/>'
        )
    return "".join(out)


def small_logo(height):
    """The D of Klaudia Bold, full, `height` units tall, centred on (50, 50)."""
    name = cmap[ord("D")]
    x_min, _, x_max, _ = bounds(name)
    scale = height / CAP
    tx = 50 - (x_min + x_max) / 2 * scale
    ty = 50 + height / 2
    return f'<path transform="translate({tx:.2f} {ty:.2f}) scale({scale:.5f})" d="{outline(name)}"/>'


def stamp():
    """Secondary mark for certificates: notched seal, the name on top, the
    signature algorithm below, separated by dots, the small logo in the
    centre. No third-party name (NIST, IEEE): a seal bearing them would
    suggest a certification they never granted."""
    points = []
    for i in range(48):
        radius = 46 if i % 2 == 0 else 41.5
        angle = math.pi * 2 * i / 48 - math.pi / 2
        points.append(f"{50 + radius * math.cos(angle):.2f},{50 + radius * math.sin(angle):.2f}")
    top = arc_text("DISPHAS", 6.4, 28.6, 3 * math.pi / 2, outward=True)
    bottom = arc_text("Ed25519", 5.6, 33.4, math.pi / 2, outward=False)
    dots = "".join(
        f'<circle cx="{50 + 31.0 * math.cos(a):.2f}" cy="{50 + 31.0 * math.sin(a):.2f}" r="1.1"/>'
        for a in (math.radians(200), math.radians(-20))
    )
    body = (
        f'<g fill="none" stroke="{INK}" stroke-linejoin="round">'
        f'<polygon stroke-width="2.4" points="{" ".join(points)}"/>'
        f'<circle cx="50" cy="50" r="37" stroke-width="1.2"/>'
        f'<circle cx="50" cy="50" r="24.5" stroke-width="1.2"/></g>'
        f'<g fill="{INK}">{top}{bottom}{dots}{small_logo(22)}</g>'
    )
    return svg("0 0 100 100", body, "Disphas · Ed25519")


FILES = {
    "disphas-wordmark-certificate.svg": wordmark(INK),
    "disphas-wordmark-os.svg": wordmark(BONE),
    "disphas-stamp.svg": stamp(),
}

if __name__ == "__main__":
    for file_name, content in FILES.items():
        (OUT / file_name).write_text(content)
        print(f"{file_name}: {len(content)} bytes")
