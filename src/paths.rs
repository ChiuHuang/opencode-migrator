use std::path::PathBuf;

pub fn find_opencode_db(custom: Option<&str>) -> Option<PathBuf> {
    if let Some(c) = custom {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }

    let mut candidates = Vec::new();

    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join(".local")
                .join("share")
                .join("opencode")
                .join("opencode.db"),
        );
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(&local_app_data)
                    .join("opencode")
                    .join("opencode.db"),
            );
        }
        if let Ok(app_data) = std::env::var("APPDATA") {
            candidates.push(
                PathBuf::from(&app_data)
                    .join("opencode")
                    .join("opencode.db"),
            );
            candidates.push(
                PathBuf::from(&app_data)
                    .join("ai.opencode.desktop")
                    .join("opencode.db"),
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
            candidates.push(PathBuf::from(xdg_data).join("opencode").join("opencode.db"));
        }
    }

    candidates.into_iter().find(|c| c.is_file())
}

/// Returns the Freebuff Desktop config dir (~/.config/freebuff-desktop or %APPDATA%\freebuff-desktop).
pub fn find_freebuff_dir(custom: Option<&str>) -> Option<PathBuf> {
    if let Some(c) = custom {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }

    let mut candidates = Vec::new();

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".config").join("freebuff-desktop"));
        candidates.push(home.join(".freebuff"));
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            candidates.push(PathBuf::from(&app_data).join("freebuff-desktop"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            candidates.push(PathBuf::from(xdg_config).join("freebuff-desktop"));
        }
    }

    for c in &candidates {
        if c.is_dir() {
            return Some(c.clone());
        }
    }
    candidates.into_iter().next()
}

/// Returns the Freebuff CLI (manicode) config dir (~/.config/manicode or FREEBUFF_CONFIG_DIR).
/// This is where `freebuff` CLI stores chat history (chat-messages.json / chat-meta.json).
pub fn find_manicode_dir() -> Option<PathBuf> {
    // Respect the same env var that freebuff CLI uses
    if let Ok(dir) = std::env::var("FREEBUFF_CONFIG_DIR") {
        let p = PathBuf::from(&dir);
        if p.is_absolute() {
            return Some(p);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config").join("manicode");
        return Some(p);
    }

    None
}

pub fn find_agy_dir(custom: Option<&str>) -> Option<PathBuf> {
    if let Some(c) = custom {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }

    let mut candidates = Vec::new();

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".gemini").join("antigravity-cli"));
        candidates.push(home.join(".gemini").join("antigravity"));
    }

    for c in &candidates {
        if c.is_dir() {
            return Some(c.clone());
        }
    }
    candidates.into_iter().next()
}
