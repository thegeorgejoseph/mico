#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
        _ => {
            print_usage();
            bail!("unknown xtask command `{command}`")
        }
    }
}

fn print_usage() {
    eprintln!("xtask commands:");
    eprintln!("  release <patch|minor|major|VERSION>");
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
