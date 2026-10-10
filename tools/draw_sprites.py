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
import math
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


def shift(canvas: Canvas, dy: int) -> Canvas:
    result = Canvas(canvas.size)
    for (x, y), color in canvas.pixels.items():
        result.put(x, y + dy, color)
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
        "..........xwwwww",  # 0 skull
        ".........xwwwwww",
        "........xwwwwwww",
        "........xwwetewe",  # 3 eye sockets
        "........xwweeewe",
        "........xywwwwwe",
        "........yxwzwzwz",  # 6 teeth
        ".........yyxxxxx",
        "..............zy",  # 8 neck
        "........oorxxxxx",  # 9 pauldron and collar bone
        "........rrswwwwx",
        ".........seeeeex",  # 11 ribs
        ".........swwwwwx",
        ".........seeeeex",
        ".........swwwwwx",
        "..........sxrrrr",  # 15 narrow pelvis with a rag
        "..........xwx...",  # 16 legs
        "..........xwx...",
        "..........yzy...",
        "..........xwx...",
        ".........xxxxx..",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=5)
    # Arm bones hang from the shoulder pads; the right hand holds a notched, rusty sword.
    for y in range(15, 24):
        c.put(7, y, colors["x"])
    c.put(8, 14, colors["y"])
    draw_patch(c, 6, 24, ["xw"], colors)
    for y, ch in zip(range(7, 19), "bbobbobbobbh"):
        c.put(25, y, colors[ch])
    draw_patch(c, 24, 19, ["xx"], colors)
    c.put(24, 21, colors["h"])
    c.put(26, 21, colors["h"])
    # A crack across the skull and a missing tooth keep it from looking mirrored.
    for x, y in ((13, 8), (14, 9), (14, 10)):
        c.put(x, y, colors["z"])
    c.put(20, 11, colors["e"])
    if frame == 1:
        c = stride(c, 21, 25)
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
        "........w.......",  # 0 horn
        "........ww......",
        ".........wcbbbbb",  # 2 head
        "........wcccbbbb",
        "........cbbbbbbb",
        "........cbRrbbbb",  # 5 eyes
        "........cbbbbbbb",
        "........cbwbbbab",  # 7 tusk and mouth
        ".........cbaaaaa",
        "......cccbbbbbbb",  # 9 neck and shoulders
        "....cccccbbbbbbb",
        "...cccbbbbbbbbbb",
        "...ccbbbbbbppbbb",
        "..ccbbbbbbbbppqb",
        "..cbbbbbbbbbpqqb",
        "..cbbbaabbbbbbbb",
        "...cbbbabbbbbbbb",
        "...cbbbbbbbbbbbb",
        "...pppppppppqqqy",  # 18 belt
        "...pqpppppppqppp",  # 19 loincloth
        "....ppqppppqpppp",
        ".....cbbbbaa....",  # 21 legs
        ".....cbbbbba....",
        ".....cbbbbba....",
        "....aaaaaaaa....",  # 24 feet
        "....aaaaaaaa....",
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
    # Scars, a broken horn and a patch of stitched hide.
    for x, y in ((19, 9), (20, 10), (21, 11)):
        c.put(x, y, colors["a"])
    c.pixels.pop((23, 5), None)
    draw_patch(c, 20, 17, ["qq", "qp"], colors)
    if frame == 1:
        c = stride(c, 25, 29)
    return c.image()


def archer(frame: int) -> Image.Image:
    """A lean hooded ranger: short cloak, visible boots, bow and quiver."""
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#3a0f1a"), "b": rgb("#7a1c26"), "c": rgb("#c02f2f"), "d": rgb("#ec6a4a"),
        "f": rgb("#d4cdd0"), "g": rgb("#a8a0ae"), "h": rgb("#7d7589"), "i": rgb("#2b2438"),
        "e": rgb("#ffe27a"), "q": rgb("#1a1626"), "k": rgb("#4a2c1e"), "j": rgb("#7a4a2a"),
        "m": rgb("#b0763a"), "x": rgb("#d4cdd0"), "l": rgb("#a8a0ae"),
    }
    darker = {"c": "b", "b": "a", "d": "c", "f": "g", "g": "h", "m": "j"}
    rows = [
        "..............bb",  # 0 hood tip
        ".............cbb",
        "............ccbb",
        "...........cccbb",
        "..........ccbbbb",
        "..........cbiiii",  # 5 hood opening
        "..........cbiffg",  # bone mask
        "..........cbifeg",  # glowing eye
        "..........cbighh",
        ".........ccbbiii",
        "........ccbbbbbb",  # 10 shoulders
        "........cbbbbbbb",
        "........cbbbbmbb",  # belt strap
        "........cbbbbbbb",
        ".........cbbbbbb",  # 14 narrow waist
        ".........cbbabbb",
        "........ccbbabbb",  # 16 short cloak
        "........ccbbbbbb",
        "........aabbaabb",  # 18 ragged hem
        "..........kk....",  # 19 legs
        "..........kk....",
        ".........qqq....",  # 21 boots
    ]
    draw_half_mirrored(c, rows, colors, darker, top=5)
    # Short bow in the right hand: wooden limb on the right, bone string on the left.
    for x, y in ((27, 12), (28, 13), (29, 14), (29, 15), (30, 16), (30, 17), (30, 18),
                 (30, 19), (30, 20), (29, 21), (29, 22), (28, 23), (27, 24)):
        c.put(x, y, colors["j"])
    for y in range(13, 24):
        c.put(27, y, colors["x"])
    draw_patch(c, 25, 18, ["ff"], colors)
    # A quiver full of arrows on the back, and a scarf that trails to one side.
    draw_patch(c, 7, 18, ["e.", "kk", "kk", "kk", "k."], colors)
    c.put(8, 17, colors["x"])
    c.put(9, 16, colors["x"])
    for x, y in ((21, 17), (22, 18), (23, 19), (23, 20)):
        c.put(x, y, colors["c"])
    if frame == 1:
        c = stride(c, 24, 26)
    return c.image()


def king(frame: int) -> Image.Image:
    """The end boss: a crowned, hollow-eyed king in spiked pauldrons and a long cape."""
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#0d0b14"), "b": rgb("#2b2438"), "c": rgb("#5a5266"), "d": rgb("#a8a0ae"),
        "f": rgb("#d4cdd0"), "g": rgb("#a8a0ae"), "h": rgb("#7d7589"), "i": rgb("#0d0b14"),
        "e": rgb("#ec6a4a"), "q": rgb("#1a1626"), "m": rgb("#e0a830"), "n": rgb("#ffe27a"),
        "p": rgb("#7a1c26"), "r": rgb("#c02f2f"),
    }
    darker = {"c": "b", "b": "a", "d": "c", "f": "g", "g": "h", "n": "m", "r": "p"}
    rows = [
        ".........m.m.m.m",  # 0 crown spikes
        ".........mmmmmmm",
        ".........mnrnrnm",  # jewels
        "........abbbbbbb",  # 3 helmet and face
        "........abffffff",
        "........abffeffg",  # ember eye
        "........abffffgg",
        "........abfhfhfh",  # teeth
        ".......cabbbbbbb",  # 8 neck
        "....mmmmcbbbbbbb",  # 9 gold trimmed pauldrons
        "...mccccbbbbbbbb",
        "...mccbbbbbbbbbb",
        "...mcbbbbbbmbbbb",
        "....cbbbbbbbbbbb",
        ".....cbbbbbppppp",  # 14 red sash
        ".....cbbbbbbbbbb",
        "....ccbbbabbbbbb",
        "....ccbbbbbbabbb",
        "...ccbbbbabbbbbb",  # 18 cape flares out
        "...ccbbbbbbbbbbb",
        "..ccbbabbbbbabbb",
        "..cbbaabbbaabbbb",
        "..aabbaabbaaabbb",  # 22 ragged hem
        "........qqq.....",  # 23 boots
        "........qqq.....",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=3)
    # Spikes on the left pauldron and a crooked golden staff in the right hand.
    for x, y in ((6, 11), (5, 10), (4, 9), (7, 10)):
        c.put(x, y, colors["d"])
    for y in range(8, 29):
        c.put(27, y, colors["c"])
    draw_patch(c, 25, 5, ["m.m", "mmm", ".m."], colors)
    draw_patch(c, 25, 17, ["ff"], colors)
    # A cracked helmet: one dark line across the face.
    for x, y in ((13, 8), (14, 9), (15, 10)):
        c.put(x, y, colors["h"])
    if frame == 1:
        c = stride(c, 26, 27)
    return c.image()


def warlock(frame: int) -> Image.Image:
    """A floating caster: horned hood, wide sleeves, no feet, orbiting runes."""
    c = Canvas(SIZE)
    colors = {
        "a": rgb("#123a44"), "b": rgb("#2a7a80"), "c": rgb("#62c9c0"), "d": rgb("#c9a8ff"),
        "f": rgb("#5b3a9e"), "g": rgb("#2c1a4d"), "h": rgb("#1a1626"), "i": rgb("#0d0b14"),
        "e": rgb("#c9a8ff"), "w": rgb("#d4cdd0"), "p": rgb("#8f6af0"), "m": rgb("#e0a830"),
    }
    darker = {"c": "b", "b": "a", "d": "c", "f": "g", "w": "h"}
    rows = [
        "............w...",  # 0 horn
        "...........wc...",
        "..........ccbbbb",  # 2 hood
        ".........ccbbbbb",
        ".........cbiiiii",
        ".........cbiiiei",  # 5 glowing eye
        ".........cbbbiii",
        "........ccbbbbbb",
        ".......cccbbbbbb",  # 8 sleeves
        "......cccbbbbbbb",
        "......ccbbbbabbb",
        ".....ccbbbbbabbb",
        ".....cbbbbbbmbbb",  # 12 sash
        "......cbbbbbbbbb",
        "......cbbabbbbbb",
        ".......cbbbbabbb",
        ".......cbbbbbbbb",
        "........cbbbbabb",
        "........cbbabbbb",
        ".........cbbabbb",
        ".........cbbbbab",
        "..........cbbbbb",  # 21 robe tapers off, nothing below
        "...........cbbbb",
        "............cbbb",
        "..............bb",
    ]
    draw_half_mirrored(c, rows, colors, darker, top=3)
    # Two orbs float by the hands, a few runes circle the wizard.
    draw_patch(c, 4, 12, [".pp.", "pddp", "pddp", ".pp."], colors)
    draw_patch(c, 24, 9, [".pp.", "pddp", "pddp", ".pp."], colors)
    for x, y in ((3, 22), (28, 20), (7, 27), (25, 26), (2, 6), (29, 3)):
        c.put(x, y, colors["d"])
    for x, y in ((4, 23), (28, 21), (7, 28)):
        c.put(x, y, colors["c"])
    if frame == 1:
        c = shift(c, -1)
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


def portal(frame: int) -> Image.Image:
    """A stone archway with a swirling portal. The swirl is light on purpose: the game tints
    the whole sprite in the color of the room behind the door."""
    c = Canvas(SIZE)
    stone = [rgb("#2b2438"), rgb("#3d3550"), rgb("#5a5266"), rgb("#7d7589")]
    swirl = [rgb("#7d7589"), rgb("#a8a0ae"), rgb("#d4cdd0"), rgb("#f2ecdf")]
    centre_x, arch_y, hole_y = 15.5, 12.0, 13.0

    def in_arch(x: int, y: int) -> bool:
        if y >= arch_y:
            return 5 <= x <= 26 and y <= 30
        return math.hypot(x - centre_x, y - arch_y) <= 10.8

    def in_hole(x: int, y: int) -> bool:
        if y >= hole_y:
            return 10 <= x <= 21 and y <= 30
        return math.hypot(x - centre_x, y - hole_y) <= 6.2

    for y in range(SIZE):
        for x in range(SIZE):
            if not in_arch(x, y):
                continue
            if in_hole(x, y):
                # Swirl: bands that spiral around the centre of the opening.
                dx, dy = x - centre_x, y - 21.0
                angle = math.atan2(dy, dx)
                radius = math.hypot(dx, dy)
                value = math.sin(angle * 2.0 + radius * 0.55 - frame * 1.7) * 0.5 + 0.5
                value -= max(0.0, (radius - 6.0) * 0.08)  # darker towards the rim
                level = int(max(0.0, min(0.999, value)) * len(swirl))
                c.put(x, y, swirl[level])
            else:
                # Stone blocks: lighter on the upper left, mortar lines between rows.
                light = 3 if (x + y) % 9 == 0 else 2 if x < 14 else 1
                if y % 5 == 4 or (x + (y // 5) * 3) % 8 == 7:
                    light = 0
                c.put(x, y, stone[light])
    # A keystone at the top of the arch.
    for x, y in ((15, 2), (16, 2), (15, 3), (16, 3)):
        c.put(x, y, rgb("#d4cdd0"))
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


def ground_crypt() -> Image.Image:
    """Cold flagstones: offset brick rows with mortar lines, chips and cracks."""
    c = Canvas(SIZE)
    base, mortar, light = rgb("#2b2438"), rgb("#1a1626"), rgb("#3d3550")
    for y in range(SIZE):
        for x in range(SIZE):
            row = y // 8
            local_x = (x + (8 if row % 2 else 0)) % 16
            local_y = y % 8
            if local_y == 7 or local_x == 15:
                color = mortar
            elif local_y == 0 or local_x == 0:
                color = light
            else:
                color = base
            c.put(x, y, color)
    dark = rgb("#0d0b14")
    for x, y in ((5, 3), (6, 4), (7, 4), (21, 12), (22, 13), (12, 26), (13, 26), (14, 27)):
        c.put(x, y, dark)
    teal = rgb("#123a44")  # damp patches
    for x, y in ((18, 4), (19, 4), (3, 20), (4, 20), (4, 21), (26, 25)):
        c.put(x, y, teal)
    return c.image(outline=False)


def ground_inferno() -> Image.Image:
    """Scorched obsidian with veins of lava."""
    c = Canvas(SIZE)
    base, ash, deep = rgb("#1a1626"), rgb("#2b2438"), rgb("#3a0f1a")
    c.pixels = {(x, y): base for x in range(SIZE) for y in range(SIZE)}
    for x, y in ((4, 4), (20, 8), (11, 19), (26, 24), (7, 28), (28, 3)):
        for dx in range(2):
            for dy in range(2):
                c.put(x + dx, y + dy, deep)
    for x, y in ((14, 6), (24, 14), (3, 13), (17, 25), (9, 10), (29, 30)):
        c.put(x, y, ash)
    glow, core, hot = rgb("#7a1c26"), rgb("#c02f2f"), rgb("#ec6a4a")
    # Two short veins per tile keep the floor readable behind a crowd of enemies.
    veins = [
        [(3, 9), (4, 9), (5, 10), (6, 10), (7, 11), (8, 11), (9, 12)],
        [(19, 22), (20, 23), (21, 23), (22, 24), (23, 24)],
    ]
    for vein in veins:
        for x, y in vein:
            c.put(x, y + 1, glow)
        for x, y in vein:
            c.put(x, y, core)
    c.put(6, 10, hot)
    return c.image(outline=False)


def decor_skull() -> Image.Image:
    c = Canvas(SMALL)
    ellipse(c, 8, 7, 5, 4, [rgb("#7d7589"), rgb("#a8a0ae"), rgb("#d4cdd0"), rgb("#f2ecdf")])
    dark = rgb("#1a1626")
    for x, y in ((6, 6), (7, 6), (6, 7), (7, 7), (9, 6), (10, 6), (9, 7), (10, 7), (8, 9)):
        c.put(x, y, dark)
    for x in range(6, 11):
        c.put(x, 11, rgb("#a8a0ae") if x % 2 else dark)
    return c.image(outline=False)


def decor_pillar() -> Image.Image:
    c = Canvas(SMALL)
    ramp = [rgb("#2b2438"), rgb("#3d3550"), rgb("#5a5266"), rgb("#7d7589")]
    for y in range(4, 14):
        for x in range(5, 11):
            shade = 3 if x == 5 else 2 if x < 8 else 1 if x < 10 else 0
            c.put(x, y, ramp[shade])
    # A broken, jagged top.
    c.pixels.pop((6, 4), None)
    c.pixels.pop((10, 4), None)
    c.pixels.pop((5, 5), None)
    for x in range(4, 12):
        c.put(x, 13, rgb("#1a1626"))
    return c.image(outline=False)


def decor_candle() -> Image.Image:
    c = Canvas(SMALL)
    for y in range(9, 13):
        c.put(7, y, rgb("#f2ecdf"))
        c.put(8, y, rgb("#d4cdd0"))
    c.put(7, 8, rgb("#1a1626"))
    for x, y, color in ((7, 6, "#e0a830"), (7, 7, "#ffe27a"), (8, 7, "#e0a830"), (7, 5, "#ec6a4a")):
        c.put(x, y, rgb(color))
    for x in range(5, 11):
        c.put(x, 13, rgb("#5a5266"))
    return c.image(outline=False)


def decor_rubble() -> Image.Image:
    c = Canvas(SMALL)
    ramp = [rgb("#1a1626"), rgb("#2b2438"), rgb("#3d3550"), rgb("#5a5266")]
    ellipse(c, 5, 10, 3, 2, ramp)
    ellipse(c, 10, 9, 3, 3, ramp)
    ellipse(c, 8, 12, 2, 1.5, ramp)
    return c.image(outline=False)


def decor_ember() -> Image.Image:
    c = Canvas(SMALL)
    ellipse(c, 8, 9, 5, 4, [rgb("#0d0b14"), rgb("#1a1626"), rgb("#2b2438"), rgb("#3d3550")])
    for x, y in ((5, 8), (6, 9), (7, 9), (8, 10), (9, 10), (10, 9), (11, 8)):
        c.put(x, y, rgb("#c02f2f"))
    c.put(8, 9, rgb("#ec6a4a"))
    c.put(7, 8, rgb("#e0a830"))
    return c.image(outline=False)


def decor_obsidian() -> Image.Image:
    c = Canvas(SMALL)
    for cx, height in ((5, 6), (8, 10), (11, 5)):
        for i in range(height):
            y = 13 - i
            half = max(0, (height - i) // 4)
            for x in range(cx - half, cx + half + 1):
                shade = "#3d3550" if x <= cx else "#1a1626"
                c.put(x, y, rgb(shade))
        c.put(cx, 13 - height + 1, rgb("#7a1c26"))
    return c.image(outline=False)


def decor_lava() -> Image.Image:
    c = Canvas(SMALL)
    ellipse(c, 8, 9, 6, 3, [rgb("#7a1c26"), rgb("#c02f2f"), rgb("#ec6a4a"), rgb("#ffe27a")])
    return c.image(outline=False)


def decor_charred() -> Image.Image:
    c = Canvas(SMALL)
    bone, shade = rgb("#5a5266"), rgb("#2b2438")
    for x in range(3, 13):
        c.put(x, 8, bone)
        c.put(x, 9, shade)
    for x, y in ((2, 7), (2, 9), (3, 7), (3, 9), (12, 7), (12, 9), (13, 8)):
        c.put(x, y, bone)
    for x, y in ((5, 11), (6, 12), (9, 11), (10, 5)):
        c.put(x, y, rgb("#0d0b14"))
    c.put(8, 7, rgb("#7a1c26"))
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


# --------------------------------------------------------------------------- polish

CHARACTER_PREFIXES = ("wizard", "goblin", "skeleton", "ogre", "archer", "bat", "warlock", "king")
# Bright accent colors (eyes, gold, magic) are left alone by the texture pass.
PROTECTED = {
    rgb(c)
    for c in ("#e0a830", "#ffe27a", "#62c9c0", "#ec6a4a", "#c9a8ff", "#f2ecdf", "#8f6af0")
}


def nearest_palette(color: tuple[float, float, float]) -> tuple[int, int, int]:
    return min(
        PALETTE,
        key=lambda p: (p[0] - color[0]) ** 2 + (p[1] - color[1]) ** 2 + (p[2] - color[2]) ** 2,
    )


def luminance(color) -> float:
    return 0.3 * color[0] + 0.59 * color[1] + 0.11 * color[2]


def scatter(x: int, y: int, seed: int) -> int:
    """A deterministic pseudo-random number in 0..99 for a pixel."""
    return ((x * 73856093) ^ (y * 19349663) ^ (seed * 83492791)) % 100


def polish(image: Image.Image, seed: int) -> Image.Image:
    """Makes mirrored, flat-shaded sprites look more hand-made.

    - the hard seam where the two mirrored halves meet is dithered,
    - the black outline takes on a dark tone of the colors it surrounds,
    - the lit top-left edges get a rim highlight,
    - a sparse texture noise breaks up the large flat areas.
    """
    w, h = image.size
    half = w // 2
    result = image.copy()
    px = result.load()
    base = image.load()

    def opaque(x: int, y: int) -> bool:
        return 0 <= x < w and 0 <= y < h and base[x, y][3] > 0

    # 1. dither the seam between the mirrored halves
    for y in range(h):
        for x in (half - 1, half):
            if (x + y) % 2 == 0 and opaque(x, y) and opaque(w - 1 - x, y):
                px[x, y] = base[w - 1 - x, y]

    snapshot = result.copy()
    snap = snapshot.load()

    def solid(x: int, y: int) -> bool:
        return 0 <= x < w and 0 <= y < h and snap[x, y][3] > 0

    def is_edge(x: int, y: int) -> bool:
        return any(not solid(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))

    for y in range(h):
        for x in range(w):
            if not solid(x, y):
                continue
            color = snap[x, y][:3]
            if is_edge(x, y) and color == OUTLINE:
                # 2. outline in a dark shade of the neighbouring body color
                neighbours = [
                    snap[x + dx, y + dy][:3]
                    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))
                    if solid(x + dx, y + dy) and snap[x + dx, y + dy][:3] != OUTLINE
                ]
                if neighbours:
                    avg = [sum(c[i] for c in neighbours) / len(neighbours) for i in range(3)]
                    target = tuple(min(channel * 0.34, 62.0) for channel in avg)
                    px[x, y] = (*nearest_palette(target), 255)
                continue
            if color in PROTECTED or color == OUTLINE:
                continue
            roll = scatter(x, y, seed)
            lit_edge = not solid(x, y - 1) or not solid(x - 1, y) or (
                snap[x, y - 1][:3] == OUTLINE or snap[x - 1, y][:3] == OUTLINE
            )
            if lit_edge and roll < 70:
                # 3. rim light on the top-left edges
                target = tuple(min(255.0, channel * 1.22 + 14) for channel in color)
                lighter = nearest_palette(target)
                if luminance(lighter) > luminance(color):
                    px[x, y] = (*lighter, 255)
            elif roll < 5:
                # 4. texture noise: a few darker and lighter specks
                target = tuple(channel * 0.8 for channel in color)
                px[x, y] = (*nearest_palette(target), 255)
            elif roll < 9:
                target = tuple(min(255.0, channel * 1.18 + 6) for channel in color)
                lighter = nearest_palette(target)
                if luminance(lighter) > luminance(color):
                    px[x, y] = (*lighter, 255)
    return result


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
    "king_0": lambda: king(0),
    "king_1": lambda: king(1),
    "ground": ground,
    "ground_crypt": ground_crypt,
    "ground_inferno": ground_inferno,
    "decor_skull": decor_skull,
    "decor_pillar": decor_pillar,
    "decor_candle": decor_candle,
    "decor_rubble": decor_rubble,
    "decor_ember": decor_ember,
    "decor_obsidian": decor_obsidian,
    "decor_lava": decor_lava,
    "decor_charred": decor_charred,
    "portal_0": lambda: portal(0),
    "portal_1": lambda: portal(1),
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

    images = {}
    for index, (name, build) in enumerate(SPRITES.items()):
        image = build()
        if name.startswith(CHARACTER_PREFIXES):
            # Both walk frames use the same seed so the texture does not flicker.
            image = polish(image, seed=sum(name.rstrip("_01").encode()))
        images[name] = image
    args.output.mkdir(parents=True, exist_ok=True)
    for name, image in images.items():
        image.save(args.output / f"{name}.png")
    print(f"wrote {len(images)} sprites to {args.output}")
    if args.preview:
        contact_sheet(images).save(args.preview)
        print(f"preview: {args.preview}")


if __name__ == "__main__":
    sys.exit(main())
