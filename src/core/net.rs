//! Minimal GitHub release + download helpers (blocking; run off the UI thread).

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result};

const USER_AGENT: &str = concat!("VaultPatcher/", env!("CARGO_PKG_VERSION"));

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
    let tmp = dest.with_extension("part");
    let mut file = fs::File::create(&tmp)?;
    std::io::copy(&mut reader, &mut file)?;
    drop(file);
    fs::rename(&tmp, dest)?;
    Ok(())
}
