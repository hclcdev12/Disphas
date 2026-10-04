#!/usr/bin/env python3
"""Generates the Oblivio logo files in assets/logo/ (run from anywhere).

Glyph outlines come from Inter Bold (assets/fonts, SIL OFL 1.1), extracted
once into glyphs-*.jsonl, so every file is pure paths: no font needed to
display them. Brand rules: docs/oblivio_charte_graphique.md §6.
"""
import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent

INDIGO = "#6366F1"        # brand indigo (dark backgrounds, icon)
INDIGO_DEEP = "#4F46E5"   # indigo on white (certificate, stamp)
INK = "#111827"
SOFT = "#F9FAFB"

R = 41.0  # outer radius of the symbol, in a 100-unit box


def symbol(band, fractions, gap):
    """The O that erases: left half ring, right half horizontal lines,
    shortened from the right, clipped to the circle silhouette."""
    n = len(fractions)
    pitch = 2 * R / n
    height = pitch * 0.62
    inner = R - band
    parts = [
        f'<path d="M50 {50 - R:.1f}A{R:.0f} {R:.0f} 0 0 0 50 {50 + R:.1f}'
        f'L50 {50 + inner:.1f}A{inner:.0f} {inner:.0f} 0 0 1 50 {50 - inner:.1f}Z"/>'
    ]
    x0 = 50 + gap
    for i, fraction in enumerate(fractions):
        top = 50 - R + i * pitch + (pitch - height) / 2
        far = max(abs(top - 50), abs(top + height - 50))
        edge = 50 + math.sqrt(max(R * R - far * far, 0.0))
        parts.append(
            f'<rect x="{x0:.1f}" y="{top:.1f}" width="{(edge - x0) * fraction:.1f}" height="{height:.1f}"/>'
        )
    return "".join(parts)


# D2: the main symbol, 4 lines; its ring weight matches Inter Bold's stroke.
MAIN = symbol(19, [0.3, 0.65, 0.92, 1.0], gap=3)
# D5: 32 px and below (app icon, favicon), 3 lines.
SMALL = symbol(20, [0.4, 0.82, 1.0], gap=4)
# In the wordmark the symbol must read as the letter o: fuller lines that
# rebuild the circle's silhouette, the erasure still visible at the top (W2).
LETTER = symbol(19, [0.55, 0.88, 1.0, 1.0], gap=3)


def glyphs(name):
    return [json.loads(line) for line in (HERE / name).read_text().splitlines()]


def svg(view_box, body, title):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}" role="img">'
        f"<title>{title}</title>{body}</svg>\n"
    )


def wordmark(ink, mark):
    """'oblivi' in Inter Bold (tracking -10/2048 em), then the symbol in place
    of the final o, sized and placed exactly like Inter's o (2048 units/em)."""
    x = 0
    letters = []
    for glyph in glyphs("glyphs-wordmark.jsonl"):
        letters.append(f'<path transform="translate({x} 0)" d="{glyph["d"]}"/>')
        x += glyph["adv"] - 10
    # Inter Bold 'o': x 81..1175, y -1132..22 (centre 628, -555).
    scale = 1154 / (2 * R)
    tx, ty = x + 628 - 50 * scale, -555 - 50 * scale
    body = (
        f'<g fill="{ink}">{"".join(letters)}</g>'
        f'<g fill="{mark}" transform="translate({tx:.1f} {ty:.1f}) scale({scale:.3f})">{LETTER}</g>'
    )
    return svg("40 -1612 6586 1674", body, "Oblivio")


def arc_text(text, glyph_map, size, baseline, centre_angle, outward):
    """Glyph paths set along a circle, centred on `centre_angle`. `outward`:
    tops point away from the centre (top of a stamp, read clockwise);
    otherwise towards it (bottom of a stamp, read left to right, upright)."""
    scale = size / 2048
    spacing = 0.5
    advances = [glyph_map[ch]["adv"] * scale for ch in text]
    total = sum(advances) + spacing * (len(text) - 1)
    out, u = [], -total / 2
    for ch, advance in zip(text, advances):
        middle = u + advance / 2
        u += advance + spacing
        d = glyph_map[ch]["d"]
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


def stamp(color):
    """Secondary mark for certificates: notched seal, the name on top, the
    signature algorithm below, separated by dots, the small symbol in the
    centre. No third-party name (NIST, IEEE): a seal bearing them would
    suggest a certification they never granted."""
    points = []
    for i in range(48):
        radius = 46 if i % 2 == 0 else 41.5
        angle = math.pi * 2 * i / 48 - math.pi / 2
        points.append(f"{50 + radius * math.cos(angle):.2f},{50 + radius * math.sin(angle):.2f}")
    glyph_map = {g["ch"]: g for g in glyphs("glyphs-stamp.jsonl")}
    top = arc_text("OBLIVIO", glyph_map, 6.4, 28.6, 3 * math.pi / 2, outward=True)
    bottom = arc_text("Ed25519", glyph_map, 5.6, 33.4, math.pi / 2, outward=False)
    dots = "".join(
        f'<circle cx="{50 + 31.0 * math.cos(a):.2f}" cy="{50 + 31.0 * math.sin(a):.2f}" r="1.1"/>'
        for a in (math.radians(200), math.radians(-20))
    )
    body = (
        f'<g fill="none" stroke="{color}" stroke-linejoin="round">'
        f'<polygon stroke-width="2.4" points="{" ".join(points)}"/>'
        f'<circle cx="50" cy="50" r="37" stroke-width="1.2"/>'
        f'<circle cx="50" cy="50" r="24.5" stroke-width="1.2"/></g>'
        f'<g fill="{color}">{top}{bottom}{dots}</g>'
        f'<g fill="{color}" transform="translate(34 34) scale(0.32)">{SMALL}</g>'
    )
    return svg("0 0 100 100", body, "Oblivio · Ed25519")


def icon():
    """Application icon: indigo tile, white small symbol."""
    body = (
        f'<rect width="512" height="512" rx="112" fill="{INDIGO}"/>'
        f'<g fill="#FFFFFF" transform="translate(92 92) scale(3.28)">{SMALL}</g>'
    )
    return svg("0 0 512 512", body, "Oblivio")


FILES = {
    "oblivio-symbol.svg": svg("0 0 100 100", f'<g fill="{INDIGO}">{MAIN}</g>', "Oblivio"),
    "oblivio-symbol-mono.svg": svg("0 0 100 100", f'<g fill="#000000">{MAIN}</g>', "Oblivio"),
    "oblivio-symbol-small.svg": svg("0 0 100 100", f'<g fill="{INDIGO}">{SMALL}</g>', "Oblivio"),
    "oblivio-wordmark-dark.svg": wordmark(SOFT, INDIGO),
    "oblivio-wordmark-light.svg": wordmark(INK, INDIGO_DEEP),
    "oblivio-wordmark-mono.svg": wordmark("#000000", "#000000"),
    "oblivio-stamp.svg": stamp(INDIGO_DEEP),
    "oblivio-icon.svg": icon(),
}

if __name__ == "__main__":
    for name, content in FILES.items():
        (OUT / name).write_text(content)
        print(f"{name}: {len(content)} bytes")
