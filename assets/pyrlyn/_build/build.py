"""Pyrlyn 'Monolith P' production build. 32x32 grid, 2 units = 1px at 16px."""
import os, sys, cairosvg
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.recordingPen import RecordingPen
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
os.makedirs(OUT, exist_ok=True)
INK, PAPER = "#0C0E11", "#F4F2ED"
EMBER_ON_DARK, EMBER_ON_LIGHT = "#FF5A36", "#E2431E"
# --- mark (all integer, even = whole pixels at 16px) ---
# stem x6-12, bowl y4-22 (outer r=9, inner r=3, centre 17,13), stroke 6; cursor 6x4 at x16 y24
P = "M6 4H17A9 9 0 0 1 17 22H12V28H6ZM12 10V16H17A3 3 0 0 0 17 10Z"
CUR = (16, 24, 6, 4)
def mark_body(fg, acc):
    x, y, w, h = CUR
    if fg == acc:  # mono: one path
        return f'<path fill="{fg}" fill-rule="evenodd" d="{P}M{x} {y}h{w}v{h}h-{w}z"/>'
    return (f'<path fill="{fg}" fill-rule="evenodd" d="{P}"/>'
            f'<path fill="{acc}" d="M{x} {y}h{w}v{h}h-{w}z"/>')
# --- wordmark: Inter wght 560 opsz 32, outlined; l top = mark top (y4), baseline = bowl foot (y22)
FONT = os.environ.get("INTER_VF", "/usr/share/fonts/truetype/sand-box/google/Inter/Inter-VariableFont_opsz,wght.ttf")
f = instancer.instantiateVariableFont(TTFont(FONT), {"wght": 600, "opsz": 32})
gs, cmap, hmtx = f.getGlyphSet(), f.getBestCmap(), f["hmtx"]
BASE, TOP = 22, 4
s = (BASE - TOP) / f["OS/2"].sCapHeight           # 'l' ascender == cap height in Inter
TRACK = -16                                        # font units, display-size tracking
KERN = {"py": -58, "yr": -14, "rl": -34, "ly": -22, "yn": -18}  # hand pass, font units
def wordmark(x0):
    pen = SVGPathPen(gs, ntos=lambda v: ("%.2f" % v).rstrip("0").rstrip("."))
    x = x0; text = "pyrlyn"; ink = []
    for i, ch in enumerate(text):
        g = cmap[ord(ch)]
        gs[g].draw(TransformPen(pen, (s, 0, 0, -s, x, BASE)))
        if i < len(text) - 1:
            x += (hmtx[g][0] + TRACK + KERN.get(text[i:i+2], 0)) * s
    last = cmap[ord("n")]
    return pen.getCommands(), x0 + hmtx[cmap[ord("p")]][1] * s, x + 1047.56 * s
GAP = 8  # mark-to-wordmark: 8 units ≈ 1.33 stroke, optically equal to p's counter width
def svg(vb_w, vb_h, body, w=None, h=None, vb_x=0):
    wh = f' width="{w}" height="{h}"' if w else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb_x} 0 {vb_w} {vb_h}"{wh}>{body}</svg>\n'
def lockup_body(fg, acc):
    # mark occupies x6..26; shift so it starts at x=0 via path offsets? keep integers: build shifted path
    d = "M0 4H11A9 9 0 0 1 11 22H6V28H0ZM6 10V16H11A3 3 0 0 0 11 10Z"
    x, y, w, h = CUR
    wm_d, ink_l, ink_r = wordmark(0)
    shift = 20 + GAP - ink_l
    wm_d, ink_l, ink_r = wordmark(shift)
    W = round(ink_r + 0.0)
    if fg == acc:  # mono: one path
        return f'<path fill="{fg}" d="{d}M{x-6} {y}h{w}v{h}h-{w}z{wm_d}"/>', W
    body = (f'<path fill="{fg}" fill-rule="evenodd" d="{d}"/>'
            f'<path fill="{acc}" d="M{x-6} {y}h{w}v{h}h-{w}z"/>'
            f'<path fill="{fg}" d="{wm_d}"/>')
    return body, W
res = {}
for name, fg, acc in (("on-dark", PAPER, EMBER_ON_DARK), ("on-light", INK, EMBER_ON_LIGHT), ("mono", "currentColor", "currentColor")):
    m = svg(32, 32, mark_body(fg, acc), 32, 32)
    b, W = lockup_body(fg, acc)
    # lockup viewBox: 0..W x 4..28 cropped to ink height with 0 padding (use 0 4 W 24)
    l = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 2 {W+4} 28" width="{(W+4)*4}" height="112">{b}</svg>\n'
    open(f"{OUT}/pyrlyn-mark-{name}.svg", "w").write(m)
    open(f"{OUT}/pyrlyn-lockup-{name}.svg", "w").write(l)
    res[name] = (m, l, W)
    if name != "mono":
        cairosvg.svg2png(bytestring=m.encode(), write_to=f"{OUT}/pyrlyn-mark-{name}-1000.png", output_width=1000, output_height=1000)
        cairosvg.svg2png(bytestring=l.encode(), write_to=f"{OUT}/pyrlyn-lockup-{name}-2000.png", output_width=2000, output_height=round(2000*28/(W+4)))
print("lockup W", res["on-dark"][2], "scale", s, "xheight", f["OS/2"].sxHeight*s)
