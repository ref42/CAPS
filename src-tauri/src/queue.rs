use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub path: PathBuf,
    pub title: String,
    pub location: String,
    pub cloud_id: Option<String>,
}

impl QueueItem {
    pub fn from_path(path: PathBuf) -> Self {
        let title = path
            .file_stem()
            .map(os_str_to_utf8)
            .unwrap_or_else(|| "Unknown".into());
        let location = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            path,
            title,
            location,
            cloud_id: None,
        }
    }

    pub fn is_cloud(&self) -> bool {
        self.cloud_id.is_some()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueEntry {
    pub title: String,
    pub location: String,
    pub cloud_id: Option<String>,
    pub is_cloud: bool,
}

impl From<&QueueItem> for QueueEntry {
    fn from(item: &QueueItem) -> Self {
        Self {
            title: item.title.clone(),
            location: item.location.clone(),
            cloud_id: item.cloud_id.clone(),
            is_cloud: item.is_cloud(),
        }
    }
}

pub fn is_audio_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref(),
        Some("wav" | "flac" | "ogg" | "mp3" | "m4a" | "aac")
    )
}

fn os_str_to_utf8(s: &std::ffi::OsStr) -> String {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = s.encode_wide().collect();
        String::from_utf16_lossy(&wide)
    }
    #[cfg(not(windows))]
    {
        s.to_string_lossy().into_owned()
    }
}
