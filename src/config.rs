use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Settings {
    pub game_path: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self {
                game_path: PathBuf::from(
                    "C:\\Program Files (x86)\\Steam\\steamapps\\common\\GarrysMod\\gmod.exe",
                ),
            }
        }

        #[cfg(target_os = "linux")]
        {
            Self {
                game_path: PathBuf::from("hl2_linux"),
            }
        }

        #[cfg(target_os = "macos")]
        {
            Self {
                game_path: PathBuf::from("hl2.sh"),
            }
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

        if !config_path.exists() {
            let json_data = serde_json::to_string_pretty(&Settings::default())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            tokio::fs::write(&config_path, json_data.as_bytes()).await?;
        }

        let data = tokio::fs::read_to_string(config_path).await?;

        let settings: Settings = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(settings)
    }
}
