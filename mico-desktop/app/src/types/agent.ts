import type { AgentKind } from "./domain";
import type { WorkspaceFocus } from "./navigation";

export type AgentToolName =
  | "select_repo"
  | "select_worktree"
  | "select_session"
  | "list_repos"
  | "list_worktrees"
  | "list_sessions";

export interface SelectRepoTool {
  repoId: string;
}

export interface SelectWorktreeTool {
  worktreeId: string;
}

export interface SelectSessionTool {
  sessionId: string;
}

export interface AgentToolCall {
  tool: AgentToolName;
  reason: string;
  selectRepo?: SelectRepoTool;
  selectWorktree?: SelectWorktreeTool;
  selectSession?: SelectSessionTool;
  listRepos?: Record<string, never>;
  listWorktrees?: Record<string, never>;
  listSessions?: Record<string, never>;
}

export interface AgentRunInput {
  provider: AgentKind;
  message: string;
}

export interface AgentRunResponse {
  provider: AgentKind;
  message: string;
  toolCall?: AgentToolCall;
  result?: {
    ok: boolean;
    message: string;
  };
  selection?: WorkspaceFocus;
}
