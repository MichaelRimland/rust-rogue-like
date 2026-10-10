#!/usr/bin/env python3
"""Draws the game's pixel-art sprites with the fixed palette (tools/palette.hex).

The sprites are authored as character grids so they are reviewable in a diff and
reproducible. Most characters are drawn as the *left half* and mirrored; the mirrored
half uses a darker tone ramp so the light always comes from the top-left.

Usage:
    pip install pillow
    python tools/draw_sprites.py [output_dir]      # default: assets/sprites
    python tools/draw_sprites.py --preview sheet.png

AI-generated images can replace any of these files; run them through
tools/process_sprites.py so they end up in the same palette.
"""

import argparse
import sys
from pathlib import Path

from PIL import Image

TOOLS = Path(__file__).parent
ROOT = TOOLS.parent
SIZE = 32
SMALL = 16


def load_palette() -> list[tuple[int, int, int]]:
    colors = []
    for line in (TOOLS / "palette.hex").read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if len(line) == 7 and line.startswith("#") and " " not in line:
            try:
                colors.append(tuple(int(line[i : i + 2], 16) for i in (1, 3, 5)))
            except ValueError:
                pass
    return colors


PALETTE = load_palette()
OUTLINE = PALETTE[0]


def rgb(hex_color: str) -> tuple[int, int, int]:
    color = tuple(int(hex_color[i : i + 2], 16) for i in (1, 3, 5))
    assert color in PALETTE, f"{hex_color} is not in the palette"
    return color


# --------------------------------------------------------------------------- helpers


class Canvas:
    def __init__(self, size: int):
        self.size = size
        self.pixels: dict[tuple[int, int], tuple[int, int, int]] = {}

    def put(self, x: int, y: int, color):
        if 0 <= x < self.size and 0 <= y < self.size:
            self.pixels[(x, y)] = color

    def image(self, outline: bool = True) -> Image.Image:
        image = Image.new("RGBA", (self.size, self.size), (0, 0, 0, 0))
        px = image.load()
        for (x, y), color in self.pixels.items():
            px[x, y] = (*color, 255)
        if outline:
            result = image.copy()
            target = result.load()
            for y in range(self.size):
                for x in range(self.size):
                    if px[x, y][3]:
                        continue
                    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                        nx, ny = x + dx, y + dy
                        if 0 <= nx < self.size and 0 <= ny < self.size and px[nx, ny][3]:
                            target[x, y] = (*OUTLINE, 255)
                            break
            return result
        return image


def draw_half_mirrored(canvas: Canvas, rows: list[str], colors: dict, darker: dict, top: int = 0):
    """Draws `rows` (left half, width = canvas.size / 2) and its mirror image."""
    half = canvas.size // 2
    for dy, row in enumerate(rows):
        assert len(row) == half, f"row {dy} has {len(row)} chars, expected {half}: {row!r}"
        for x, ch in enumerate(row):
            if ch == ".":
                continue
            canvas.put(x, top + dy, colors[ch])
            mirror_ch = darker.get(ch, ch)
            canvas.put(canvas.size - 1 - x, top + dy, colors[mirror_ch])


def draw_patch(canvas: Canvas, x0: int, y0: int, rows: list[str], colors: dict):
    for dy, row in enumerate(rows):
        for dx, ch in enumerate(row):
            if ch != ".":
                canvas.put(x0 + dx, y0 + dy, colors[ch])


def stride(canvas: Canvas, y_start: int, y_end: int) -> Canvas:
    """Second walk frame: left half of the rows moves down 1 px, right half up 1 px."""
    result = Canvas(canvas.size)
    half = canvas.size // 2
    for (x, y), color in canvas.pixels.items():
        if y_start <= y <= y_end:
            y = y + 1 if x < half else y - 1
        result.put(x, y, color)
    return result


def ellipse(canvas: Canvas, cx: float, cy: float, rx: float, ry: float, ramp: list):
    """Filled ellipse shaded from the top-left; `ramp` goes dark -> light."""
    for y in range(canvas.size):
        for x in range(canvas.size):
            dx, dy = (x + 0.5 - cx) / rx, (y + 0.5 - cy) / ry
            if dx * dx + dy * dy <= 1.0:
                light = (-dx - dy) / 2.8 + 0.5  # 0 bottom-right .. 1 top-left
                index = min(len(ramp) - 1, max(0, int(light * len(ramp))))
                canvas.put(x, y, ramp[index])


# --------------------------------------------------------------------------- sprites


def wizard(frame: int) -> Image.Image:
    """A slim bearded wizard: tall pointed hat with a bent tip, glowing eyes, belt, robe."""
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#2c1a4d"), "b": rgb("#5b3a9e"), "c": rgb("#8f6af0"), "d": rgb("#c9a8ff"),
        "g": rgb("#a8a0ae"), "h": rgb("#7d7589"), "x": rgb("#d4cdd0"), "w": rgb("#f2ecdf"),
        "e": rgb("#62c9c0"), "m": rgb("#e0a830"), "n": rgb("#ffe27a"), "q": rgb("#1a1626"),
        "k": rgb("#7a4a2a"), "l": rgb("#b0763a"),
    }
    # The mirrored (right) half is only slightly darker so the figure keeps one color mass.
    darker = {"c": "b", "d": "c", "g": "h", "x": "g", "w": "x", "n": "m"}
    rows = [
        "...............b",  # 0 hat tip
        "...............b",
        "..............cb",
        "..............cb",
        ".............ccb",
        ".............cbb",
        "............ccbb",
        "............cbbb",
        "...........ccbbb",
        "...........cmmmm",  # 9 gold hat band
        "........cccccbbb",  # 10 brim
        "........aaaaaaaa",  # 11 brim shadow
        ".........aaggggg",  # 12 face
        ".........aageggg",  # 13 glowing eye
        ".........aaggxxx",  # 14 nose and beard
        "..........aaxxxx",  # 15 beard
        "........ccbaaxxx",  # 16 shoulders
        "........ccbbbxxw",
        "........cbbbbbww",  # 18 beard tip
        "........cmmmmmmn",  # 19 belt
        "........cbbbbbbb",
        "........cbbbbbab",
        ".......ccbbbbbbb",
        ".......ccbbabbbb",
        "......ccbbbbbbbb",
        "......ccbbbabbbb",
        ".....ccbbbbbbbbb",  # 26 robe flares out
        ".....cbbaabbbabb",
        ".....aabbaabbaab",  # 28 ragged hem
        ".........qqq....",  # 29 boots
        ".........qqq....",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=1)
    # Shade the outer right edge of the robe to give the body some volume.
    for y in range(18, 30):
        xs = [x for (x, py) in c.pixels if py == y]
        if xs:
            c.put(max(xs), y, colors["a"])
    # The tip of the hat flops over to the right.
    for x, y, ch in ((17, 1, "b"), (18, 1, "a"), (18, 2, "b"), (19, 2, "a")):
        c.put(x, y, colors[ch])
    # Staff next to the right hand with a glowing orb.
    for y in range(11, 32):
        c.put(26, y, colors["l"])
        c.put(27, y, colors["k"])
    draw_patch(c, 25, 5, [".dd.", "dddd", "dccd", ".cc."], colors)
    # The right arm reaches the staff; the left hand throws a small spark.
    for x, y, ch in ((23, 18, "b"), (24, 19, "b"), (24, 20, "a")):
        c.put(x, y, colors[ch])
    draw_patch(c, 24, 21, ["gg"], colors)
    draw_patch(c, 7, 21, ["g", "h"], colors)
    c.put(6, 19, colors["e"])
    c.put(5, 18, colors["d"])
    if frame == 1:
        c = stride(c, 30, 31)
    return c.image()


def goblin(frame: int) -> Image.Image:
    c = Canvas(SIZE)
    colors = {
        "e": rgb("#11201a"), "s": rgb("#2f5a3a"), "m": rgb("#4c8044"), "l": rgb("#86b04f"),
        "r": rgb("#4a2c1e"), "t": rgb("#7a4a2a"), "y": rgb("#ffe27a"), "w": rgb("#f2ecdf"),
        "k": rgb("#1a1626"), "b": rgb("#a8a0ae"), "d": rgb("#d4cdd0"), "h": rgb("#2b2438"),
    }
    darker = {"l": "m", "m": "s", "s": "e", "t": "r", "y": "y"}
    rows = [
        ".............lll",  # 9 head top
        "...........llmmm",
        ".....l..llmmmmmm",  # 11 ear tips
        "....lll.llmmmmmm",
        "....lllmmmmmmmmm",
        ".....ll.mmmyymmm",  # 14 eyes
        ".....s..mmmmmmmm",
        "........smmmwkkk",  # 16 mouth + teeth
        "........ssmmmmmm",
        "......mmttttttrr",  # 18 tunic
        "......mmtttttttt",
        "......mmttttrttt",
        ".......mttttttrr",
        "........tttttrrr",
        "........t.tt.trr",
        "..........sms...",  # 24 legs
        "..........sms...",
        ".........kkkk...",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=9)
    # Rusty dagger in the right hand.
    draw_patch(c, 24, 14, [".d.", ".b.", ".d.", ".b.", ".d.", "hhh", ".h.", "mm."], colors)
    if frame == 1:
        c = stride(c, 24, 26)
    return c.image()


def skeleton(frame: int) -> Image.Image:
    c = Canvas(SIZE)
    colors = {
        "w": rgb("#f2ecdf"), "x": rgb("#d4cdd0"), "y": rgb("#a8a0ae"), "z": rgb("#7d7589"),
        "e": rgb("#2b2438"), "t": rgb("#62c9c0"), "o": rgb("#b0763a"), "r": rgb("#7a4a2a"),
        "s": rgb("#4a2c1e"), "b": rgb("#a8a0ae"), "h": rgb("#4a2c1e"),
    }
    darker = {"w": "x", "x": "y", "y": "z", "o": "r", "r": "s"}
    rows = [
        "..........xwwwww",  # 7 skull
        ".........xwwwwww",
        "........xwwwwwww",
        "........xwwetewe",  # 10 eye sockets
        "........xwweeewe",
        "........xywwwwwe",
        "........yxwzwzwz",  # 13 teeth
        ".........yyxxxxx",
        "..............zy",  # 15 neck
        "........oorxxxxx",  # 16 pauldron + ribs
        "........rrswwwwx",
        "........seeeeeex",
        "........swwwwwwx",
        "........seeeeeex",
        ".......sssrrrrrr",
        "..........xwx...",  # 22 legs
        "..........xwx...",
        "..........yzy...",
        "..........xwx...",
        ".........xxxxx..",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=7)
    # A notched, rusty sword.
    draw_patch(c, 25, 9, ["b", "b", "o", "b", "b", "o", "b", "b", "o", "b", "b", "h", "h"], colors)
    c.put(24, 21, colors["h"])
    c.put(26, 21, colors["h"])
    draw_patch(c, 24, 19, ["xx"], colors)
    # Hanging bone arm on the left, lit side.
    for y in range(18, 27):
        c.put(7, y, colors["x"])
    draw_patch(c, 6, 27, ["xw"], colors)
    c.put(8, 18, colors["y"])
    if frame == 1:
        c = stride(c, 22, 26)
    return c.image()


def ogre(frame: int) -> Image.Image:
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#4a2c1e"), "b": rgb("#7a4a2a"), "c": rgb("#b0763a"), "d": rgb("#d9a65a"),
        "p": rgb("#2b2438"), "q": rgb("#3d3550"), "r": rgb("#c02f2f"), "R": rgb("#ec6a4a"),
        "w": rgb("#f2ecdf"), "y": rgb("#a8a0ae"), "k": rgb("#4a2c1e"), "j": rgb("#7a4a2a"),
        "z": rgb("#7d7589"), "v": rgb("#1a1626"),
    }
    darker = {"d": "c", "c": "b", "b": "a", "q": "p", "R": "r", "w": "y"}
    rows = [
        ".........ccbbbbb",  # 4 head
        "........cccbbbbb",
        "........cbbRrbbb",  # 6 eyes
        "........cbbbbbbb",
        "........cbwbbbwb",  # 8 tusks
        ".........abbaaaa",
        ".....cccbbbbbbbb",  # 10 shoulders
        "....cccbabbbbbbb",
        "...ccbbbabbbbppb",
        "...cbbbbabbbppqb",
        "...cbbbbabbbpqqb",
        "...cbbbbabbbbbbb",
        "..ccbbbbabbbbbbb",
        "..ccbbbbabbbbbbb",
        "...pppppppppqqqy",  # 18 belt
        "..cbbbbbbbbbbbbb",
        "....pppppppppppp",  # 20 loincloth
        "....ppqppppqpppp",
        "......cbbba.....",  # 22 legs
        "......cbbba.....",
        "......cbbba.....",
        "....aaaaaaa.....",
        "....aaaaaaa.....",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=4)
    # Spiked club in the right hand.
    for y in range(3, 25):
        c.put(28, y, colors["j"])
        c.put(29, y, colors["k"])
    for y in range(4, 12):
        c.put(27, y, colors["j"])
        c.put(30, y, colors["k"])
    for y in (4, 7, 10):
        c.put(26, y, colors["y"])
        c.put(31, y, colors["z"])
    draw_patch(c, 25, 17, ["cc", "bb"], colors)
    if frame == 1:
        c = stride(c, 22, 26)
    return c.image()


def draw_hooded_body(c: Canvas, colors: dict, darker: dict):
    """A short hooded figure; shared by the archer and the warlock."""
    rows = [
        "..............bb",  # 6 hood tip
        ".............cbb",
        "............ccbb",
        "...........ccbbb",
        "..........ccbbbb",
        "..........cbiiii",  # 11 hood opening
        "..........cbiffg",  # bone mask
        "..........cbifeg",  # glowing eye
        "..........cbighh",
        ".........ccbbiii",
        "........ccbbbbbb",  # 16 shoulders
        ".......ccbbbbbbb",
        ".......cbbbbbbbb",
        "......ccbbbbbbbb",
        "......cbbbbbbbbb",
        "......ccbbbbabbb",
        ".....ccbbbbbabbb",
        ".....ccbbbbbbbbb",
        ".....cbbbabbbbbb",
        ".....aabbaabbaab",  # 25 ragged hem
        "........qqq.....",  # 26 boots
        "........qqq.....",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=6)


def archer(frame: int) -> Image.Image:
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#3a0f1a"), "b": rgb("#7a1c26"), "c": rgb("#c02f2f"), "d": rgb("#ec6a4a"),
        "f": rgb("#d4cdd0"), "g": rgb("#a8a0ae"), "h": rgb("#7d7589"), "i": rgb("#2b2438"),
        "e": rgb("#ffe27a"), "q": rgb("#1a1626"), "k": rgb("#7a4a2a"), "j": rgb("#4a2c1e"),
        "x": rgb("#d4cdd0"),
    }
    darker = {"c": "b", "b": "a", "d": "c", "f": "g", "g": "h"}
    draw_hooded_body(c, colors, darker)
    # Short bow in the right hand: wooden limb on the right, bone string on the left.
    for x, y in ((27, 11), (28, 12), (29, 13), (29, 14), (30, 15), (30, 16), (30, 17),
                 (30, 18), (30, 19), (29, 20), (29, 21), (28, 22), (27, 23)):
        c.put(x, y, colors["k"])
    for y in range(12, 23):
        c.put(27, y, colors["x"])
    draw_patch(c, 25, 17, ["ff"], colors)
    if frame == 1:
        c = stride(c, 26, 27)
    return c.image()


def warlock(frame: int) -> Image.Image:
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#123a44"), "b": rgb("#2a7a80"), "c": rgb("#62c9c0"), "d": rgb("#c9a8ff"),
        "f": rgb("#5b3a9e"), "g": rgb("#2c1a4d"), "h": rgb("#1a1626"), "i": rgb("#0d0b14"),
        "e": rgb("#c9a8ff"), "q": rgb("#1a1626"), "p": rgb("#8f6af0"), "j": rgb("#4a2c1e"),
    }
    darker = {"c": "b", "b": "a", "d": "c", "f": "g", "g": "h"}
    draw_hooded_body(c, colors, darker)
    # A crooked staff topped by a floating orb.
    for y in range(14, 29):
        c.put(28, y, colors["j"])
    draw_patch(c, 26, 8, [".pp.", "pddp", "pddp", ".pp."], colors)
    draw_patch(c, 25, 17, ["ff"], colors)
    if frame == 1:
        c = stride(c, 26, 27)
    return c.image()


def bat(frame: int) -> Image.Image:
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#2c1a4d"), "b": rgb("#5b3a9e"), "i": rgb("#2b2438"), "h": rgb("#3d3550"),
        "r": rgb("#ec6a4a"), "w": rgb("#f2ecdf"),
    }
    darker = {"b": "a", "h": "i"}
    rows = [
        "............i.ii",  # ears
        "...........ihhhh",
        ".....bbbb..ihrhh",  # eyes, wing tips
        "...bbbbbbbbihhhh",
        "..bbbbbbbbbbhwhh",  # fangs
        "..abbbbbbbbahhhh",
        "..a.aabbaa..hhhh",  # ragged wing edge
        "......a..a...hhh",
        "..............ii",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=11)
    if frame == 1:
        c = stride(c, 12, 20)
    return c.image()


def enemy_bolt() -> Image.Image:
    c = Canvas(SMALL)
    ramp = [rgb("#3a0f1a"), rgb("#7a1c26"), rgb("#c02f2f"), rgb("#ec6a4a"), rgb("#ffe27a")]
    ellipse(c, 8, 8, 5, 5, ramp)
    return c.image()


def missile() -> Image.Image:
    c = Canvas(SMALL)
    colors = {
        "a": rgb("#2c1a4d"), "b": rgb("#5b3a9e"), "c": rgb("#8f6af0"),
        "d": rgb("#c9a8ff"), "e": rgb("#f2ecdf"),
    }
    top = ["........bbb.....", "...aabbbccccb...", "..aabbcccddddc.."]
    for i, row in enumerate(top):
        draw_patch(c, 0, 5 + i, [row], colors)
        draw_patch(c, 0, 10 - i, [row], colors)
    draw_patch(c, 0, 8, ["..aabbcccddddc.."], colors)
    c.put(13, 7, colors["e"])
    c.put(13, 8, colors["e"])
    return c.image(outline=False)


def gem(half_w: int, half_h: int, ramp: list) -> Image.Image:
    c = Canvas(SMALL)
    ellipse_diamond(c, 8, 8, half_w, half_h, ramp)
    return c.image()


def ellipse_diamond(c: Canvas, cx: int, cy: int, half_w: int, half_h: int, ramp: list):
    for y in range(c.size):
        for x in range(c.size):
            dx, dy = (x + 0.5 - cx) / half_w, (y + 0.5 - cy) / half_h
            if abs(dx) + abs(dy) <= 1.0:
                light = (-dx - dy) / 2.0 + 0.5
                index = min(len(ramp) - 1, max(0, int(light * len(ramp))))
                c.put(x, y, ramp[index])
    # A bright highlight pixel at the upper left.
    c.put(cx - 1, cy - max(1, half_h // 2), ramp[-1])


def rune() -> Image.Image:
    c = Canvas(SMALL)
    gold = [rgb("#8a5a12"), rgb("#e0a830"), rgb("#ffe27a")]
    ellipse(c, 8, 8, 6, 6, gold)
    dark = rgb("#1a1626")
    for x, y in ((8, 4), (8, 5), (8, 6), (8, 7), (8, 8), (8, 9), (8, 10), (6, 6), (7, 7), (10, 6), (9, 7)):
        c.put(x, y, dark)
    return c.image()


def ground() -> Image.Image:
    """A seamless forest floor tile: dark moss with sparse lighter specks and tufts."""
    c = Canvas(SIZE)
    base = rgb("#11201a")
    speck = [rgb("#1d3a2b"), rgb("#2f5a3a")]
    c.pixels = {(x, y): base for x in range(SIZE) for y in range(SIZE)}
    # A fixed, hand-picked scatter keeps the tile identical on every run.
    for x, y in ((3, 5), (11, 2), (19, 9), (27, 4), (7, 14), (23, 17), (14, 21), (30, 26),
                 (2, 27), (9, 30), (20, 29), (16, 12), (28, 12), (5, 20), (12, 8), (25, 22)):
        c.put(x, y, speck[0])
    for x, y in ((8, 18), (22, 6), (17, 25), (29, 15), (4, 10), (13, 28)):
        c.put(x, y, speck[1])
        c.put(x + 1, y, speck[0])
    # A few darker patches for depth.
    dark = rgb("#0d0b14")
    for x, y in ((10, 11), (11, 11), (24, 27), (25, 27), (6, 3), (18, 16), (19, 16)):
        c.put(x, y, dark)
    return c.image(outline=False)


def decor_bush() -> Image.Image:
    c = Canvas(SMALL)
    ramp = [rgb("#11201a"), rgb("#1d3a2b"), rgb("#2f5a3a")]
    ellipse(c, 6, 9, 5, 4, ramp)
    ellipse(c, 10, 8, 4, 4, ramp)
    ellipse(c, 8, 6, 4, 3, ramp)
    return c.image(outline=False)


def decor_stone() -> Image.Image:
    c = Canvas(SMALL)
    ellipse(c, 8, 9, 6, 4, [rgb("#2b2438"), rgb("#3d3550"), rgb("#5a5266"), rgb("#7d7589")])
    c.put(5, 7, rgb("#1d3a2b"))  # moss
    c.put(6, 7, rgb("#2f5a3a"))
    return c.image(outline=False)


def decor_bones() -> Image.Image:
    c = Canvas(SMALL)
    bone, shade = rgb("#d4cdd0"), rgb("#7d7589")
    for x in range(3, 13):
        c.put(x, 8, bone)
        c.put(x, 9, shade)
    for x, y in ((2, 7), (2, 9), (3, 7), (3, 9), (12, 7), (12, 9), (13, 7), (13, 9)):
        c.put(x, y, bone)
    for x in range(5, 11):
        c.put(x, 12, shade)
    c.put(4, 11, shade)
    c.put(11, 11, shade)
    return c.image(outline=False)


def decor_mushroom() -> Image.Image:
    c = Canvas(SMALL)
    ellipse(c, 8, 8, 5, 3, [rgb("#3a0f1a"), rgb("#7a1c26"), rgb("#c02f2f")])
    for x, y in ((6, 7), (9, 6), (10, 8)):
        c.put(x, y, rgb("#f2ecdf"))
    for y in range(10, 14):
        c.put(7, y, rgb("#a8a0ae"))
        c.put(8, y, rgb("#7d7589"))
    return c.image(outline=False)


def decor_tuft() -> Image.Image:
    c = Canvas(SMALL)
    dark, mid, light = rgb("#1d3a2b"), rgb("#2f5a3a"), rgb("#4c8044")
    for x, height in ((4, 4), (6, 7), (8, 9), (10, 6), (12, 4)):
        for y in range(13 - height, 13):
            c.put(x, y, mid if y > 13 - height else light)
        c.put(x + 1, 12, dark)
    return c.image(outline=False)


SPRITES = {
    "wizard_0": lambda: wizard(0),
    "wizard_1": lambda: wizard(1),
    "goblin_0": lambda: goblin(0),
    "goblin_1": lambda: goblin(1),
    "skeleton_0": lambda: skeleton(0),
    "skeleton_1": lambda: skeleton(1),
    "ogre_0": lambda: ogre(0),
    "ogre_1": lambda: ogre(1),
    "archer_0": lambda: archer(0),
    "archer_1": lambda: archer(1),
    "bat_0": lambda: bat(0),
    "bat_1": lambda: bat(1),
    "warlock_0": lambda: warlock(0),
    "warlock_1": lambda: warlock(1),
    "ground": ground,
    "missile": missile,
    "enemy_bolt": enemy_bolt,
    "gem_small": lambda: gem(4, 5, [rgb("#2a7a80"), rgb("#62c9c0"), rgb("#f2ecdf"), rgb("#f2ecdf")]),
    "gem_medium": lambda: gem(5, 6, [rgb("#e0a830"), rgb("#ffe27a"), rgb("#f2ecdf"), rgb("#f2ecdf")]),
    "gem_large": lambda: gem(6, 7, [rgb("#5b3a9e"), rgb("#8f6af0"), rgb("#c9a8ff"), rgb("#f2ecdf")]),
    "rune": rune,
    "decor_bush": decor_bush,
    "decor_stone": decor_stone,
    "decor_bones": decor_bones,
    "decor_mushroom": decor_mushroom,
    "decor_tuft": decor_tuft,
}


def contact_sheet(images: dict[str, Image.Image], scale: int = 8) -> Image.Image:
    background = (24, 34, 28, 255)
    cell = SIZE * scale
    columns = 6
    rows = (len(images) + columns - 1) // columns
    sheet = Image.new("RGBA", (columns * cell, rows * cell), background)
    for index, image in enumerate(images.values()):
        tile = Image.new("RGBA", image.size, background)
        tile.alpha_composite(image)
        tile = tile.resize((image.width * scale, image.height * scale), Image.NEAREST)
        x = (index % columns) * cell + (cell - tile.width) // 2
        y = (index // columns) * cell + (cell - tile.height) // 2
        sheet.paste(tile, (x, y))
    return sheet


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("output", nargs="?", type=Path, default=ROOT / "assets" / "sprites")
    parser.add_argument("--preview", type=Path, help="also write an enlarged contact sheet")
    args = parser.parse_args()

    images = {name: build() for name, build in SPRITES.items()}
    args.output.mkdir(parents=True, exist_ok=True)
    for name, image in images.items():
        image.save(args.output / f"{name}.png")
    print(f"wrote {len(images)} sprites to {args.output}")
    if args.preview:
        contact_sheet(images).save(args.preview)
        print(f"preview: {args.preview}")


if __name__ == "__main__":
    sys.exit(main())
