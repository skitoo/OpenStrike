# OpenStrike contributor guide

## Project boundaries

- Keep the functional core and imperative shell split: `crates/formats` decodes supplied bytes into plain data, `crates/sim` owns deterministic game rules, and `crates/client` handles assets, input, and rendering.
- Put game rules in `openstrike-sim`, not in the raylib client. Keep format decoders independent of file I/O and rendering libraries.
- Preserve deterministic simulation behavior at 25 ticks per second. Follow the original game's integer arithmetic, ordering, and quirks where the reverse-engineering notes document them.
- Keep crate integration tests in each crate's `tests/` directory and exercise public APIs, matching the existing layout.

## Reverse engineering and game data

- Treat `docs/formats.md`, `docs/movement.md`, and `docs/original-code.md` as the project references for file formats, simulation behavior, and original-code addresses. Update the relevant note when implementation changes establish or correct a reverse-engineered fact.
- Mark deductions as inferred or unconfirmed in code comments and documentation; distinguish observed behavior from hypotheses.
- Original Sudden Strike data is user-provided and copyrighted. Use local files under `data/SuddenStrikeGold/` for investigation; never add game data, extracted assets, binaries, or analysis databases to the repository.
- Data-backed tests may require a local game installation. Keep tests usable without that data by following the existing skip behavior.

## Working in the Rust workspace

- Do not add co-author trailers or co-author attribution to commit messages.
- Write commit messages using Conventional Commits: `type(scope): imperative summary` (for example, `feat(client): add fog of war`).
- The repository is a Rust 2024 Cargo workspace. Use workspace dependencies and existing crate APIs before adding dependencies or cross-crate coupling.
- Keep changes scoped to the crate that owns the behavior; expose a public API only when another crate or an external caller needs it.
- Follow nearby Rust style and naming. Keep comments focused on game-specific behavior, invariants, or reverse-engineering evidence.
- Before each commit, run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
- Other useful checks: `cargo fmt --all -- --check` and `cargo check --workspace`. Use `cargo test --workspace --release` when checking release behavior. Data-backed tests are skipped when game data is absent, as described in `README.md`.

## Project map

- `crates/formats`: SUE archives, maps, sprites, palettes, object sets, and description/config formats.
- `crates/sim`: grid, movement, pathfinding, vision, and deterministic world stepping.
- `crates/client`: game-data loading, mission setup, terrain, camera, UI state, and raylib rendering.
- `docs/`: reverse-engineering notes and implementation evidence.
- `tools/`: Python probes, validators, and extraction helpers for local analysis.
