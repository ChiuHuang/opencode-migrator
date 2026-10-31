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
