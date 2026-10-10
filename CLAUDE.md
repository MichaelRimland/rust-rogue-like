# CLAUDE.md – Project handoff and conventions

Context for Claude (or any AI assistant) working on this repository.
**The user communicates in German – answer in German.** Code, comments, commit
messages and the README stay in English (portfolio project).

## Who and why

- The user is an experienced programmer building this game as a **portfolio project**
  for job applications as a software developer. The GitHub repo is linked in their CV.
- Goal: let AI take over a lot of the typing, while the result shows clean architecture,
  tests, CI and a playable browser demo that recruiters can open with one click.
- Quality bar: idiomatic Rust, small focused plugins, tests for logic, clippy-clean,
  a commit history that reads well.

## The game

**rust-rogue-like** (working title, final name still open – "Runebound" was dropped because a board game of that name exists) – a fantasy survivor-roguelike (Vampire Survivors meets roguelike runs).
A wizard in a cursed forest; spells fire automatically, the player only moves and picks
upgrades on level-up. Later milestones add procedurally generated dungeon floors
(arena rooms, shops, treasure, bosses) between the survivor fights.

## Current status (Oct 2026)

**Milestone 1 (core loop) is done and playable.** The Bevy code builds on 0.20.0 without
warnings; `cargo clippy --all-targets -- -D warnings`, `cargo test` (19 tests) and `cargo fmt`
are clean. The repo lives at github.com/MichaelRimland/rust-rogue-like (private for now) with
CI (fmt, clippy, test, wasm build) and a Pages deploy workflow.

- The Arcane Missile fires automatically towards the **mouse cursor** (no target selection);
  volleys fan out symmetrically around the aim direction.
- The WASM build works in CI. The deploy job fails until GitHub Pages is enabled
  (Settings → Pages → Source: GitHub Actions); Pages needs a public repo on the free plan.
- Still open from M1: gameplay GIF for the README (`docs/gameplay.gif`), balancing pass.
- Next: **M2 performance** (benchmark scene, profiling, pooling, 5 000 enemies at 60 FPS).

## What Milestone 1 contains

- Player: WASD/arrows, health, regeneration, red hurt tint, wizard hat child mesh.
- Enemies: Goblin (from 0 s), Skeleton (from 45 s), Ogre (from 120 s). Spawn in waves on
  a ring around the player, chase with separation, get wrapped back when too far away,
  squash animation on hit, death particles, cap of 1 500 enemies.
- Weapons: Arcane Missile (start weapon, fires towards the mouse cursor, volley count, pierce),
  Holy Aura (damage ticks in a circle), Orbiting Runes (rotating hexagons).
- Progression: XP gems with magnet pickup, quadratic XP curve, stacked level-ups,
  3 weighted upgrade choices (new weapons weighted higher), caps per upgrade.
- UI: main menu, HUD (XP bar, HP bar, level/time/kills), level-up cards, pause overlay,
  game over screen with seed.
- Visuals: no sprite assets yet – unit meshes (circle, hexagon, rhombus, triangle) scaled
  via `Transform`, colors in `assets.rs`. Endless forest floor via wrapping decor.
- Seeded runs: seed from `Time<Real>` at run start (works on WASM), shown on game over.

## File map

```
src/main.rs         window (1280x720, fit canvas on web), adds GamePlugin
src/lib.rs          GamePlugin, GameSet ordering: Spatial → Movement → Combat → Damage → Progression
src/states.rs       AppState {MainMenu, InGame, GameOver}, sub-state GamePhase {Running, LevelUp, Paused}
src/run.rs          GameRng, Stats (PlayerStats), Xp, RunStats; start_run; pause toggle
src/assets.rs       GameAssets: shared meshes + materials
src/world.rs        camera spawn/follow, wrapping decor
src/player.rs       Player, Health, movement, regen, hurt tint, death check
src/enemy.rs        Enemy, EnemyGrid (spatial hash), spawning, steering, wrapping, hit squash
src/weapons.rs      missiles, aura, runes (fire + hit detection)
src/combat.rs       DamageEnemy message, apply_damage (kills, gems, particles), contact damage
src/progression.rs  XpGem, PendingLevelUps, LevelUpChoices, choose_upgrade
src/ui/             mod.rs (helpers, colors), hud.rs, menus.rs, level_up.rs
src/logic/          engine-independent + unit tested:
                    rng.rs (SplitMix64), spatial.rs (SpatialGrid), stats.rs (PlayerStats, Upgrade,
                    roll_upgrades), progression.rs (XP + difficulty curves), enemies.rs (EnemyKind)
web/index.html      page for the WASM build
.github/workflows/  ci.yml (fmt, clippy, test, wasm build), deploy.yml (GitHub Pages)
```

## Balancing numbers

- Player base stats: `logic/stats.rs` → `PlayerStats::default()`; weapon scaling in
  `AuraStats` / `RuneStats` methods; upgrade effects and caps in `Upgrade::apply` and the
  `MAX_*` constants.
- Enemy stats and spawn weights: `logic/enemies.rs`.
- XP curve, spawn interval, batch size, HP scaling over time: `logic/progression.rs`.
- Spawn distance, enemy cap, separation: constants at the top of `enemy.rs`.
- Missile range/radius/lifetime: constants at the top of `weapons.rs`.

## Roadmap

- [x] **M1 core loop** – done
- [ ] **M2 performance** – benchmark scene, profiling, pooling, goal: 5 000 enemies at 60 FPS
- [ ] **M3 dungeon** – generated floors of rooms (arena, shop, treasure, elite), doors open
      after a wave, boss at the end of a floor
- [ ] **M4 content** – weapons/enemies data-driven from RON files, weapon evolutions
      (item A + item B → super weapon), bosses, more enemy types
- [ ] **M5 polish** – CC0 pixel art (Kenney / itch.io), sound, screen shake, damage numbers,
      meta-progression with save game, settings
- Possible later: headless integration tests (`App` with `MinimalPlugins` + `StatesPlugin`),
  controller support, more characters.

## Bevy 0.20 pitfalls

Bevy changes its API every release and most training data shows older versions.
Always write code for **0.20**. In particular:

- Buffered events are **messages**: `#[derive(Message)]`, `MessageWriter::write`,
  `MessageReader::read`, `app.add_message::<T>()`. `#[derive(Event)]` is only for
  observers (`On<T>`).
- No bundles like `SpriteBundle`/`NodeBundle`; spawn components directly (required components).
- State-scoped cleanup uses `DespawnOnExit(state)` (formerly `StateScoped`); works for sub-states too.
- `NextState::set_if_neq` is now `set_if_different`.
- `ui::widgets::Button` and `Interaction` are deprecated; use `bevy::ui_widgets::Button`,
  `Hovered` and `Pressed` (see Bevy's `examples/state/states.rs`).
- `TextFont::font_size` is a `FontSize` (`FontSize::Px(20.0)`), not an `f32`.
- `BorderRadius` is a field on `Node` and takes `CornerRadius` values (`BorderRadius::all(px(8.0))`).
- Shape primitives moved to `bevy_shape` but are still in `bevy::prelude`.
- Run-condition combinator is `.and_then(...)`; chaining `.run_if(a).run_if(b)` also works.
- `Assets::get_mut` returns an `AssetMut` guard.
- Use `Single<..>` for queries that must match exactly one entity (system is skipped if none).
- Requires Rust 1.97.1+.

When unsure, check the official examples and migration guides for 0.20
(github.com/bevyengine/bevy, tag v0.20.0) instead of guessing.

## Conventions

- One plugin per feature; new gameplay systems go into one of the `GameSet` sets in `lib.rs`.
- Engine-independent rules go into `src/logic/` and get unit tests there.
- Weapons never modify enemy health directly; they write `DamageEnemy` messages.
- Entities created during a run get `DespawnOnExit(AppState::InGame)`.
- Randomness only through the `GameRng` resource, so runs stay reproducible from their seed.
- Only dependency is `bevy` (2d + ui features). Add crates only when clearly worth it and
  check that they support Bevy 0.20.
- Clippy lints `too_many_arguments` and `type_complexity` are allowed in `Cargo.toml`.
- Before committing: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- Commit messages in English, imperative mood, small focused commits.
