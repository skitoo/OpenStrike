//! Command line options.

use std::path::{Path, PathBuf};

pub const USAGE: &str = "\
Usage: openstrike [options] [map.ssm]

Options:
  --data DIR          Sudden Strike Gold install (default: data/SuddenStrikeGold)
  --climate NAME      beach, desert, summer or winter (default: summer)
  --look-at X,Y       centre the camera on a map pixel at start
  --zoom Z            initial zoom (0.5 to 3)
  --show-blocked      show the blocked terrain tiles (toggle in game with B)
  --screenshot FILE   run a short demo, save the screen to FILE and quit
  --help              show this help

Without a map, the single-player map 'Act of Sabotage' of the install is used.";

pub const CLIMATES: [&str; 4] = ["beach", "desert", "summer", "winter"];

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub data: PathBuf,
    pub map: PathBuf,
    pub climate: String,
    pub screenshot: Option<PathBuf>,
    /// Map pixel to centre the camera on at start (defaults to the first unit).
    pub look_at: Option<(i32, i32)>,
    pub zoom: f32,
    pub show_blocked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    Run(Options),
    Help,
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

/// Parse the arguments (without the program name). Paths are not checked here: see [`check`].
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    let mut args = args.into_iter();
    let mut o = Options {
        data: PathBuf::from("data/SuddenStrikeGold"),
        map: PathBuf::new(),
        climate: "summer".into(),
        screenshot: None,
        look_at: None,
        zoom: 1.0,
        show_blocked: false,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" | "-h" => return Ok(Parsed::Help),
            "--data" => o.data = value(&mut args, &a)?.into(),
            "--climate" => {
                let c = value(&mut args, &a)?.to_ascii_lowercase();
                if !CLIMATES.contains(&c.as_str()) {
                    return Err(format!("unknown climate {c:?}, expected one of {}", CLIMATES.join(", ")));
                }
                o.climate = c;
            }
            "--screenshot" => o.screenshot = Some(value(&mut args, &a)?.into()),
            "--show-blocked" => o.show_blocked = true,
            "--zoom" => {
                let v = value(&mut args, &a)?;
                o.zoom = v.parse().map_err(|_| format!("--zoom expects a number, got {v:?}"))?;
            }
            "--look-at" => {
                let v = value(&mut args, &a)?;
                let parsed = v.split_once(',').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
                o.look_at = Some(parsed.ok_or_else(|| format!("--look-at expects X,Y, got {v:?}"))?);
            }
            _ if a.starts_with('-') => return Err(format!("unknown option {a:?} (see --help)")),
            _ if !o.map.as_os_str().is_empty() => return Err(format!("only one map can be given, got {:?} and {a:?}", o.map)),
            _ => o.map = PathBuf::from(a),
        }
    }
    if o.map.as_os_str().is_empty() {
        o.map = o.data.join("Maps/single/'Act of Sabotage'.ssm");
    }
    Ok(Parsed::Run(o))
}

/// Check that the install and the map exist, with messages that say what to do.
pub fn check(o: &Options) -> Result<(), String> {
    if !o.data.join("units.sue").is_file() {
        return Err(format!(
            "no Sudden Strike Gold install in {:?}: put the game there or pass --data DIR",
            absolute(&o.data)
        ));
    }
    if !o.map.is_file() {
        return Err(format!("map not found: {:?}", absolute(&o.map)));
    }
    Ok(())
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}
