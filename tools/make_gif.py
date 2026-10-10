#!/usr/bin/env python3
"""Assembles a frame sequence (see `--capture-dir` of the autoplay tool) into an animation.

The output format follows the file extension: `.webp` is much smaller than `.gif` for
noisy game footage and GitHub shows it in READMEs.

Usage:
    python tools/make_gif.py FRAMES_DIR docs/gameplay.webp [--width 800] [--fps 12] [--colors 96] [--crop 0.75]
"""

import argparse
from pathlib import Path

from PIL import Image


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("frames", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--width", type=int, default=720)
    parser.add_argument("--fps", type=int, default=12)
    parser.add_argument("--colors", type=int, default=96)
    parser.add_argument(
        "--crop",
        type=float,
        default=0.72,
        help="keep only this central fraction of the frame (0.7 = middle 70%%), to save size",
    )
    args = parser.parse_args()

    paths = sorted(args.frames.glob("frame_*.png"))
    if not paths:
        raise SystemExit(f"no frame_*.png files in {args.frames}")

    frames = []
    for path in paths:
        image = Image.open(path).convert("RGB")
        if args.crop < 1.0:
            w, h = image.size
            cw, ch = round(w * args.crop), round(h * args.crop)
            left, top = (w - cw) // 2, (h - ch) // 2
            image = image.crop((left, top, left + cw, top + ch))
        height = round(image.height * args.width / image.width)
        image = image.resize((args.width, height), Image.LANCZOS)
        if args.output.suffix.lower() == ".gif":
            image = image.quantize(colors=args.colors, method=Image.MEDIANCUT, dither=Image.NONE)
        frames.append(image)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    options = {"quality": 75, "method": 6} if args.output.suffix.lower() == ".webp" else {}
    frames[0].save(
        args.output,
        save_all=True,
        append_images=frames[1:],
        duration=round(1000 / args.fps),
        loop=0,
        optimize=True,
        **options,
    )
    size = args.output.stat().st_size / 1_000_000
    print(f"wrote {args.output} ({len(frames)} frames, {size:.1f} MB)")


if __name__ == "__main__":
    main()
