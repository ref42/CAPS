use serde::Deserialize;

const RELEASE_API_URL: &str = "https://api.github.com/repos/ref42/CAPS/releases/latest";
const RELEASE_PAGE_URL: &str = "https://github.com/ref42/CAPS/releases/latest";

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    Current {
        version: String,
    },
    Available {
        current: String,
        latest: String,
        url: String,
    },
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
}

pub async fn check_latest_release() -> Result<UpdateStatus, String> {
    let current = current_version();
    let release: GitHubRelease = reqwest::Client::builder()
        .user_agent("CAPS updater")
        .build()
        .map_err(|err| format!("Update check unavailable: {err}"))?
        .get(RELEASE_API_URL)
        .send()
        .await
        .map_err(|err| format!("Update check failed: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Update check failed: {err}"))?
        .json()
        .await
        .map_err(|err| format!("Invalid update response: {err}"))?;

    if release.draft || release.prerelease {
        return Ok(UpdateStatus::Current { version: current });
    }

    let latest = normalize_version(&release.tag_name);
    if is_newer_version(&latest, &current) {
        Ok(UpdateStatus::Available {
            current,
            latest,
            url: if release.html_url.trim().is_empty() {
                RELEASE_PAGE_URL.to_string()
            } else {
                release.html_url
            },
        })
    } else {
        Ok(UpdateStatus::Current { version: current })
    }
}

pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
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
}
