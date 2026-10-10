# Art style guide

All sprites of the game must look like they were drawn by one hand. This file is the
single source of truth; every generation prompt starts with the **style block** below and the
post-processing script (`tools/process_sprites.py`) enforces size and palette.

## Direction

Dark fantasy survivor-roguelike. Mood references:

- **Berserk**: grim, gritty, scarred; heavy silhouettes, no cute proportions.
- **Hero Siege**: top-down 2D pixel-art ARPG readability, chunky sprites, bright spell effects.
- **WoW**: bold, readable silhouettes and strong, saturated spell colors.
- **Lunacid**: eerie, foggy, desaturated world with a few glowing accent colors (purple, teal).

Rule of thumb: the **world is dark and muted**, the **player's magic and loot are bright**.
That also keeps the screen readable with hundreds of enemies.

## Technical rules

| Rule | Value |
| --- | --- |
| Perspective | top-down with slight 3/4 tilt, characters face the camera/down |
| Base cell | 32x32 px (ogre and bosses 48x48 or 64x64, effects 16x16) |
| Outline | 1 px, near-black (`#0d0b14`), added by the script |
| Shading | 2-3 tones per material, light from the top-left, no gradients |
| Palette | exactly the 32 colors in `tools/palette.hex` |
| Background | transparent in the final PNG |
| Animation | idle = 1 frame, walk = 2 frames (alternating), the rest is done in code (bobbing, squash on hit, flip) |

Why so few animation frames: AI generators wobble from frame to frame. Two frames plus
code-driven motion keeps the whole project consistent.

## Palette

Defined in `tools/palette.hex` (one `#RRGGBB` per line). Groups:

| Group | Use |
| --- | --- |
| near-black / violet-gray | outlines, shadows, void |
| stone / bone | skeletons, rocks, UI text |
| moss greens | forest floor, goblins |
| leather / earth | ogre hide, wood, dirt |
| blood reds | damage, hurt tint, enemy eyes |
| arcane purple | the wizard's magic (Arcane Missile, runes) |
| ghost teal | fog, ghostly accents, XP gems |
| gold | holy aura, pickups, UI highlights |

## Current sprites

The sprites in `assets/sprites/` are drawn in code by `tools/draw_sprites.py`
(`python tools/draw_sprites.py`). Edit the character grids there and re-run to change them.
AI-generated art can replace any file; keep the file names.

## Generation workflow

1. Generate the **reference wizard** first and pick one you like. Every other prompt reuses it
   as a style reference (image-to-image at low denoise, or a style/IP adapter).
2. Generate each asset with the prompt template below, 1024x1024, **flat magenta background**.
3. Put the raw images into `assets/raw/` (git-ignored, name = final sprite name, e.g.
   `goblin.png`).
4. Run the script, see `tools/process_sprites.py --help`:

   ```bash
   pip install pillow
   python tools/process_sprites.py assets/raw assets/sprites --size ogre=48
   ```
5. Check the result in `assets/sprites/`; regenerate the raw image if it is off-style.

## Prompt template

```
STYLE BLOCK (paste unchanged at the start of every prompt):
pixel art game sprite, dark fantasy, gritty and grim, top-down view with slight 3/4 tilt,
limited muted color palette with one bright accent color, 1 pixel dark outline,
flat shading with 2-3 tones per material, light from the top-left, chunky readable
silhouette, centered, full body visible, no text, no border, no shadow on the ground,
on a flat solid magenta (#FF00FF) background

SUBJECT: <see list below>
```

Negative prompt (if the tool supports it): `blurry, anti-aliased, gradient, photo, 3d render,
realistic, text, watermark, multiple characters, cropped`.

## Asset list

| Name | Size | Subject |
| --- | --- | --- |
| `wizard` | 32 | hooded wizard in a tattered violet robe with a crooked staff, pale face in shadow, a faint arcane glow in the hands |
| `wizard_walk` | 32 | same wizard mid-step, other foot forward (frame 2) |
| `goblin` | 32 | hunched feral goblin with a rusty dagger, sickly green skin, torn rags, yellow eyes |
| `goblin_walk` | 32 | frame 2 |
| `skeleton` | 32 | rattling skeleton in rusted armor scraps, cracked skull, notched sword, teal glowing eye sockets |
| `skeleton_walk` | 32 | frame 2 |
| `ogre` | 48 | hulking ogre with patchwork hide and a spiked wooden club, scarred, tiny angry red eyes |
| `ogre_walk` | 48 | frame 2 |
| `missile` | 16 | glowing arcane bolt, violet core, white-lilac tip, short trail |
| `gem` | 16 | small faceted experience crystal, teal with a bright highlight |
| `rune` | 16 | glowing carved rune stone, gold, hexagonal |
| `ground_a` / `ground_b` / `ground_c` | 32 | seamless dark forest floor tiles: moss and dirt, fallen leaves, small roots (must tile) |
| `decor_bush`, `decor_stone`, `decor_bones` | 16-32 | dead bush, mossy stone, scattered bones |

Ground tiles are the one place where AI generators struggle: they must tile. If a tile
does not repeat cleanly, generate it with a "seamless tile" option or let the script check
edges (`--check-tiling`, planned).
