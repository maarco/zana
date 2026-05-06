use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub release_url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

#[tauri::command]
pub async fn check_for_updates() -> UpdateStatus {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let Some((owner, repo)) = update_repository() else {
        return UpdateStatus {
            current_version,
            latest_version: None,
            available: false,
            release_url: None,
            error: Some("No GitHub repository configured for update checks".to_string()),
        };
    };

    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    match fetch_latest_release(&url).await {
        Ok(release) => {
            let latest_version = normalize_version(&release.tag_name);
            let available = version_is_newer(&latest_version, &current_version);

            UpdateStatus {
                current_version,
                latest_version: Some(latest_version),
                available,
                release_url: Some(release.html_url),
                error: None,
            }
        }
        Err(error) => UpdateStatus {
            current_version,
            latest_version: None,
            available: false,
            release_url: None,
            error: Some(error),
        },
    }
}

async fn fetch_latest_release(url: &str) -> Result<GitHubRelease, String> {
    let response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::USER_AGENT, "qVoice-updater")
        .send()
        .await
        .map_err(|error| format!("Update check failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Update check returned HTTP {}",
            response.status().as_u16()
        ));
    }

    response
        .json::<GitHubRelease>()
        .await
        .map_err(|error| format!("Update response was invalid: {error}"))
}

fn update_repository() -> Option<(String, String)> {
    std::env::var("QVOICE_UPDATE_REPOSITORY")
        .ok()
        .or_else(|| std::env::var("GITHUB_REPOSITORY").ok())
        .or_else(|| Some(env!("CARGO_PKG_REPOSITORY").to_string()))
        .and_then(|value| parse_github_repository(&value))
}

fn parse_github_repository(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim().trim_end_matches(".git");
    if trimmed.is_empty() {
        return None;
    }

    let path = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))
        .unwrap_or(trimmed);

    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();

    if parts.next().is_some() || owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some((owner, repo))
}

fn normalize_version(version: &str) -> String {
    version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let latest_parts = parse_version_parts(latest);
    let current_parts = parse_version_parts(current);
    latest_parts > current_parts
}

fn parse_version_parts(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+'])
        .take_while(|part| part.chars().all(|ch| ch.is_ascii_digit()))
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_github_repository_formats() {
        assert_eq!(
            parse_github_repository("https://github.com/malmazan/qVoice"),
            Some(("malmazan".to_string(), "qVoice".to_string()))
        );
        assert_eq!(
            parse_github_repository("git@github.com:malmazan/qVoice.git"),
            Some(("malmazan".to_string(), "qVoice".to_string()))
        );
        assert_eq!(
            parse_github_repository("malmazan/qVoice"),
            Some(("malmazan".to_string(), "qVoice".to_string()))
        );
    }

    #[test]
    fn compares_release_versions() {
        assert!(version_is_newer("1.0.1", "1.0.0"));
        assert!(version_is_newer("1.1.0", "1.0.9"));
        assert!(!version_is_newer("1.0.0", "1.0.0"));
        assert!(!version_is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn normalizes_tag_prefix() {
        assert_eq!(normalize_version("v1.2.3"), "1.2.3");
        assert_eq!(normalize_version("V1.2.3"), "1.2.3");
    }
}
