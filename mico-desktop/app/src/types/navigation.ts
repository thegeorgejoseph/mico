export interface WorkspaceFocus {
  repoId: string;
  worktreeId: string;
  sessionId: string;
  mode: UIMode;
}

export type UISelection = WorkspaceFocus;

export type UIMode = "effort" | "agent";

export interface WorkspaceSearchResult {
  id: string;
  kind: "repo" | "worktree";
  label: string;
  meta: string;
  repoId?: string;
}
