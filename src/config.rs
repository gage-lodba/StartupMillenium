use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

use serde::{Deserialize, Serialize};

pub(crate) static GLOBAL_CONFIG: LazyLock<Mutex<Settings>> =
    LazyLock::new(|| Mutex::new(Settings::default()));

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Settings {
    pub game_path: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        // Based on target os change default paths accordingly
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
                game_path: PathBuf::from("<hl2_linux>"),
            }
        }

        #[cfg(target_os = "macos")]
        {
            Self {
                game_path: PathBuf::from("<hl2_macos>"),
            }
        }
    }
}

impl Settings {
    pub fn get_settings() -> std::sync::MutexGuard<'static, Settings> {
        GLOBAL_CONFIG.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub async fn initialize() -> std::io::Result<()> {
        let settings = Self::read_config().await?;
        *Self::get_settings() = settings;
        Ok(())
    }

    #[cfg(test)]
    pub async fn write_config(&self) -> std::io::Result<()> {
        let config_path = std::env::current_dir()?.join("config.json");

        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(config_path, json_data.as_bytes()).await?;

        Ok(())
    }

    pub async fn read_config() -> std::io::Result<Self> {
        let config_path = std::env::current_dir()?.join("config.json");

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
