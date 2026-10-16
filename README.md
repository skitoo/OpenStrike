# OpenStrike

Open-source reimplementation of the Sudden Strike engine (Fireglow / CDV, 2000), in the spirit of OpenRA and OpenTTD:
a modern engine running natively on macOS, Linux and Windows, which reads the data of an original copy of the game.

OpenStrike ships no game data. You need your own copy of *Sudden Strike Gold* (Steam/GOG).

## Status

Early engine, not yet a game: nothing fights yet.

- A mission map is drawn in the game's isometric view with its ground, roads, rails, trees, fences,
  buildings, bridges and piers, in the state the map stores.
- The mission's own tanks are placed for each player and side. They move and find their way with the original
  game's rules and path finder, alone or in formation.
- Each side sees only what its units see, with the original fog of war.
- The original interface is drawn: panel, minimap with the view frame and units, orders grid, information bar
  with tooltips and the hovered unit's details, status bars, and unit groups. Only the stop and hold orders
  act so far.
File formats are being reverse engineered; see `docs/formats.md`, `docs/movement.md` and the address index `docs/original-code.md`.

## Screenshot

![OpenStrike rendering a map with terrain, buildings and the minimap](docs/images/gameplay.png)

## Architecture

Functional core, imperative shell:

- `crates/formats` (`openstrike-formats`): pure decoders, bytes in, plain data out (archives, palettes, sprites, maps, ground).
- `crates/sim` (`openstrike-sim`): the deterministic simulation, `step(&World, &Rules, &[Command]) -> World` at the
  game's 25 ticks per second, reproducing the original movement code (`docs/movement.md`), so that replays and
  lockstep multiplayer stay possible. Integer arithmetic, except where the original itself uses floating point.
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

Controls: left click or drag to select (Shift adds), right click to move, arrows or middle drag to scroll,
wheel to zoom. As in the original: the orders grid answers the 3 x 3 block of keys under the digits (Q W E / A S D /
Z X C on QWERTY), R and T toggle the hold buttons, Ctrl + digit makes a group and the digit selects it (twice
centres the view). B shows blocked cell quarters.
Options: `--data dir` (default `data/SuddenStrikeGold/`), `--climate`; debug: `--look-at x,y`, `--zoom z`,
`--show-blocked`, `--screenshot file.png`.

## Layout

- `crates/` the engine
- `docs/` technical notes (file formats, game logic)
- `tools/` Python analysis and extraction scripts

## License

OpenStrike is free software, released under the GNU General Public License version 3 or later (see `LICENSE`).
It contains no data from the original game, which remains the property of its rights holders.
