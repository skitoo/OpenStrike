# OpenStrike contributor guide

## Project boundaries

- Keep the functional core and imperative shell split: `crates/formats` decodes supplied bytes into plain data, `crates/sim` owns deterministic game rules, and `crates/client` handles assets, input, and rendering.
- Put game rules in `openstrike-sim`, not in the raylib client. Keep format decoders independent of file I/O and rendering libraries.
- Preserve deterministic simulation behavior at 25 ticks per second. Follow the original game's integer arithmetic, ordering, and quirks where the reverse-engineering notes document them. Use integers, except where the original itself uses floating point (see `crates/sim/tests/fmath.rs`).
- Keep crate integration tests in each crate's `tests/` directory and exercise public APIs, matching the existing layout.

## Reverse engineering and game data

- Treat `docs/formats.md`, `docs/movement.md`, and `docs/original-code.md` as the project references for file formats, simulation behavior, and original-code addresses. Update the relevant note when implementation changes establish or correct a reverse-engineered fact.
- Mark deductions as inferred or unconfirmed in code comments and documentation; distinguish observed behavior from hypotheses.
- Original Sudden Strike data is user-provided and copyrighted. Use local files under `data/SuddenStrikeGold/` for investigation; never add game data, extracted assets, binaries, or analysis databases to the repository.
- Data-backed tests may require a local game installation. Keep tests usable without that data: read the install from `OPENSTRIKE_DATA` (default `data/SuddenStrikeGold/`) and return early when it is absent, as `data_dir()` does in `crates/client/tests/terrain.rs`.
- Screenshots of the engine running may be committed under `docs/images/`. Sprites, extracted files, and anything else taken from the game data may not.
- Differences from the original that were accepted on purpose are recorded in `docs/`; do not "fix" them without checking there first.
- `tools/` mirrors some Rust decoders (for example `tools/pck.py`). When you correct a format in one, correct the other.

## Working in the Rust workspace

- Do not add co-author trailers or co-author attribution to commit messages.
- Write commit messages using Conventional Commits: `type(scope): imperative summary` (for example, `feat(client): add fog of war`). Scopes are crate or directory names (`formats`, `sim`, `client`, `tools`, or `formats,client`); mark breaking behavior changes with `!`.
- Code, docs, comments, and commit messages are in English.
- The repository is a Rust 2024 Cargo workspace. Use workspace dependencies and existing crate APIs before adding dependencies or cross-crate coupling.
- Keep changes scoped to the crate that owns the behavior; expose a public API only when another crate or an external caller needs it.
- Follow nearby Rust style and naming. Keep comments focused on game-specific behavior, invariants, or reverse-engineering evidence.
- Before each commit, run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
- Other useful checks: `cargo check --workspace`. Formatting is configured in `rustfmt.toml` (120 columns); `cargo fmt --all -- --check` must pass. Use `cargo test --workspace --release` when checking release behavior. To check a rendering change, run the client with `--screenshot out.png` (plus `--look-at x,y` and `--zoom z`) and look at the image; see `README.md` for the options. Data-backed tests are skipped when game data is absent, as described in `README.md`.

## Project map

- `crates/formats`: SUE archives, maps, sprites, palettes, object sets, and description/config formats.
- `crates/sim`: grid, movement, pathfinding, vision, and deterministic world stepping.
- `crates/client`: game-data loading, mission setup, terrain, camera, UI state, and raylib rendering.
- `docs/`: reverse-engineering notes and implementation evidence.
- `tools/`: Python probes, validators, and extraction helpers for local analysis.

## Agent skills

### Issue tracker

Issues live in the repo's GitHub Issues, handled with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels, each named after its role. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
