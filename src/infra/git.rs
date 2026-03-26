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
    domain::model::{RepoTarget, WorktreeCheckout, WorktreePlan},
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
            checkout,
        })
    }

    fn create_worktree(&self, repo: &RepoTarget, plan: &WorktreePlan) -> anyhow::Result<()> {
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
        }
    }

    fn remove_worktree(&self, repo: &RepoTarget, worktree_path: &Path) -> anyhow::Result<()> {
        let path = worktree_path.to_string_lossy().to_string();
        Self::status(Some(&repo.path), &["worktree", "remove", "--force", &path])
    }
}
