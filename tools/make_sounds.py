#!/usr/bin/env python3
"""Synthesizes the game's sound effects and the ambient loop as 16-bit mono WAV files.

Everything is generated from simple oscillators and noise, so the sounds are original,
license-free and reproducible. Pure standard library, no dependencies.

Usage:
    python tools/make_sounds.py [output_dir]      # default: assets/sounds
"""

import math
import random
import struct
import sys
import wave
from pathlib import Path

RATE = 22050
ROOT = Path(__file__).resolve().parent.parent


def write_wav(path: Path, samples: list[float]) -> None:
    with wave.open(str(path), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        frames = b"".join(
            struct.pack("<h", int(max(-1.0, min(1.0, s)) * 32767)) for s in samples
        )
        out.writeframes(frames)


def tone(f0: float, f1: float, seconds: float, volume: float, shape: str = "sine",
         decay: float = 4.0) -> list[float]:
    """A tone gliding from f0 to f1 Hz with an exponential decay envelope."""
    count = int(RATE * seconds)
    rng = random.Random(7)
    out, phase = [], 0.0
    for i in range(count):
        t = i / count
        phase += 2 * math.pi * (f0 + (f1 - f0) * t) / RATE
        if shape == "sine":
            value = math.sin(phase)
        elif shape == "square":
            value = 1.0 if math.sin(phase) >= 0 else -1.0
        elif shape == "saw":
            value = (phase / math.pi) % 2.0 - 1.0
        else:  # noise
            value = rng.uniform(-1.0, 1.0)
        attack = min(1.0, i / (RATE * 0.004))
        out.append(value * volume * attack * math.exp(-decay * t))
    return out


def mix(*tracks: list[float]) -> list[float]:
    length = max(len(t) for t in tracks)
    return [sum(t[i] for t in tracks if i < len(t)) for i in range(length)]


def concat(*tracks: list[float]) -> list[float]:
    return [s for t in tracks for s in t]


def silence(seconds: float) -> list[float]:
    return [0.0] * int(RATE * seconds)


def smooth(samples: list[float], window: int) -> list[float]:
    """Crude low-pass: a moving average."""
    out, total = [], 0.0
    for i, s in enumerate(samples):
        total += s
        if i >= window:
            total -= samples[i - window]
        out.append(total / min(i + 1, window))
    return out


def ambient(seconds: int = 12) -> list[float]:
    """A dark drone: an open fifth with a slow tremolo. Frequencies are chosen so every
    oscillator completes a whole number of cycles, which makes the loop seamless."""
    count = RATE * seconds
    notes = [(110.0, 0.20), (165.0, 0.12), (262.5, 0.05), (55.0, 0.18)]
    out = []
    for i in range(count):
        t = i / RATE
        tremolo = 0.75 + 0.25 * math.sin(2 * math.pi * 0.25 * t)
        value = sum(a * math.sin(2 * math.pi * f * t) for f, a in notes)
        out.append(value * tremolo * 0.6)
    return out


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "assets" / "sounds"
    out_dir.mkdir(parents=True, exist_ok=True)
    rng = random.Random(1)

    sounds = {
        "shoot": tone(900, 420, 0.10, 0.22, "square", decay=5.0),
        "hit": smooth(tone(0, 0, 0.05, 0.9, "noise", decay=6.0), 3),
        "kill": mix(tone(320, 70, 0.18, 0.35, "sine", 4.5),
                    smooth(tone(0, 0, 0.12, 0.5, "noise", decay=7.0), 6)),
        "pickup": tone(660, 1320, 0.09, 0.22, "sine", decay=3.0),
        "levelup": concat(*[tone(f, f, 0.11, 0.3, "square", decay=2.5)
                            for f in (523.3, 659.3, 784.0, 1046.5)]),
        "hurt": mix(tone(150, 55, 0.25, 0.5, "saw", 4.0),
                    smooth(tone(0, 0, 0.12, 0.5, "noise", decay=8.0), 8)),
        "dash": smooth(tone(0, 0, 0.22, 0.9, "noise", decay=3.0), 14),
        "blink": mix(tone(1900, 260, 0.16, 0.3, "sine", 3.0),
                     tone(950, 130, 0.16, 0.2, "square", 5.0)),
        "clear": mix(tone(659.3, 659.3, 0.6, 0.25, "sine", 4.0),
                     concat(silence(0.08), tone(880, 880, 0.55, 0.25, "sine", 4.0)),
                     concat(silence(0.16), tone(1318.5, 1318.5, 0.5, 0.2, "sine", 4.5))),
        "boss": mix(tone(80, 35, 0.55, 0.7, "sine", 3.0),
                    smooth(tone(0, 0, 0.4, 0.5, "noise", decay=5.0), 20)),
        "coin": concat(tone(1250, 1250, 0.06, 0.25, "square", 2.0),
                       tone(1700, 1700, 0.14, 0.25, "square", 4.0)),
        "door": mix(tone(130, 85, 0.22, 0.5, "sine", 4.0),
                    smooth(tone(0, 0, 0.15, 0.4, "noise", decay=6.0), 10)),
    }
    for name, samples in sounds.items():
        write_wav(out_dir / f"{name}.wav", samples)
    write_wav(out_dir / "ambient.wav", ambient())
    print(f"wrote {len(sounds) + 1} sounds to {out_dir}")


if __name__ == "__main__":
    main()
