use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Settings {
    /// Steam application id of the game. It is launched through the
    /// `steam://rungameid/<id>` protocol handler, so Steam resolves the install
    /// location itself — no filesystem path is needed, on any platform.
    pub steam_app_id: u32,
    /// One or more process names used to detect when the game is running and to
    /// stop it. Matched by name only, so it is independent of where the game is
    /// installed. Accepts either a single string or a list in `config.json`,
    /// since the name varies by branch and runtime (e.g. `hl2_linux` for the
    /// default Linux build, `gmod` for the x86-64 branch, `gmod.exe` on
    /// Windows). All listed names are matched, so one config can cover several.
    #[serde(deserialize_with = "string_or_seq::deserialize")]
    pub process_names: Vec<String>,
    /// Read-throughput threshold in MB/s below which the game is treated as done
    /// loading and safe to close. While loading, the game reads assets from disk
    /// at hundreds of MB/s; once it reaches the menu that drops to ~zero (unlike
    /// CPU, which a focused 3D menu keeps high). Raise it if your machine has
    /// heavy background read activity; lower it (toward, but above, zero) to be
    /// stricter. Must be positive — `0` or less can never register as idle, so
    /// `read_config` replaces a non-positive value with the default.
    #[serde(default = "default_idle_read_threshold")]
    pub idle_read_threshold_mb_s: f64,
}

fn default_idle_read_threshold() -> f64 {
    10.0
}

impl Default for Settings {
    fn default() -> Self {
        // GMod is app 4000 on every platform, and Steam resolves the install via
        // the steam:// handler, so no path is ever needed. The process we monitor
        // and kill depends on the runtime, so we match every name a given OS
        // might produce. Override `process_names` if your setup differs.
        #[cfg(target_os = "windows")]
        let process_names = ["gmod.exe"];
        // Default 32-bit build launches as `hl2_linux`; the x86-64 branch as
        // `gmod` (the launcher at bin/linux64/gmod). Matching both means one
        // config works on either branch. (`hl2_linux` is shared by all native
        // Source games, so this can also match another Source title if running.)
        #[cfg(target_os = "linux")]
        let process_names = ["hl2_linux", "gmod"];
        #[cfg(target_os = "macos")]
        let process_names = ["hl2_osx"]; // GMod's macOS build is unmaintained; unverified.

        Self {
            steam_app_id: 4000,
            process_names: process_names.iter().map(|s| s.to_string()).collect(),
            idle_read_threshold_mb_s: default_idle_read_threshold(),
        }
    }
}

impl Settings {
    fn config_path(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    fn default_config_dir() -> std::io::Result<PathBuf> {
        let exe = std::env::current_exe()?;
        Ok(exe
            .parent()
            .expect("Executable must have a parent directory")
            .to_path_buf())
    }

    pub async fn initialize() -> std::io::Result<Self> {
        let dir = Self::default_config_dir()?;
        Self::read_config(&dir).await
    }

    #[cfg(test)]
    pub(crate) async fn write_config(&self, dir: &Path) -> std::io::Result<()> {
        let config_path = Self::config_path(dir);

        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        tokio::fs::write(config_path, json_data.as_bytes()).await?;

        Ok(())
    }

    pub async fn read_config(dir: &Path) -> std::io::Result<Self> {
        let config_path = Self::config_path(dir);

        // Load an existing config if it parses cleanly.
        if config_path.exists() {
            let data = tokio::fs::read_to_string(&config_path).await?;
            match serde_json::from_str::<Settings>(&data) {
                Ok(mut settings) => {
                    settings.normalize();
                    return Ok(settings);
                }
                Err(e) => {
                    // Don't clobber the user's file outright — keep a copy so a
                    // hand-edit with a typo is recoverable.
                    let backup = config_path.with_extension("json.bak");
                    eprintln!(
                        "config.json is invalid ({e}); backing it up to {} and resetting to defaults.",
                        backup.display()
                    );
                    if let Err(err) = tokio::fs::rename(&config_path, &backup).await {
                        eprintln!("Could not back up invalid config: {err}");
                    }
                }
            }
        }

        // The file is missing or unparsable (e.g. left over from an older
        // version with a different shape). Write fresh defaults and use them so
        // a stale config self-heals instead of aborting startup. I/O errors
        // above still propagate — only invalid contents are recovered here.
        let settings = Settings::default();
        let json_data = serde_json::to_string_pretty(&settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(&config_path, json_data.as_bytes()).await?;

        Ok(settings)
    }

    /// Repairs values that parse but are nonsensical, so a bad number degrades
    /// to the default instead of silently breaking behavior at runtime.
    fn normalize(&mut self) {
        if !(self.idle_read_threshold_mb_s.is_finite() && self.idle_read_threshold_mb_s > 0.0) {
            let fallback = default_idle_read_threshold();
            eprintln!(
                "idle_read_threshold_mb_s must be a positive number; using {fallback} instead of {}.",
                self.idle_read_threshold_mb_s
            );
            self.idle_read_threshold_mb_s = fallback;
        }
    }
}

/// Deserializes a field that may be written as either a single string or a list
/// of strings, always yielding a `Vec<String>`. Lets `process_names` be a bare
/// `"gmod.exe"` or a `["hl2_linux", "gmod"]` in `config.json`.
mod string_or_seq {
    use serde::de::{Deserializer, SeqAccess, Visitor};
    use std::fmt;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrSeq;

        impl<'de> Visitor<'de> for StringOrSeq {
            type Value = Vec<String>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a process name string or a list of names")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(vec![value.to_owned()])
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut names = Vec::new();
                while let Some(name) = seq.next_element::<String>()? {
                    names.push(name);
                }
                Ok(names)
            }
        }

        deserializer.deserialize_any(StringOrSeq)
    }
}
