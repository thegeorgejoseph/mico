#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use anyhow::{Context, bail};
use semver::Version;
use toml_edit::{DocumentMut, value};

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    match command.as_str() {
        "release" => {
            let spec = args
                .next()
                .context("usage: cargo run -p xtask -- release <patch|minor|major|VERSION>")?;
            release(&spec)
        }
        "update-tap" => {
            let version = args
                .next()
                .context("usage: cargo run -p xtask -- update-tap <VERSION>")?;
            update_tap(&version)
        }
        "ship" => {
            let spec = args
                .next()
                .context("usage: cargo run -p xtask -- ship <patch|minor|major|VERSION>")?;
            ship(&spec)
        }
        _ => {
            print_usage();
            bail!("unknown xtask command `{command}`")
        }
    }
}

fn print_usage() {
    eprintln!("xtask commands:");
    eprintln!("  release <patch|minor|major|VERSION>");
    eprintln!("  update-tap <VERSION>");
    eprintln!("  ship <patch|minor|major|VERSION>");
}

fn release(spec: &str) -> anyhow::Result<()> {
    let root = project_root()?;
    ensure_main_branch(&root)?;
    ensure_clean_tree(&root)?;

    let cargo_manifest = root.join("Cargo.toml");
    let current = load_version(&cargo_manifest)?;
    let next = next_version(&current, spec)?;

    println!("releasing mico {current} -> {next}");
    write_version(&cargo_manifest, &next)?;

    run_checked(&root, "cargo", &["generate-lockfile"])?;
    run_checked(&root, "cargo", &["fmt", "--all"])?;
    run_checked(&root, "cargo", &["check", "--workspace", "--all-targets"])?;
    run_checked(&root, "cargo", &["test", "--workspace", "--all-targets"])?;
    run_checked(&root, "git", &["add", "Cargo.toml", "Cargo.lock"])?;
    run_checked(
        &root,
        "git",
        &["commit", "-m", &format!("chore: release v{next}")],
    )?;
    run_checked(
        &root,
        "git",
        &["tag", "-a", &format!("v{next}"), "-m", &format!("v{next}")],
    )?;
    run_checked(&root, "git", &["push", "origin", "main", "--follow-tags"])?;

    Ok(())
}

fn ship(spec: &str) -> anyhow::Result<()> {
    let root = project_root()?;
    let cargo_manifest = root.join("Cargo.toml");
    let current = load_version(&cargo_manifest)?;
    let next = next_version(&current, spec)?;

    release(spec)?;
    update_tap(&next.to_string())
}

fn project_root() -> anyhow::Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest did not have a parent directory")
}

fn ensure_main_branch(root: &Path) -> anyhow::Result<()> {
    let branch = output_checked(root, "git", &["branch", "--show-current"])?;

    if branch.trim() == "main" {
        Ok(())
    } else {
        bail!("release must run from `main`, found `{}`", branch.trim())
    }
}

fn ensure_clean_tree(root: &Path) -> anyhow::Result<()> {
    let status = Command::new("git")
        .args(["status", "--short"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .output()
        .context("failed to inspect git status")?;

    if !status.status.success() {
        bail!("git status failed")
    }

    if String::from_utf8_lossy(&status.stdout).trim().is_empty() {
        Ok(())
    } else {
        bail!("working tree must be clean before releasing")
    }
}

fn load_version(path: &Path) -> anyhow::Result<Version> {
    let raw = fs::read_to_string(path)?;
    let document = raw.parse::<DocumentMut>()?;
    let version = document["package"]["version"]
        .as_str()
        .context("package.version was missing from Cargo.toml")?;
    Version::parse(version).context("package.version was not valid semver")
}

fn write_version(path: &Path, next: &Version) -> anyhow::Result<()> {
    let raw = fs::read_to_string(path)?;
    let mut document = raw.parse::<DocumentMut>()?;
    document["package"]["version"] = value(next.to_string());
    fs::write(path, document.to_string())?;
    Ok(())
}

fn update_tap(version: &str) -> anyhow::Result<()> {
    let root = project_root()?;
    let tap_root = tap_root(&root)?;
    ensure_clean_tree(&tap_root)?;

    let repository = load_repository(&root.join("Cargo.toml"))?;
    let formula_path = tap_root.join("Formula").join("mico.rb");
    let checksum = fetch_release_checksum(&repository, version)?;

    update_formula(&formula_path, version, &repository, &checksum)?;
    run_checked(&tap_root, "git", &["add", "Formula/mico.rb"])?;
    run_checked(
        &tap_root,
        "git",
        &["commit", "-m", &format!("mico {version}")],
    )?;
    run_checked(&tap_root, "git", &["push"])?;

    Ok(())
}

fn next_version(current: &Version, spec: &str) -> anyhow::Result<Version> {
    let mut next = current.clone();

    match spec {
        "patch" => next.patch += 1,
        "minor" => {
            next.minor += 1;
            next.patch = 0;
        }
        "major" => {
            next.major += 1;
            next.minor = 0;
            next.patch = 0;
        }
        exact => return Version::parse(exact).context("invalid exact semver version"),
    }

    Ok(next)
}

fn tap_root(root: &Path) -> anyhow::Result<PathBuf> {
    if let Ok(path) = env::var("MICO_HOMEBREW_TAP_DIR") {
        return Ok(PathBuf::from(path));
    }

    root.parent()
        .map(|parent| parent.join("homebrew-tap"))
        .context("project root did not have a parent directory")
}

fn load_repository(path: &Path) -> anyhow::Result<String> {
    let raw = fs::read_to_string(path)?;
    let document = raw.parse::<DocumentMut>()?;
    document["package"]["repository"]
        .as_str()
        .map(str::to_string)
        .context("package.repository was missing from Cargo.toml")
}

fn fetch_release_checksum(repository: &str, version: &str) -> anyhow::Result<String> {
    let checksum_url = format!(
        "{repository}/releases/download/v{version}/mico-{version}-aarch64-apple-darwin.tar.gz.sha256"
    );

    for attempt in 1..=24 {
        match output_checked(Path::new("."), "curl", &["-fsSL", &checksum_url]) {
            Ok(output) => {
                let checksum = output
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                if checksum.is_empty() {
                    bail!("release checksum file was empty for v{version}")
                }
                return Ok(checksum);
            }
            Err(error) if attempt < 24 => {
                println!("waiting for release artifact checksum (attempt {attempt}/24): {error}");
                thread::sleep(Duration::from_secs(5));
            }
            Err(error) => return Err(error),
        }
    }

    bail!("release checksum did not become available for v{version}")
}

fn update_formula(
    formula_path: &Path,
    version: &str,
    repository: &str,
    checksum: &str,
) -> anyhow::Result<()> {
    let raw = fs::read_to_string(formula_path)?;
    let release_url = format!(
        "{repository}/releases/download/v{version}/mico-{version}-aarch64-apple-darwin.tar.gz"
    );

    let updated = raw
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("url ") {
                format!("  url \"{release_url}\"")
            } else if trimmed.starts_with("sha256 ") {
                format!("  sha256 \"{checksum}\"")
            } else if trimmed.starts_with("version ") {
                format!("  version \"{version}\"")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(formula_path, format!("{updated}\n"))?;
    Ok(())
}

fn run_checked(root: &Path, program: &str, args: &[&str]) -> anyhow::Result<()> {
    println!("> {} {}", program, args.join(" "));

    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .with_context(|| format!("failed to run `{program}`"))?;

    if status.success() {
        Ok(())
    } else {
        bail!("`{program}` exited with status {status}")
    }
}

fn output_checked(root: &Path, program: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to run `{program}`"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        bail!("`{program}` exited with status {}", output.status)
    }
}
