use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, bail};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::ports::Updater;

const FORMULA_NAME: &str = "mico";
const PACKAGE_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

#[derive(Debug, Default, Clone)]
pub struct SystemUpdater;

#[derive(Debug, Deserialize)]
struct LatestReleaseResponse {
    tag_name: String,
}

impl SystemUpdater {
    pub fn new() -> Self {
        Self
    }

    fn brew_available() -> bool {
        Command::new("brew")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn brew_has_mico() -> bool {
        Command::new("brew")
            .args(["list", "--versions", FORMULA_NAME])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn run_checked(program: &str, args: &[&str]) -> anyhow::Result<()> {
        let status = Command::new(program)
            .args(args)
            .status()
            .with_context(|| format!("failed to run `{program}`"))?;

        if status.success() {
            Ok(())
        } else {
            bail!("`{program}` exited with status {status}")
        }
    }

    fn output_checked(program: &str, args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("failed to run `{program}`"))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            bail!("`{program}` failed: {detail}")
        }
    }

    fn install_dir() -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir().context("could not resolve home directory for install")?;
        Ok(home.join(".local").join("bin"))
    }

    fn valid_repo_component(component: &str) -> bool {
        !component.is_empty()
            && component
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    }

    fn validate_repo_slug(repo: &str) -> anyhow::Result<String> {
        let trimmed = repo.trim();
        let mut parts = trimmed.split('/');
        let owner = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();

        if parts.next().is_some()
            || !Self::valid_repo_component(owner)
            || !Self::valid_repo_component(name)
        {
            bail!("invalid GitHub repo slug `{trimmed}`; expected `owner/repo`")
        }

        Ok(trimmed.to_string())
    }

    fn github_repo_slug(configured_repo: Option<&str>) -> anyhow::Result<Option<String>> {
        if let Some(repo) = configured_repo
            && !repo.trim().is_empty()
        {
            return Self::validate_repo_slug(repo).map(Some);
        }

        let Some(without_scheme) = PACKAGE_REPOSITORY
            .strip_prefix("https://github.com/")
            .or_else(|| PACKAGE_REPOSITORY.strip_prefix("http://github.com/"))
        else {
            return Ok(None);
        };
        let repo = without_scheme
            .trim_end_matches('/')
            .trim_end_matches(".git");

        let resolved = if repo.is_empty() || repo.contains("REPLACE_ME") {
            None
        } else {
            Some(Self::validate_repo_slug(repo)?)
        };

        Ok(resolved)
    }

    fn temp_dir() -> PathBuf {
        env::temp_dir().join(format!("mico-install-{}", Uuid::new_v4().simple()))
    }

    fn latest_release_version(repo: &str) -> anyhow::Result<String> {
        let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let output = Self::output_checked(
            "curl",
            &[
                "-fsSL",
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                "User-Agent: mico",
                &api_url,
            ],
        )?;
        let response: LatestReleaseResponse =
            serde_json::from_str(&output).context("failed to parse GitHub release response")?;
        let version = response.tag_name.trim_start_matches('v').trim().to_string();

        if version.is_empty() {
            bail!("GitHub release did not include a valid tag_name")
        }

        Ok(version)
    }

    fn download_file(url: &str, destination: &Path) -> anyhow::Result<()> {
        let destination = destination.to_string_lossy().to_string();
        Self::run_checked("curl", &["-fsSL", url, "-o", &destination])
    }

    fn sha256(path: &Path) -> anyhow::Result<String> {
        let path = path.to_string_lossy().to_string();
        let output = Self::output_checked("shasum", &["-a", "256", &path])?;
        let checksum = output
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();

        if checksum.is_empty() {
            bail!("could not parse sha256 output for {}", path)
        }

        Ok(checksum)
    }

    fn expected_checksum(path: &Path) -> anyhow::Result<String> {
        let raw = fs::read_to_string(path)?;
        let checksum = raw
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();

        if checksum.is_empty() {
            bail!("checksum file was empty: {}", path.display())
        }

        Ok(checksum)
    }

    fn install_from_github_release(repo: &str) -> anyhow::Result<()> {
        let version = Self::latest_release_version(repo)?;
        let artifact = format!("mico-{version}-aarch64-apple-darwin");
        let archive = format!("{artifact}.tar.gz");
        let checksum = format!("{archive}.sha256");
        let release_base = format!("https://github.com/{repo}/releases/download/v{version}");
        let archive_url = format!("{release_base}/{archive}");
        let checksum_url = format!("{release_base}/{checksum}");
        let temp_dir = Self::temp_dir();
        let archive_path = temp_dir.join(&archive);
        let checksum_path = temp_dir.join(&checksum);
        let extracted_binary = temp_dir.join(&artifact).join("mico");
        let install_dir = Self::install_dir()?;
        let install_path = install_dir.join("mico");

        fs::create_dir_all(&temp_dir)?;
        fs::create_dir_all(&install_dir)?;

        let install_result = (|| -> anyhow::Result<()> {
            Self::download_file(&archive_url, &archive_path)?;
            Self::download_file(&checksum_url, &checksum_path)?;

            let actual = Self::sha256(&archive_path)?;
            let expected = Self::expected_checksum(&checksum_path)?;
            if actual != expected {
                bail!("download checksum mismatch for {archive}")
            }

            let archive_path = archive_path.to_string_lossy().to_string();
            let temp_dir_str = temp_dir.to_string_lossy().to_string();
            Self::run_checked("tar", &["-xzf", &archive_path, "-C", &temp_dir_str])?;

            if !extracted_binary.exists() {
                bail!(
                    "release artifact did not contain expected binary: {}",
                    extracted_binary.display()
                )
            }

            fs::copy(&extracted_binary, &install_path)
                .with_context(|| format!("failed to install {}", install_path.display()))?;
            let permissions = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&install_path, permissions)?;
            println!("mico {version} installed to {}", install_path.display());
            Ok(())
        })();

        let _ = fs::remove_dir_all(&temp_dir);
        install_result
    }
}

impl Updater for SystemUpdater {
    fn install_or_update(&self, github_repo: Option<&str>) -> anyhow::Result<()> {
        if Self::brew_available() && Self::brew_has_mico() {
            return Self::run_checked("brew", &["upgrade", "--formula", FORMULA_NAME]);
        }

        if let Some(repo) = Self::github_repo_slug(github_repo)? {
            return Self::install_from_github_release(&repo);
        }

        if Self::brew_available() {
            return Self::run_checked("brew", &["install", "--formula", FORMULA_NAME]);
        }

        bail!(
            "mico is not Homebrew-managed yet and no GitHub repo slug is configured or discoverable"
        )
    }
}
