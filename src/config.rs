use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Watched friends in priority order (first available wins).
    #[serde(default)]
    pub watched_steam_ids: Vec<u64>,
    /// Alert sound volume in `0.0..=1.0`.
    #[serde(default = "default_volume")]
    pub sound_volume: f32,
    /// Optional path to a custom alert sound (mp3/wav/ogg/flac).
    #[serde(default)]
    pub custom_sound_path: Option<String>,
    /// Show raw rich-presence key dumps in the UI.
    #[serde(default)]
    pub show_rp_debug: bool,
    /// Active watch poll interval in seconds (`1.0..=10.0`).
    #[serde(default = "default_active_poll")]
    pub active_poll_secs: f32,
    /// Idle friend-list refresh interval in seconds (`5.0..=60.0`).
    #[serde(default = "default_idle_poll")]
    pub idle_poll_secs: f32,
}

fn default_volume() -> f32 {
    1.0
}

fn default_active_poll() -> f32 {
    1.5
}

fn default_idle_poll() -> f32 {
    15.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            watched_steam_ids: Vec::new(),
            sound_volume: default_volume(),
            custom_sound_path: None,
            show_rp_debug: false,
            active_poll_secs: default_active_poll(),
            idle_poll_secs: default_idle_poll(),
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        let mut dir = dirs::config_dir()?;
        dir.push("cs2-friendwatch");
        Some(dir.join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let mut cfg: Self = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        cfg.sound_volume = cfg.sound_volume.clamp(0.0, 1.0);
        cfg.active_poll_secs = cfg.active_poll_secs.clamp(1.0, 10.0);
        cfg.idle_poll_secs = cfg.idle_poll_secs.clamp(5.0, 60.0);
        if cfg
            .custom_sound_path
            .as_ref()
            .is_some_and(|p: &String| p.trim().is_empty())
        {
            cfg.custom_sound_path = None;
        }
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or_else(|| "could not resolve config directory".to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())
    }
}
