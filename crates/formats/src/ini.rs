//! Windows `.ini` files, as `GetPrivateProfileString` reads them: `[section]` headers, `key=value`
//! lines, names compared without case, values trimmed, `;` lines ignored.
//!
//! The game reads its settings from `SudTest.ini` (`GAME_EXE.EXE`, `[Debug] UseIniFile=1`); the
//! key names are string resources of `text_dll.dll` (44 `DataPathScm%i_%i`, 46 `BrightnessScm%i`,
//! 54 `SSDraw%i`).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ini {
    /// (section, key, value) in file order, names lowercased.
    entries: Vec<(String, String, String)>,
}

impl Ini {
    pub fn parse(text: &str) -> Self {
        let mut section = String::new();
        let mut entries = Vec::new();
        for line in text.lines().map(str::trim) {
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_ascii_lowercase();
            } else if let Some((key, value)) = line.split_once('=')
                && !line.starts_with(';')
            {
                entries.push((section.clone(), key.trim().to_ascii_lowercase(), value.trim().to_string()));
            }
        }
        Self { entries }
    }

    /// The first value of `key` in `section`.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let (section, key) = (section.to_ascii_lowercase(), key.to_ascii_lowercase());
        self.entries.iter().find(|(s, k, _)| *s == section && *k == key).map(|(_, _, v)| v.as_str())
    }

    /// The archives the game mounts for data scheme `scheme` (0 the shared data, then 1 + the
    /// map's climate), in order: the `;`-separated lists of `[Game] DataPathScm<scheme>_<i>` for
    /// `i` = 0, 1, ... up to the first missing or empty one, 32 at most (`Game_Dll.dll`
    /// `0x10058c20` for scheme 0, `0x10058d20` for the climate; each list mounted by `0x10093590`).
    pub fn data_paths(&self, scheme: u32) -> Vec<String> {
        (0..32)
            .map_while(|i| self.get("Game", &format!("DataPathScm{scheme}_{i}")).filter(|v| !v.is_empty()))
            .flat_map(|list| list.split(';').map(str::trim).filter(|n| !n.is_empty()).map(String::from).collect::<Vec<_>>())
            .collect()
    }
}
