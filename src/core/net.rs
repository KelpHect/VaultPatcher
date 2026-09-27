//! Minimal GitHub release + download helpers (blocking; run off the UI thread).

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result};

const USER_AGENT: &str = concat!("Vaulter/", env!("CARGO_PKG_VERSION"));

/// Shared agent with timeouts, so a stalled connection fails instead of
/// hanging a setup run forever.
fn agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(15))
            .timeout_read(std::time::Duration::from_secs(60))
            .build()
    })
}

pub struct ReleaseAsset {
    pub tag: String,
    pub name: String,
    pub url: String,
}

/// The latest release of `repo` (`owner/name`) and its first asset whose name
/// satisfies `pick`.
pub fn latest_asset(repo: &str, pick: impl Fn(&str) -> bool) -> Result<ReleaseAsset> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let release: serde_json::Value = agent().get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("contacting GitHub for {repo}"))?
        .into_json()?;
    let tag = release["tag_name"].as_str().unwrap_or("latest").to_string();
    let asset = release["assets"]
        .as_array()
        .and_then(|assets| assets.iter().find(|a| a["name"].as_str().is_some_and(&pick)))
        .with_context(|| format!("no matching download in {repo} {tag}"))?;
    Ok(ReleaseAsset {
        tag,
        name: asset["name"].as_str().unwrap_or_default().to_string(),
        url: asset["browser_download_url"]
            .as_str()
            .context("asset has no download url")?
            .to_string(),
    })
}

pub fn download(url: &str, dest: &Path) -> Result<()> {
    let mut reader = agent().get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("downloading {url}"))?
        .into_reader();
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    // A name no other download uses, removed again if this one fails.
    let tmp = super::atomic::temp_file("download.part");
    let copied = fs::File::create(&tmp).and_then(|mut file| std::io::copy(&mut reader, &mut file).and_then(|_| file.sync_all()));
    let moved = copied.and_then(|()| fs::rename(&tmp, dest).or_else(|_| fs::copy(&tmp, dest).map(drop)));
    fs::remove_file(&tmp).ok();
    moved.with_context(|| format!("saving {}", dest.display()))
}

/// The newest Vaulter release: the highest version tag among recent
/// published releases. The same repo also hosts `comparisons-*` image pack
/// releases, which `releases/latest` could return instead.
pub fn latest_app_release(repo: &str) -> Result<(String, String)> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=30");
    let releases: serde_json::Value = agent()
        .get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("contacting GitHub for {repo}"))?
        .into_json()?;
    newest_version(releases.as_array().map(Vec::as_slice).unwrap_or_default()).context("no app release found")
}

/// (tag, page) of the highest `v1.2.3`-style tag among published releases.
fn newest_version(releases: &[serde_json::Value]) -> Option<(String, String)> {
    releases
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false) && !r["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            let version = parse_version(tag)?;
            Some((version, tag.to_string(), r["html_url"].as_str().unwrap_or_default().to_string()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, tag, page)| (tag, page))
}

fn parse_version(s: &str) -> Option<Vec<u64>> {
    s.trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect()
}

/// The latest release's tag and web page (for update checks).
pub fn latest_release(repo: &str) -> Result<(String, String)> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let release: serde_json::Value = agent()
        .get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("contacting GitHub for {repo}"))?
        .into_json()?;
    let tag = release["tag_name"].as_str().context("release has no tag")?.to_string();
    let page = release["html_url"].as_str().unwrap_or_default().to_string();
    Ok((tag, page))
}

/// True when `tag` (like `v0.2.0`) is a newer version than `current`.
pub fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_compare() {
        assert!(super::is_newer("v0.2.0", "0.1.9"));
        assert!(super::is_newer("1.0.0", "0.9.0"));
        assert!(!super::is_newer("v0.1.0", "0.1.0"));
        assert!(!super::is_newer("comparisons-bl2", "0.1.0"));
    }

    #[test]
    fn app_updates_skip_image_pack_releases() {
        let r = |tag: &str, pre: bool| serde_json::json!({"tag_name": tag, "html_url": format!("page/{tag}"), "draft": false, "prerelease": pre});
        let list = [r("comparisons-bl2", false), r("v0.3.0", false), r("comparisons-tps", false), r("v0.10.0", false), r("v0.11.0", true), r("v0.9.1", false)];
        assert_eq!(super::newest_version(&list), Some(("v0.10.0".into(), "page/v0.10.0".into())));
        assert_eq!(super::newest_version(&list[..1]), None);
    }
}
