import type { AppliedMigration, LogEvent, MicoState, Notification, Repo, Session, Worktree, WorkspaceFocus } from "../types";

function asArray<T>(value: unknown): T[] {
  return Array.isArray(value) ? (value as T[]) : [];
}

function asRecord(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" ? (value as Record<string, unknown>) : {};
}

function normalizeSelection(value: unknown): WorkspaceFocus {
  const selection = asRecord(value);
  return {
    repoId: typeof selection.repoId === "string" ? selection.repoId : "",
    worktreeId: typeof selection.worktreeId === "string" ? selection.worktreeId : "",
    sessionId: typeof selection.sessionId === "string" ? selection.sessionId : "",
    mode: selection.mode === "agent" ? "agent" : "effort",
  };
}

export function normalizeLogEvents(value: unknown): LogEvent[] {
  return asArray<LogEvent>(value);
}

export function normalizeStringList(value: unknown): string[] {
  return asArray<unknown>(value).filter((entry): entry is string => typeof entry === "string");
}

export function normalizeMicoState(value: unknown): MicoState {
  const state = asRecord(value);
  const version = state.version;
  return {
    version: typeof version === "number" && Number.isFinite(version) ? version : 1,
    repos: asArray<Repo>(state.repos),
    worktrees: asArray<Worktree>(state.worktrees),
    sessions: asArray<Session>(state.sessions),
    notifications: asArray<Notification>(state.notifications),
    selection: normalizeSelection(state.selection),
    logs: normalizeLogEvents(state.logs),
    migrations: asArray<AppliedMigration>(state.migrations),
  };
}
