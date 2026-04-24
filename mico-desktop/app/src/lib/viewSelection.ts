import type { MicoState, Session, UISelection, Worktree } from "../types";

interface ViewSelection {
  repoId: string;
  sessionId: string;
  worktreeId: string;
}

function findWorktree(worktrees: Worktree[], worktreeId: string) {
  return worktrees.find((worktree) => worktree.id === worktreeId) ?? null;
}

function findSession(sessions: Session[], sessionId: string) {
  return sessions.find((session) => session.id === sessionId) ?? null;
}

function normalizeSelection(state: MicoState, selection: UISelection): ViewSelection {
  const repoId = state.repos.some((repo) => repo.id === selection.repoId)
    ? selection.repoId
    : (state.repos[0]?.id ?? "");

  const selectedWorktree = findWorktree(state.worktrees, selection.worktreeId);
  const worktreeId =
    selectedWorktree && selectedWorktree.repoId === repoId
      ? selectedWorktree.id
      : "";

  const selectedSession = findSession(state.sessions, selection.sessionId);
  const sessionId =
    selectedSession && selectedSession.worktreeId === worktreeId
      ? selectedSession.id
      : "";

  return { repoId, worktreeId, sessionId };
}

export function deriveViewSelection(state: MicoState): ViewSelection {
  return normalizeSelection(state, state.selection);
}
