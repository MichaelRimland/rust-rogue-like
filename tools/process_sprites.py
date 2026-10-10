#!/usr/bin/env python3
"""Turns raw AI-generated images into game-ready pixel sprites.

For every PNG in the input directory the script
  1. removes the flat magenta background,
  2. crops to the visible content and centers it on a square canvas,
  3. scales it down to the target size,
  4. snaps every pixel to the fixed game palette (tools/palette.hex),
  5. adds a 1 px near-black outline.

The palette snap is what makes sprites from different generations look related.

Usage:
    pip install pillow
    python tools/process_sprites.py assets/raw assets/sprites --size ogre=48 --size missile=16
"""

import argparse
import re
import sys
from pathlib import Path

from PIL import Image

HEX_COLOR = re.compile(r"#[0-9a-fA-F]{6}")
DEFAULT_PALETTE = Path(__file__).with_name("palette.hex")
BACKGROUND_KEY = (255, 0, 255)
# Pixels at most this far (per channel sum of squares) from the key count as background.
BACKGROUND_TOLERANCE = 110
ALPHA_THRESHOLD = 128
# Empty border kept around the sprite inside its canvas, in pixels of the final size.
MARGIN = 1


def load_palette(path: Path) -> list[tuple[int, int, int]]:
    colors = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        # Color lines are exactly "#rrggbb"; everything else is a comment.
        if not HEX_COLOR.fullmatch(line):
            continue
        colors.append(tuple(int(line[i : i + 2], 16) for i in (1, 3, 5)))
    if not colors:
        sys.exit(f"no colors found in {path}")
    return colors


def remove_background(image: Image.Image) -> Image.Image:
    image = image.convert("RGBA")
    limit = BACKGROUND_TOLERANCE**2
    kr, kg, kb = BACKGROUND_KEY
    pixels = image.load()
    for y in range(image.height):
        for x in range(image.width):
            r, g, b, _ = pixels[x, y]
            if (r - kr) ** 2 + (g - kg) ** 2 + (b - kb) ** 2 <= limit:
                pixels[x, y] = (r, g, b, 0)
    return image


def fit_to_canvas(image: Image.Image, size: int) -> Image.Image:
    box = image.getchannel("A").getbbox()
    if box is None:
        sys.exit("image is empty after background removal; is the background really magenta?")
    image = image.crop(box)

    inner = size - 2 * MARGIN
    scale = inner / max(image.size)
    new_size = (max(1, round(image.width * scale)), max(1, round(image.height * scale)))
    image = image.resize(new_size, Image.LANCZOS)

    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.paste(image, ((size - new_size[0]) // 2, (size - new_size[1]) // 2))
    return canvas


def snap_to_palette(image: Image.Image, palette: list[tuple[int, int, int]]) -> Image.Image:
    cache: dict[tuple[int, int, int], tuple[int, int, int]] = {}

    def nearest(rgb: tuple[int, int, int]) -> tuple[int, int, int]:
        if rgb not in cache:
            cache[rgb] = min(
                palette,
                key=lambda c: (c[0] - rgb[0]) ** 2 + (c[1] - rgb[1]) ** 2 + (c[2] - rgb[2]) ** 2,
            )
        return cache[rgb]

    pixels = image.load()
    for y in range(image.height):
        for x in range(image.width):
            r, g, b, a = pixels[x, y]
            if a < ALPHA_THRESHOLD:
                pixels[x, y] = (0, 0, 0, 0)
            else:
                pixels[x, y] = (*nearest((r, g, b)), 255)
    return image


def add_outline(image: Image.Image, color: tuple[int, int, int]) -> Image.Image:
    width, height = image.size
    source = image.load()
    result = image.copy()
    target = result.load()
    for y in range(height):
        for x in range(width):
            if source[x, y][3] != 0:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < width and 0 <= ny < height and source[nx, ny][3] != 0:
                    target[x, y] = (*color, 255)
                    break
    return result


def parse_sizes(entries: list[str]) -> dict[str, int]:
    sizes = {}
    for entry in entries:
        name, _, value = entry.partition("=")
        if not value.isdigit():
            sys.exit(f"--size expects NAME=PIXELS, got {entry!r}")
        sizes[name] = int(value)
    return sizes


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("input", type=Path, help="directory with raw PNGs (magenta background)")
    parser.add_argument("output", type=Path, help="directory for the finished sprites")
    parser.add_argument("--palette", type=Path, default=DEFAULT_PALETTE)
    parser.add_argument("--default-size", type=int, default=32, help="canvas size in px (default 32)")
    parser.add_argument(
        "--size",
        action="append",
        default=[],
        metavar="NAME=PX",
        help="override the canvas size for one sprite, e.g. ogre=48 (repeatable)",
    )
    parser.add_argument("--no-outline", action="store_true", help="skip the 1 px outline")
    args = parser.parse_args()

    palette = load_palette(args.palette)
    outline = palette[0]
    sizes = parse_sizes(args.size)

    files = sorted(args.input.glob("*.png"))
    if not files:
        sys.exit(f"no PNG files in {args.input}")
    args.output.mkdir(parents=True, exist_ok=True)

    for path in files:
        size = sizes.get(path.stem, args.default_size)
        image = remove_background(Image.open(path))
        image = fit_to_canvas(image, size)
        image = snap_to_palette(image, palette)
        if not args.no_outline:
            image = add_outline(image, outline)
        destination = args.output / path.name
        image.save(destination)
        print(f"{path.name} -> {destination} ({size}x{size})")


if __name__ == "__main__":
    main()
