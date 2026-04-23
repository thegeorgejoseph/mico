import type { WorkspaceFocus } from "./navigation";

export interface Repo {
  id: string;
  name: string;
  path: string;
  createdAt: string;
}

export type WorktreeStatus = "ready" | "running" | "stopped";

export interface Worktree {
  id: string;
  repoId: string;
  branch: string;
  base: string;
  path: string;
  status: WorktreeStatus;
  createdAt: string;
  updatedAt: string;
}

export type SessionStatus = "running" | "exited" | "failed";

export interface Session {
  id: string;
  worktreeId: string;
  agent: AgentKind;
  command: string[];
  sessionName: string;
  status: SessionStatus;
  createdAt: string;
  updatedAt: string;
  exitCode?: number;
}

export type NotificationLevel = "info" | "success" | "warning" | "error";

export interface Notification {
  id: string;
  level: NotificationLevel;
  title: string;
  body: string;
  seen: boolean;
  createdAt: string;
}

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogEvent {
  id: string;
  level: LogLevel;
  scope: string;
  action: string;
  message: string;
  fields?: Record<string, string>;
  createdAt: string;
}

export interface AppliedMigration {
  id: string;
  appliedAt: string;
}

export interface MicoState {
  version: number;
  repos: Repo[];
  worktrees: Worktree[];
  sessions: Session[];
  notifications: Notification[];
  selection: WorkspaceFocus;
  logs: LogEvent[];
  migrations?: AppliedMigration[];
}

export const AGENT_KINDS = {
  TERMINAL: "terminal",
  CODEX: "codex",
  CLAUDE: "claude",
} as const;

export type AgentKind = (typeof AGENT_KINDS)[keyof typeof AGENT_KINDS];
