import type { AgentRunInput, AgentRunResponse } from "./agent";
import type { LogEvent, MicoState, Notification, Repo, Session, Worktree } from "./domain";
import type { WorkspaceFocus, WorkspaceSearchResult } from "./navigation";
import type { AppInfo, DoctorReport, UpdateInfo } from "./system";

export interface AddRepoInput {
  path: string;
  name?: string;
}

export interface CreateWorktreeInput {
  repoId: string;
  branch: string;
  base: string;
  existing?: boolean;
}

export interface StartSessionInput {
  worktreeId: string;
  agent: Session["agent"];
}

export interface FocusWorkspaceInput {
  repoId?: string;
  worktreeId?: string;
  sessionId?: string;
  mode?: WorkspaceFocus["mode"];
}

export interface MicoApi {
  platform: string;
  appInfo: () => Promise<AppInfo>;
  checkForUpdates: () => Promise<UpdateInfo>;
  openUpdate: (targetURL?: string) => Promise<{ ok: boolean }>;
  state: () => Promise<MicoState>;
  doctor: () => Promise<DoctorReport>;
  branches: (repoId: string) => Promise<string[]>;
  refreshRepo: (repoId: string) => Promise<{ ok: boolean }>;
  captureTerminal: (sessionId: string) => Promise<{ lines: string[] }>;
  logs: () => Promise<LogEvent[]>;
  terminalSocketURL: (sessionId: string) => string;
  addRepo: (input: AddRepoInput) => Promise<Repo>;
  runAgent: (input: AgentRunInput) => Promise<AgentRunResponse>;
  createWorktree: (input: CreateWorktreeInput) => Promise<Worktree>;
  pickRepoFolder: () => Promise<string | null>;
  searchWorkspace: (query: string, limit?: number) => Promise<WorkspaceSearchResult[]>;
  focusWorkspace: (input: FocusWorkspaceInput) => Promise<WorkspaceFocus>;
  dismissNotification: (id: string) => Promise<{ ok: boolean }>;
  sendTerminalInput: (sessionId: string, text: string) => Promise<{ ok: boolean }>;
  startSession: (input: StartSessionInput) => Promise<Session>;
  stopSession: (sessionId: string) => Promise<Session>;
  resumeSession: (sessionId: string) => Promise<Session>;
}
