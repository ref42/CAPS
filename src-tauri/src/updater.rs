use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

const RELEASE_API_URL: &str = "https://api.github.com/repos/ref42/CAPS/releases/latest";
const RELEASE_PAGE_URL: &str = "https://github.com/ref42/CAPS/releases/latest";

#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseUpdate {
    pub current: String,
    pub latest: String,
    pub url: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    Current {
        current: String,
        latest: String,
        url: String,
    },
    Available(ReleaseUpdate),
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    #[serde(default)]
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

pub async fn check_latest_release() -> Result<UpdateStatus, String> {
    let current = current_version();
    let client = http_client()?;
    let release: GitHubRelease = client
        .get(RELEASE_API_URL)
        .send()
        .await
        .map_err(|err| format!("Update check failed: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Update check failed: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid update response: {err}"))?;

    let latest_url = release_url(&release);
    if release.draft || release.prerelease {
        return Ok(UpdateStatus::Current {
            current: current.clone(),
            latest: current,
            url: latest_url,
        });
    }

    let latest = normalize_version(&release.tag_name);
    if is_newer_version(&latest, &current) {
        let asset = release
            .assets
            .into_iter()
            .find(is_windows_exe_asset)
            .ok_or_else(|| {
                format!(
                    "CAPS {latest} is available, but the Windows installer asset is missing. Open {latest_url}."
                )
            })?;
        if asset.browser_download_url.trim().is_empty() {
            return Err(format!(
                "CAPS {latest} is available, but its download URL is missing. Open {latest_url}."
            ));
        }
        Ok(UpdateStatus::Available(ReleaseUpdate {
            current,
            latest,
            url: latest_url,
            asset_name: asset.name,
            asset_url: asset.browser_download_url,
            asset_size: asset.size,
        }))
    } else {
        Ok(UpdateStatus::Current {
            current,
            latest,
            url: latest_url,
        })
    }
}

pub async fn download_and_install_update<F>(
    update: ReleaseUpdate,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, Option<u64>),
{
    let client = http_client()?;
    let path = download_update(&client, &update, &mut on_progress).await?;
    launch_update_handoff(&path)
}

pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("CAPS updater")
        .build()
        .map_err(|err| format!("Update check unavailable: {err}"))
}

async fn download_update<F>(
    client: &reqwest::Client,
    update: &ReleaseUpdate,
    on_progress: &mut F,
) -> Result<PathBuf, String>
where
    F: FnMut(u64, Option<u64>),
{
    let final_path = update_cache_path(&update.latest, &update.asset_name)?;
    let Some(parent) = final_path.parent() else {
        return Err("Update cache path is invalid.".to_string());
    };
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|err| format!("Update cache unavailable: {err}"))?;
    let temp_path = final_path.with_extension("download");
    let _ = tokio::fs::remove_file(&temp_path).await;

    let mut response = client
        .get(&update.asset_url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
        .map_err(|err| format!("Update download failed: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Update download failed: {err}"))?;
    let expected_size = response
        .content_length()
        .or((update.asset_size > 0).then_some(update.asset_size));
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|err| format!("Update cache write failed: {err}"))?;
    let mut downloaded = 0_u64;
    on_progress(downloaded, expected_size);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("Update download read failed: {err}"))?
    {
        downloaded += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .map_err(|err| format!("Update cache write failed: {err}"))?;
        on_progress(downloaded, expected_size);
    }
    file.flush()
        .await
        .map_err(|err| format!("Update cache write failed: {err}"))?;
    if downloaded == 0 {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err("Update download was empty.".to_string());
    }
    if let Some(expected) = expected_size {
        if downloaded != expected {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(format!(
                "Update download size mismatch: got {downloaded} bytes, expected {expected} bytes."
            ));
        }
    }
    let _ = tokio::fs::remove_file(&final_path).await;
    tokio::fs::rename(&temp_path, &final_path)
        .await
        .map_err(|err| format!("Update cache finalize failed: {err}"))?;
    Ok(final_path)
}

#[cfg(target_os = "windows")]
fn launch_update_handoff(update_path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let current_exe =
        std::env::current_exe().map_err(|err| format!("Current app path unavailable: {err}"))?;
    let pid = std::process::id();
    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $pidToWait = {pid}; \
         $source = {}; \
         $target = {}; \
         Wait-Process -Id $pidToWait -ErrorAction SilentlyContinue; \
         Start-Sleep -Milliseconds 350; \
         Copy-Item -LiteralPath $source -Destination $target -Force; \
         Start-Process -FilePath $target;",
        ps_quote(&update_path.to_string_lossy()),
        ps_quote(&current_exe.to_string_lossy())
    );

    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|err| format!("Update installer failed to start: {err}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_update_handoff(_update_path: &Path) -> Result<(), String> {
    Err("Automatic updates are only available in Windows builds.".to_string())
}

#[cfg(target_os = "windows")]
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn release_url(release: &GitHubRelease) -> String {
    if release.html_url.trim().is_empty() {
        RELEASE_PAGE_URL.to_string()
    } else {
        release.html_url.clone()
    }
}

fn is_windows_exe_asset(asset: &GitHubAsset) -> bool {
    let name = asset.name.to_ascii_lowercase();
    name.ends_with(".exe") && name.contains("windows") && name.contains("x64")
}

fn update_cache_path(version: &str, asset_name: &str) -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| "Update cache path is not available.".to_string())?;
    Ok(base
        .join("CAPS")
        .join("update-cache")
        .join(normalize_version(version))
        .join(safe_file_name(asset_name)))
}

fn safe_file_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn normalize_version(version: &str) -> String {
    version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    version_parts(latest) > version_parts(current)
}

fn version_parts(version: &str) -> [u64; 3] {
    let mut parts = [0_u64; 3];
    for (index, part) in normalize_version(version).split('.').take(3).enumerate() {
        parts[index] = part
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_version_tags() {
        assert_eq!(normalize_version("v1.2.3"), "1.2.3");
        assert_eq!(normalize_version("V1.2.3"), "1.2.3");
    }

    #[test]
    fn compares_semver_like_versions() {
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(is_newer_version("1.1.0", "1.0.9"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("0.9.9", "1.0.0"));
    }

    #[test]
    fn picks_windows_release_executable() {
        assert!(is_windows_exe_asset(&GitHubAsset {
            name: "CAPS-v1.0.1-windows-x64.exe".to_string(),
            browser_download_url: String::new(),
            size: 0,
        }));
        assert!(!is_windows_exe_asset(&GitHubAsset {
            name: "CAPS-v1.0.1-windows-x64.exe.sha256".to_string(),
            browser_download_url: String::new(),
            size: 0,
        }));
    }
}
