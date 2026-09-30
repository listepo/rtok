"""Pyrlyn 'Prompt' (>_) production build. 32x32 grid, 2 units = 1 px at 16 px.
Usage: python3 build.py [OUT_DIR]   (needs fonttools, cairosvg; JetBrains Mono variable font via JBMONO_VF)"""
import os, sys, cairosvg
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
os.makedirs(OUT, exist_ok=True)
INK, PAPER = "#0C0E11", "#F4F2ED"
AMBER_ON_DARK, AMBER_ON_LIGHT = "#F2B33D", "#B97C06"
# --- mark: chevron with flat ends (horizontal thickness 6, 45 deg arms), cursor 10x4 on the baseline row
CHEV = "M4 6H10L20 16L10 26H4L14 16Z"
CUR = (18, 22, 10, 4)
def cur_d(dx=0):
    x, y, w, h = CUR
    return f"M{x+dx} {y}h{w}v{h}h-{w}z"
def mark_body(fg, acc, dx=0):
    chev = CHEV if dx == 0 else "M{} 6H{}L{} 16L{} 26H{}L{} 16Z".format(4+dx, 10+dx, 20+dx, 10+dx, 4+dx, 14+dx)
    if fg == acc:
        return f'<path fill="{fg}" d="{chev}{cur_d(dx)}"/>'
    return f'<path fill="{fg}" d="{chev}"/><path fill="{acc}" d="{cur_d(dx)}"/>'
# --- wordmark: JetBrains Mono wght 600, outlined. l top = chevron top (y6), baseline y22 = cursor top,
# so the cursor sits where an underscore would and the chevron centre matches the x-height centre.
FONT = os.environ.get("JBMONO_VF", "/usr/share/fonts/truetype/sand-box/google/JetBrains Mono/JetBrainsMono-VariableFont_wght.ttf")
f = instancer.instantiateVariableFont(TTFont(FONT), {"wght": 600})
gs, cmap, hmtx = f.getGlyphSet(), f.getBestCmap(), f["hmtx"]
BASE, TOP = 22, 6
s = (BASE - TOP) / f["OS/2"].sCapHeight
KERN = {"py": -21, "yr": -35, "rl": 12, "ly": -5, "yn": -22}  # hand pass (font units, 1000 upm): even optical gaps
def wordmark(x0):
    pen = SVGPathPen(gs, ntos=lambda v: ("%.2f" % v).rstrip("0").rstrip("."))
    x = x0; t = "pyrlyn"
    for i, ch in enumerate(t):
        g = cmap[ord(ch)]
        gs[g].draw(TransformPen(pen, (s, 0, 0, -s, x, BASE)))
        if i < len(t) - 1:
            x += (hmtx[g][0] + KERN.get(t[i:i+2], 0)) * s
    from fontTools.pens.boundsPen import BoundsPen
    bp = BoundsPen(gs); gs[cmap[ord("n")]].draw(bp)
    return pen.getCommands(), x0 + bp.bounds[0] * 0 + 80.49 * s, x + bp.bounds[2] * s
GAP = 7  # cursor to wordmark
def svg(vb, w, h, body):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}" width="{w}" height="{h}">{body}</svg>\n'
def lockup(fg, acc):
    mk = mark_body(fg, acc, dx=-4)          # mark ink x 0..24
    d0, l0, _ = wordmark(0)
    d, l, r = wordmark(24 + GAP - (l0 - 0))
    W = round(r)
    if fg == acc:
        mk = mk.replace('"/>', f'{d}"/>')
        body = mk
    else:
        body = mk + f'<path fill="{fg}" d="{d}"/>'
    # padding 2 units; y from 4 (above l/chevron top 6) to 30 (below descenders ~25.9 and cursor 26)
    return svg(f"-2 4 {W+4} 26", (W+4)*4, 104, body), W
def png(s_, path, w, h):
    cairosvg.svg2png(bytestring=s_.encode(), write_to=path, output_width=w, output_height=h)
for name, fg, acc in (("on-dark", PAPER, AMBER_ON_DARK), ("on-light", INK, AMBER_ON_LIGHT), ("mono", "currentColor", "currentColor")):
    m = svg("0 0 32 32", 32, 32, mark_body(fg, acc))
    l, W = lockup(fg, acc)
    open(f"{OUT}/pyrlyn-mark-{name}.svg", "w").write(m)
    open(f"{OUT}/pyrlyn-lockup-{name}.svg", "w").write(l)
    if name != "mono":
        png(m, f"{OUT}/pyrlyn-mark-{name}-1000.png", 1000, 1000)
        png(l, f"{OUT}/pyrlyn-lockup-{name}-2000.png", 2000, round(2000 * 26 / (W + 4)))
# favicons: adaptive SVG; PNGs on an ink rounded tile; apple-touch + GitHub avatar opaque ink squares
fav = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">'
       f'<style>path{{fill:{INK}}}.c{{fill:{AMBER_ON_LIGHT}}}'
       f'@media (prefers-color-scheme:dark){{path{{fill:{PAPER}}}.c{{fill:{AMBER_ON_DARK}}}}}</style>'
       f'<path d="{CHEV}"/><path class="c" d="{cur_d()}"/></svg>\n')
open(f"{OUT}/pyrlyn-favicon.svg", "w").write(fav)
tile = svg("0 0 32 32", 32, 32, f'<rect width="32" height="32" rx="6" fill="{INK}"/>' + mark_body(PAPER, AMBER_ON_DARK))
for px in (32, 64):
    png(tile, f"{OUT}/pyrlyn-favicon-{px}.png", px, px)
def square(scale):  # opaque ink square, mark scaled about the centre (16,16)
    t = 16 * (1 - scale)
    return svg("0 0 32 32", 32, 32, f'<rect width="32" height="32" fill="{INK}"/>'
               f'<g transform="translate({t:g} {t:g}) scale({scale:g})">{mark_body(PAPER, AMBER_ON_DARK)}</g>')
png(square(0.75), f"{OUT}/pyrlyn-apple-touch-icon.png", 180, 180)
png(square(0.625), f"{OUT}/pyrlyn-github-avatar-1000.png", 1000, 1000)
print("lockup W", W)
