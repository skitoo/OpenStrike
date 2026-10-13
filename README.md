# OpenStrike

Open-source reimplementation of the Sudden Strike engine (Fireglow / CDV, 2000), in the spirit of OpenRA and OpenTTD:
a modern engine running natively on macOS, Linux and Windows, which reads the data of an original copy of the game.

OpenStrike ships no game data. You need your own copy of *Sudden Strike Gold* (Steam/GOG).

## Status

Early engine: a map's ground scrolls in a raylib window and a tank drives where you right-click.
File formats are being reverse engineered; see `docs/formats.md`.

## Architecture

Functional core, imperative shell:

- `crates/formats` (`openstrike-formats`): pure decoders, bytes in, plain data out (archives, palettes, sprites, maps, ground).
- `crates/sim` (`openstrike-sim`): the deterministic simulation, `step(&World, &[Command]) -> World` at a fixed
  20 ticks per second, integer arithmetic only, so that replays and lockstep multiplayer stay possible.
- `crates/client` (`openstrike`): the raylib shell. It reads devices into commands, calls `step`, and draws the world.
  No game logic lives here.

Tests sit next to each crate in `tests/` and only use the public API.

## Running

Put your copy of the game in `data/SuddenStrikeGold/` (ignored by git), then:

```sh
cargo run --release -p openstrike                      # default map
cargo run --release -p openstrike -- path/to/map.ssm --climate winter
cargo test --workspace --release                        # data tests are skipped without the game
```

Controls: right click to move, arrows / WASD or middle drag to scroll, wheel to zoom.

## Layout

- `crates/` the engine
- `docs/` technical notes (file formats, game logic)
- `tools/` Python analysis and extraction scripts
