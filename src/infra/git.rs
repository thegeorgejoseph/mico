use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, bail};
use uuid::Uuid;

use crate::{
    app::ports::RepoService,
    domain::model::{RepoTarget, WorktreeCheckout, WorktreeOwnership, WorktreePlan},
};

#[derive(Debug, Default, Clone)]
pub struct GitCliRepoService;

impl GitCliRepoService {
    pub fn new() -> Self {
        Self
    }

    fn output(repo_path: Option<&Path>, args: &[&str]) -> anyhow::Result<String> {
        let mut command = Command::new("git");

        if let Some(path) = repo_path {
            command.arg("-C").arg(path);
        }

        let output = command
            .args(args)
            .output()
            .with_context(|| format!("failed to run `git {}`", args.join(" ")))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            bail!("{}", String::from_utf8_lossy(&output.stderr).trim())
        }
    }

    fn status(repo_path: Option<&Path>, args: &[&str]) -> anyhow::Result<()> {
        let mut command = Command::new("git");

        if let Some(path) = repo_path {
            command.arg("-C").arg(path);
        }

        let output = command
            .args(args)
            .output()
            .with_context(|| format!("failed to run `git {}`", args.join(" ")))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            bail!("git {} failed: {detail}", args.join(" "))
        }
    }

    fn ref_exists(repo: &RepoTarget, reference: &str) -> bool {
        Self::output(Some(&repo.path), &["rev-parse", "--verify", reference]).is_ok()
    }

    fn slugify(input: &str) -> String {
        let mut slug = String::new();
        let mut last_dash = false;

        for ch in input.chars() {
            let normalized = if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            };

            if normalized == '-' {
                if !last_dash && !slug.is_empty() {
                    slug.push('-');
                }
                last_dash = true;
            } else {
                slug.push(normalized);
                last_dash = false;
            }
        }

        slug.trim_matches('-').to_string()
    }

    fn ensure_new_branch_available(repo: &RepoTarget, branch: &str) -> anyhow::Result<()> {
        if Self::ref_exists(repo, &format!("refs/heads/{branch}")) {
            bail!("branch `{branch}` already exists locally")
        }

        if Self::ref_exists(repo, &format!("refs/remotes/origin/{branch}")) {
            bail!("branch `{branch}` already exists on origin")
        }

        Ok(())
    }

    fn checked_out_branch_path(repo: &RepoTarget, branch: &str) -> Option<PathBuf> {
        let raw = Self::output(Some(&repo.path), &["worktree", "list", "--porcelain"]).ok()?;
        let needle = format!("refs/heads/{branch}");

        let mut current_path: Option<PathBuf> = None;
        let mut current_branch: Option<String> = None;

        for line in raw.lines().chain(std::iter::once("")) {
            let line = line.trim();

            if line.is_empty() {
                if current_branch.as_deref() == Some(needle.as_str()) {
                    return current_path;
                }
                current_path = None;
                current_branch = None;
                continue;
            }

            if let Some(path) = line.strip_prefix("worktree ") {
                current_path = Some(PathBuf::from(path));
            } else if let Some(reference) = line.strip_prefix("branch ") {
                current_branch = Some(reference.to_string());
            }
        }

        None
    }
}

impl RepoService for GitCliRepoService {
    fn discover_repo(&self, path: &Path, display_name: Option<&str>) -> anyhow::Result<RepoTarget> {
        let root = Self::output(Some(path), &["rev-parse", "--show-toplevel"])?;
        let canonical_path = PathBuf::from(root);
        let derived_name = display_name
            .map(ToOwned::to_owned)
            .or_else(|| {
                canonical_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .context("could not determine a display name for the repository")?;
        let slug = Self::slugify(&derived_name);

        if slug.is_empty() {
            bail!("could not derive a safe slug from repository name `{derived_name}`")
        }

        Ok(RepoTarget {
            id: Uuid::new_v4(),
            path: fs::canonicalize(canonical_path)?,
            display_name: derived_name,
            slug,
        })
    }

    fn current_branch(&self, path: &Path) -> anyhow::Result<Option<String>> {
        let branch = Self::output(Some(path), &["branch", "--show-current"])?;

        if branch.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(branch))
        }
    }

    fn list_branches(&self, repo: &RepoTarget) -> anyhow::Result<Vec<String>> {
        let raw = Self::output(
            Some(&repo.path),
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/remotes/origin",
                "refs/heads",
            ],
        )?;
        let branches = raw
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| *line != "origin/HEAD")
            .map(|line| line.strip_prefix("origin/").unwrap_or(line).to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(branches)
    }

    fn fetch_latest(&self, repo: &RepoTarget) -> anyhow::Result<()> {
        Self::status(Some(&repo.path), &["fetch", "--prune", "origin"])
    }

    fn plan_new_worktree(
        &self,
        repo: &RepoTarget,
        worktrees_root: &Path,
        branch: &str,
        base_branch: &str,
    ) -> anyhow::Result<WorktreePlan> {
        Self::ensure_new_branch_available(repo, branch)?;

        let base_ref = if Self::ref_exists(repo, &format!("refs/remotes/origin/{base_branch}")) {
            format!("origin/{base_branch}")
        } else if Self::ref_exists(repo, &format!("refs/heads/{base_branch}")) {
            base_branch.to_string()
        } else {
            bail!("base branch `{base_branch}` does not exist locally or on origin")
        };

        let branch_slug = Self::slugify(branch);

        if branch_slug.is_empty() {
            bail!("could not derive a safe directory name from branch `{branch}`")
        }

        let worktree_path = worktrees_root.join(&repo.slug).join(branch_slug);

        if worktree_path.exists() {
            bail!("worktree path already exists: {}", worktree_path.display())
        }

        Ok(WorktreePlan {
            branch: branch.to_string(),
            worktree_path,
            worktree_ownership: WorktreeOwnership::Managed,
            checkout: WorktreeCheckout::NewBranch { base_ref },
        })
    }

    fn plan_existing_worktree(
        &self,
        repo: &RepoTarget,
        worktrees_root: &Path,
        branch: &str,
    ) -> anyhow::Result<WorktreePlan> {
        let local_exists = Self::ref_exists(repo, &format!("refs/heads/{branch}"));
        let remote_exists = Self::ref_exists(repo, &format!("refs/remotes/origin/{branch}"));

        if !local_exists && !remote_exists {
            bail!("branch `{branch}` does not exist locally or on origin")
        }

        if local_exists && let Some(existing_path) = Self::checked_out_branch_path(repo, branch) {
            return Ok(WorktreePlan {
                branch: branch.to_string(),
                worktree_path: existing_path,
                worktree_ownership: WorktreeOwnership::External,
                checkout: WorktreeCheckout::ExistingCheckout,
            });
        }

        let branch_slug = Self::slugify(branch);

        if branch_slug.is_empty() {
            bail!("could not derive a safe directory name from branch `{branch}`")
        }

        let worktree_path = worktrees_root.join(&repo.slug).join(branch_slug);

        if worktree_path.exists() {
            bail!("worktree path already exists: {}", worktree_path.display())
        }

        let checkout = if local_exists {
            WorktreeCheckout::ExistingBranch { start_ref: None }
        } else {
            WorktreeCheckout::ExistingBranch {
                start_ref: Some(format!("origin/{branch}")),
            }
        };

        Ok(WorktreePlan {
            branch: branch.to_string(),
            worktree_path,
            worktree_ownership: WorktreeOwnership::Managed,
            checkout,
        })
    }

    fn create_worktree(&self, repo: &RepoTarget, plan: &WorktreePlan) -> anyhow::Result<()> {
        if matches!(plan.checkout, WorktreeCheckout::ExistingCheckout) {
            return Ok(());
        }

        fs::create_dir_all(
            plan.worktree_path
                .parent()
                .context("worktree path was missing a parent directory")?,
        )?;

        let path = plan.worktree_path.to_string_lossy().to_string();
        match &plan.checkout {
            WorktreeCheckout::NewBranch { base_ref } => Self::status(
                Some(&repo.path),
                &["worktree", "add", "-b", &plan.branch, &path, base_ref],
            ),
            WorktreeCheckout::ExistingBranch { start_ref } => {
                if let Some(start_ref) = start_ref {
                    Self::status(
                        Some(&repo.path),
                        &["worktree", "add", "-b", &plan.branch, &path, start_ref],
                    )
                } else {
                    Self::status(Some(&repo.path), &["worktree", "add", &path, &plan.branch])
                }
            }
            WorktreeCheckout::ExistingCheckout => Ok(()),
        }
    }

    fn remove_worktree(&self, repo: &RepoTarget, worktree_path: &Path) -> anyhow::Result<()> {
        let path = worktree_path.to_string_lossy().to_string();
        Self::status(Some(&repo.path), &["worktree", "remove", "--force", &path])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use tempfile::TempDir;

    fn git(repo_path: &Path, args: &[&str]) -> anyhow::Result<String> {
        GitCliRepoService::output(Some(repo_path), args)
    }

    fn git_ok(repo_path: &Path, args: &[&str]) -> anyhow::Result<()> {
        GitCliRepoService::status(Some(repo_path), args)
    }

    fn init_repo_fixture() -> anyhow::Result<(TempDir, RepoTarget, PathBuf, PathBuf, String)> {
        let temp = TempDir::new()?;
        let remote_path = temp.path().join("remote.git");
        let seed_path = temp.path().join("seed");
        let clone_path = temp.path().join("clone");
        let worktrees_root = temp.path().join("worktrees");

        GitCliRepoService::status(None, &["init", "--bare", remote_path.to_str().unwrap()])?;
        GitCliRepoService::status(
            None,
            &[
                "clone",
                remote_path.to_str().unwrap(),
                seed_path.to_str().unwrap(),
            ],
        )?;

        git_ok(&seed_path, &["config", "user.name", "Mico Tests"])?;
        git_ok(&seed_path, &["config", "user.email", "tests@mico.local"])?;

        fs::write(seed_path.join("README.md"), "seed\n")?;
        git_ok(&seed_path, &["add", "README.md"])?;
        git_ok(&seed_path, &["commit", "-m", "initial commit"])?;
        let default_branch = git(&seed_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        git_ok(&seed_path, &["push", "-u", "origin", &default_branch])?;

        GitCliRepoService::status(
            None,
            &[
                "clone",
                remote_path.to_str().unwrap(),
                clone_path.to_str().unwrap(),
            ],
        )?;

        let service = GitCliRepoService::new();
        let repo = service.discover_repo(&clone_path, Some("Test Repo"))?;
        Ok((temp, repo, seed_path, worktrees_root, default_branch))
    }

    #[test]
    fn slugify_normalizes_branch_names() {
        assert_eq!(
            GitCliRepoService::slugify("Feature/Add Search"),
            "feature-add-search"
        );
        assert_eq!(GitCliRepoService::slugify("  weird__name!! "), "weird-name");
    }

    #[test]
    fn current_branch_reports_checked_out_branch() -> anyhow::Result<()> {
        let (_temp, repo, seed_path, _worktrees_root, _default_branch) = init_repo_fixture()?;
        let service = GitCliRepoService::new();

        git_ok(&seed_path, &["checkout", "-b", "branch-drift-check"])?;
        git_ok(&seed_path, &["push", "-u", "origin", "branch-drift-check"])?;
        service.fetch_latest(&repo)?;
        git_ok(&repo.path, &["checkout", "branch-drift-check"])?;

        assert_eq!(
            service.current_branch(&repo.path)?,
            Some("branch-drift-check".to_string())
        );
        Ok(())
    }

    #[test]
    fn plan_new_worktree_prefers_origin_base_ref_when_available() -> anyhow::Result<()> {
        let (_temp, repo, _seed_path, worktrees_root, default_branch) = init_repo_fixture()?;
        let service = GitCliRepoService::new();

        let plan =
            service.plan_new_worktree(&repo, &worktrees_root, "feature-a", &default_branch)?;

        assert_eq!(plan.branch, "feature-a");
        assert_eq!(plan.worktree_ownership, WorktreeOwnership::Managed);
        assert_eq!(
            plan.checkout,
            WorktreeCheckout::NewBranch {
                base_ref: format!("origin/{default_branch}"),
            }
        );
        Ok(())
    }

    #[test]
    fn plan_existing_worktree_uses_origin_ref_for_remote_only_branch() -> anyhow::Result<()> {
        let (_temp, repo, seed_path, worktrees_root, _default_branch) = init_repo_fixture()?;
        let service = GitCliRepoService::new();

        git_ok(&seed_path, &["checkout", "-b", "remote-only"])?;
        fs::write(seed_path.join("feature.txt"), "remote branch\n")?;
        git_ok(&seed_path, &["add", "feature.txt"])?;
        git_ok(&seed_path, &["commit", "-m", "remote only"])?;
        git_ok(&seed_path, &["push", "-u", "origin", "remote-only"])?;
        service.fetch_latest(&repo)?;

        let plan = service.plan_existing_worktree(&repo, &worktrees_root, "remote-only")?;

        assert_eq!(plan.branch, "remote-only");
        assert_eq!(plan.worktree_ownership, WorktreeOwnership::Managed);
        assert_eq!(
            plan.checkout,
            WorktreeCheckout::ExistingBranch {
                start_ref: Some("origin/remote-only".to_string()),
            }
        );
        Ok(())
    }

    #[test]
    fn create_worktree_from_remote_branch_creates_local_branch() -> anyhow::Result<()> {
        let (_temp, repo, seed_path, worktrees_root, _default_branch) = init_repo_fixture()?;
        let service = GitCliRepoService::new();

        git_ok(&seed_path, &["checkout", "-b", "remote-only"])?;
        fs::write(seed_path.join("feature.txt"), "remote branch\n")?;
        git_ok(&seed_path, &["add", "feature.txt"])?;
        git_ok(&seed_path, &["commit", "-m", "remote only"])?;
        git_ok(&seed_path, &["push", "-u", "origin", "remote-only"])?;
        service.fetch_latest(&repo)?;

        let plan = service.plan_existing_worktree(&repo, &worktrees_root, "remote-only")?;
        service.create_worktree(&repo, &plan)?;

        let head = git(&plan.worktree_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        let branches = git(&plan.worktree_path, &["branch", "--list", "remote-only"])?;

        assert_eq!(head, "remote-only");
        assert_eq!(branches.trim(), "* remote-only");
        Ok(())
    }

    #[test]
    fn plan_existing_worktree_adopts_existing_checkout_when_branch_is_already_in_use()
    -> anyhow::Result<()> {
        let (_temp, repo, _seed_path, worktrees_root, default_branch) = init_repo_fixture()?;
        let service = GitCliRepoService::new();

        let plan = service.plan_existing_worktree(&repo, &worktrees_root, &default_branch)?;

        assert_eq!(plan.branch, default_branch);
        assert_eq!(plan.worktree_ownership, WorktreeOwnership::External);
        assert_eq!(plan.checkout, WorktreeCheckout::ExistingCheckout);
        assert_eq!(plan.worktree_path, repo.path);
        Ok(())
    }
}
