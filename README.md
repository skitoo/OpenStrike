# OpenStrike

Open-source reimplementation of the Sudden Strike engine (Fireglow / CDV, 2000), in the spirit of OpenRA and OpenTTD:
a modern engine running natively on macOS, Linux and Windows, which reads the data of an original copy of the game.

OpenStrike ships no game data. You need your own copy of *Sudden Strike Gold* (Steam/GOG).

## Status

Exploration: reverse engineering the data formats.

## Layout

- `docs/` technical notes (file formats, game logic)
- `tools/` analysis and extraction scripts

Put the original game data in `data/SuddenStrikeGold/` (ignored by git).
