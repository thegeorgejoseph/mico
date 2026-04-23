import { ChevronDown, ChevronRight, FolderGit2, GitBranch, LoaderCircle, Rocket, Search, SquareTerminal } from "lucide-react";
import type { FormEvent, ReactNode } from "react";

import { Button } from "./Button";
import { SelectField, TextField } from "./Field";
import { ListItem } from "./ListItem";
import { handleScopedNavigation } from "../lib/navigation";
import type { AppInfo, Repo, Worktree } from "../types";

export function SplashScreen({
  appInfo,
  error,
  fading,
  markUrl,
  status,
  wordmark,
}: {
  appInfo: AppInfo | null;
  error: string;
  fading: boolean;
  markUrl: string;
  status: string;
  wordmark: string;
}) {
  return (
    <div className={`splash-screen ${fading ? "is-fading" : ""}`} role="status" aria-live="polite">
      <div className="splash-screen__glow" />
      <div className="splash-screen__grid" />
      <div className="splash-screen__content">
        <div className="splash-screen__hero">
          <img className="splash-screen__mark" src={markUrl} alt="" />
          <pre className="splash-screen__ascii">{wordmark}</pre>
          <div className="splash-screen__label">
            <strong>mico</strong>
            <span>mission control for local agent work</span>
          </div>
        </div>
        <div className={`splash-screen__status ${error ? "is-error" : ""}`}>
          <LoaderCircle size={14} />
          <span>{error || status}</span>
          <em>v{appInfo?.version ?? "1.0.0"}</em>
        </div>
      </div>
    </div>
  );
}

interface NavSectionProps {
  compact: boolean;
  expanded: boolean;
  onAdd: () => void;
  repos: Repo[];
  selectedRepoId: string;
  onSelect: (id: string) => void;
  onToggleExpanded: () => void;
}

export function NavSection({ compact, expanded, onAdd, repos, selectedRepoId, onSelect, onToggleExpanded }: NavSectionProps) {
  const selectedRepo = repos.find((repo) => repo.id === selectedRepoId) ?? null;

  if (compact) {
    return (
      <section className="nav-section is-compact">
        <div className="compact-project-list" data-nav-scope="projects">
          {repos.map((repo) => (
            <button
              aria-label={`Select project ${repo.name}`}
              className={`compact-project ${repo.id === selectedRepoId ? "is-selected" : ""}`}
              data-nav-item="true"
              key={repo.id}
              onClick={() => onSelect(repo.id)}
              onKeyDown={(event) => handleScopedNavigation(event)}
              title={repo.name}
              type="button"
            >
              <span>{repo.name.slice(0, 1).toUpperCase()}</span>
            </button>
          ))}
        </div>
      </section>
    );
  }

  return (
    <section className={`nav-section ${expanded ? "is-expanded" : "is-collapsed"}`}>
      <div className="section-title">
        <button
          className="section-title__toggle"
          onClick={onToggleExpanded}
          type="button"
          aria-expanded={expanded}
          aria-label={expanded ? "Collapse projects" : "Expand projects"}
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <div className="section-label">
            <FolderGit2 size={14} />
            <h2>Projects</h2>
            <span>{repos.length}</span>
          </div>
        </button>
        <div className="section-title__actions">
          <button onClick={onAdd} type="button" title="Add project">
            +
          </button>
        </div>
      </div>
      {expanded ? (
        <div className="list" data-nav-scope="projects">
          {repos.map((repo) => (
            <ListItem
              icon={<FolderGit2 size={14} />}
              key={repo.id}
              onClick={() => onSelect(repo.id)}
              selected={repo.id === selectedRepoId}
              title={repo.name}
            >
              {repo.path}
            </ListItem>
          ))}
        </div>
      ) : (
        <button className="nav-section__collapsed" onClick={onToggleExpanded} type="button">
          <span className="nav-section__collapsed-title">{selectedRepo?.name ?? "Projects"}</span>
          <span className="nav-section__collapsed-meta">{selectedRepo?.path ?? `${repos.length} tracked projects`}</span>
        </button>
      )}
    </section>
  );
}

interface RepoFormProps {
  onPickFolder: () => void;
  repoName: string;
  repoPath: string;
  setRepoName: (value: string) => void;
  setRepoPath: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}

export function RepoForm({ onPickFolder, repoName, repoPath, setRepoName, setRepoPath, onSubmit }: RepoFormProps) {
  return (
    <form className="modal-form" onSubmit={onSubmit}>
      <label>
        <span>Project path</span>
        <div className="path-row">
          <TextField onChange={(event) => setRepoPath(event.target.value)} placeholder="/Users/you/code/repo" value={repoPath} />
          <Button onClick={onPickFolder} type="button">
            Browse
          </Button>
        </div>
      </label>
      <label>
        <span>Display name</span>
        <TextField onChange={(event) => setRepoName(event.target.value)} placeholder="Defaults to folder name" value={repoName} />
      </label>
      <Button disabled={!repoPath} type="submit" variant="primary">
        Track Project
      </Button>
    </form>
  );
}

interface WorktreeCreatorProps {
  base: string;
  branch: string;
  branches: string[];
  branchesLoading: boolean;
  disabled: boolean;
  existing: boolean;
  onBaseChange: (value: string) => void;
  onBranchChange: (value: string) => void;
  onExistingChange: (value: boolean) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}

export function WorktreeCreator({ base, branch, branches, branchesLoading, disabled, existing, onBaseChange, onBranchChange, onExistingChange, onSubmit }: WorktreeCreatorProps) {
  return (
    <form className="modal-form" onSubmit={onSubmit}>
      <label>
        <span>Branch strategy</span>
        <div className="worktree-mode" data-nav-scope="worktree-mode">
          <button className={!existing ? "is-active" : ""} data-nav-item="true" onClick={() => onExistingChange(false)} onKeyDown={(event) => handleScopedNavigation(event, "horizontal")} type="button">
            New branch
          </button>
          <button className={existing ? "is-active" : ""} data-nav-item="true" onClick={() => onExistingChange(true)} onKeyDown={(event) => handleScopedNavigation(event, "horizontal")} type="button">
            Existing branch
          </button>
        </div>
      </label>
      {!existing ? (
        <label>
          <span>Base branch</span>
          <SelectField disabled={disabled || branchesLoading || branches.length === 0} onChange={(event) => onBaseChange(event.target.value)} value={base}>
            {branches.map((candidate) => (
              <option key={candidate} value={candidate}>
                {candidate}
              </option>
            ))}
          </SelectField>
          {branchesLoading ? <small>Refreshing available branches...</small> : null}
        </label>
      ) : null}
      <label>
        <span>{existing ? "Branch" : "New branch"}</span>
        {existing ? (
          <SelectField disabled={disabled || branchesLoading || branches.length === 0} onChange={(event) => onBranchChange(event.target.value)} value={branch}>
            <option value="">Choose a branch</option>
            {branches.map((candidate) => (
              <option key={candidate} value={candidate}>
                {candidate}
              </option>
            ))}
          </SelectField>
        ) : (
          <TextField disabled={disabled} onChange={(event) => onBranchChange(event.target.value)} placeholder="feature/my-task" value={branch} />
        )}
      </label>
      <Button disabled={disabled || (!existing && !base) || !branch} type="submit" variant="primary">
        Create Worktree
      </Button>
    </form>
  );
}

interface WorktreeListProps {
  onAdd: () => void;
  selectedWorktreeId: string;
  worktrees: Worktree[];
  onSelect: (id: string) => void;
}

export function WorktreeList({ onAdd, selectedWorktreeId, worktrees, onSelect }: WorktreeListProps) {
  return (
    <section className="panel-group panel-group--flush">
      <div className="panel-group__header">
        <div className="section-title">
          <div className="section-label">
            <GitBranch size={14} />
            <h2>Worktrees</h2>
          </div>
          <button onClick={onAdd} type="button" title="Create worktree">
            +
          </button>
        </div>
        <p>Durable local checkouts managed by mico desktop.</p>
      </div>
      <div className="list list--large" data-nav-scope="worktrees">
        {worktrees.map((worktree) => (
          <ListItem
            eyebrow={worktree.status}
            icon={<GitBranch size={14} />}
            key={worktree.id}
            meta={`base ${worktree.base}`}
            onClick={() => onSelect(worktree.id)}
            selected={worktree.id === selectedWorktreeId}
            title={worktree.branch}
          >
            {worktree.path}
          </ListItem>
        ))}
      </div>
    </section>
  );
}

interface SummaryMetricProps {
  icon: ReactNode;
  label: string;
  value: number;
}

export function SummaryMetric({ icon, label, value }: SummaryMetricProps) {
  return (
    <div className="summary-metric">
      <div className="summary-metric__label">
        <span className="summary-metric__icon">{icon}</span>
        <span>{label}</span>
      </div>
      <strong>{value}</strong>
    </div>
  );
}
