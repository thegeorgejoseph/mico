import { Bell, FolderGit2, GitBranch, GitPullRequestArrow, RefreshCw, Rocket, SquareTerminal } from "lucide-react";
import type { CSSProperties } from "react";

import { SummaryMetric, WorktreeList } from "./SidebarPanels";
import { AmbientChip, type StatusNotice } from "./StatusChip";
import { TerminalHeader } from "./TerminalHeader";
import { TerminalView } from "./TerminalView";
import { ActivityPanel } from "./ActivityPanel";
import type { AgentKind, Repo, Session, ThemeName, Worktree } from "../types";

interface WorkspaceViewProps {
  activityOpen: boolean;
  agent: AgentKind;
  branchesCount: number;
  canResumeSession: boolean;
  canStopSession: boolean;
  contentGridStyle: CSSProperties;
  notificationsCount: number;
  onOpenMission: () => void;
  onRefreshProject: () => void;
  onSelectSession: (sessionId: string) => void;
  onStartSession: () => void;
  onStopSession: () => void;
  onResumeSession: () => void;
  onToggleActivity: () => void;
  onToggleNotifications: () => void;
  onToggleInspectorResize: (event: React.PointerEvent<HTMLDivElement>) => void;
  onAddWorktree: () => void;
  onSelectWorktree: (worktreeId: string) => void;
  repoWorktrees: Worktree[];
  selectedRepo: Repo | null;
  selectedSession: Session | null;
  selectedWorktree: Worktree | null;
  sessionsForWorktree: Session[];
  setAgent: (agent: AgentKind) => void;
  statusNotice: StatusNotice | null;
  logs: import("../types").LogEvent[];
  theme: ThemeName;
}

export function WorkspaceView({
  activityOpen,
  agent,
  branchesCount,
  canResumeSession,
  canStopSession,
  contentGridStyle,
  logs,
  notificationsCount,
  onAddWorktree,
  onOpenMission,
  onRefreshProject,
  onResumeSession,
  onSelectSession,
  onSelectWorktree,
  onStartSession,
  onStopSession,
  onToggleActivity,
  onToggleInspectorResize,
  onToggleNotifications,
  repoWorktrees,
  selectedRepo,
  selectedSession,
  selectedWorktree,
  sessionsForWorktree,
  setAgent,
  statusNotice,
  theme,
}: WorkspaceViewProps) {
  return (
    <main className="workspace">
      <header className="toolbar">
        <div className="toolbar__leading">
          <div className="toolbar__context">
            <h2>{selectedRepo?.name ?? "Add a project"}</h2>
            <p>
              {selectedWorktree
                ? `${selectedWorktree.branch} · ${sessionsForWorktree.length} session(s)`
                : selectedRepo
                  ? `${repoWorktrees.length} worktree(s) in this project`
                  : "Choose a local git project to begin."}
            </p>
          </div>
        </div>
        <div className="toolbar__actions">
          <button className="toolbar-icon toolbar-icon--wide" onClick={onOpenMission} type="button" title="Open Mission command palette (⌘K)">
            <Rocket size={16} />
            <span>Mission</span>
            <kbd>⌘K</kbd>
          </button>
          <button className="toolbar-icon" onClick={onRefreshProject} type="button" aria-label="Refresh project" title="Refresh project">
            <RefreshCw size={16} />
          </button>
          <AmbientChip notice={statusNotice} session={selectedSession} />
          <button
            className="toolbar-icon"
            onClick={onToggleNotifications}
            type="button"
            aria-label="Open notifications"
            title="Open notifications"
          >
            <Bell size={15} />
            {notificationsCount ? <i>{notificationsCount}</i> : null}
          </button>
        </div>
      </header>

      <div className="content-grid" style={contentGridStyle}>
        <section className="inspector">
          <section className="panel-group repo-summary">
            <div className="panel-group__header">
              <div className="panel-heading">
                <FolderGit2 size={15} />
                <h2>{selectedRepo?.name ?? "No project selected"}</h2>
              </div>
              <p>{selectedRepo?.path ?? "Use the + button next to Projects."}</p>
            </div>
            <div className="summary-grid">
              <SummaryMetric icon={<GitPullRequestArrow size={14} />} label="Branches" value={branchesCount} />
              <SummaryMetric icon={<GitBranch size={14} />} label="Worktrees" value={repoWorktrees.length} />
              <SummaryMetric icon={<SquareTerminal size={14} />} label="Sessions" value={sessionsForWorktree.length} />
            </div>
          </section>
          <WorktreeList
            selectedWorktreeId={selectedWorktree?.id ?? ""}
            worktrees={repoWorktrees}
            onAdd={onAddWorktree}
            onSelect={onSelectWorktree}
          />
        </section>
        <div
          aria-label="Resize inspector"
          className="resize-handle resize-handle--inspector"
          onPointerDown={onToggleInspectorResize}
          role="separator"
        />

        <div className="main-stack">
          <section className="terminal-card">
            <TerminalHeader
              agent={agent}
              canResume={canResumeSession}
              canStop={canStopSession}
              resumeSession={onResumeSession}
              selectedSession={selectedSession}
              selectedWorktree={selectedWorktree}
              sessions={sessionsForWorktree}
              setAgent={setAgent}
              setSelectedSessionId={onSelectSession}
              startSession={onStartSession}
              stopSession={onStopSession}
            />
            <TerminalView session={selectedSession} theme={theme} />
          </section>
          <ActivityPanel logs={logs} open={activityOpen} onToggle={onToggleActivity} />
        </div>
      </div>
    </main>
  );
}
