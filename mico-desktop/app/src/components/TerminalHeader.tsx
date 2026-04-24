import { Bot, ChevronRight, SquareTerminal } from "lucide-react";

import { Button } from "./Button";
import { SelectField } from "./Field";
import { handleScopedNavigation } from "../lib/navigation";
import type { AgentKind, Session, Worktree } from "../types";

interface TerminalHeaderProps {
  agent: AgentKind;
  canResume: boolean;
  canStop: boolean;
  selectedSession: Session | null;
  selectedWorktree: Worktree | null;
  sessions: Session[];
  resumeSession: () => void;
  setAgent: (agent: AgentKind) => void;
  setSelectedSessionId: (id: string) => void;
  startSession: () => void;
  stopSession: () => void;
}

export function TerminalHeader({
  agent,
  canResume,
  canStop,
  resumeSession,
  selectedSession,
  selectedWorktree,
  sessions,
  setAgent,
  setSelectedSessionId,
  startSession,
  stopSession,
}: TerminalHeaderProps) {
  return (
    <div className="terminal-header">
      <div className="terminal-header__title">
        <span className="terminal-header__badge">
          <SquareTerminal size={14} />
        </span>
        <div>
          <h2>{selectedWorktree?.branch ?? "Terminal"}</h2>
          <p>{selectedSession?.sessionName ?? "Start a durable tmux-backed session."}</p>
        </div>
      </div>
      <div className="terminal-header__controls">
        <SelectField onChange={(event) => setAgent(event.target.value as AgentKind)} value={agent}>
          <option value="terminal">Terminal</option>
          <option value="codex">Codex</option>
          <option value="claude">Claude</option>
        </SelectField>
        <div className="terminal-header__actions">
          {canResume ? (
            <Button onClick={resumeSession} type="button" variant="default">
              Resume
            </Button>
          ) : null}
          {canStop ? (
            <Button onClick={stopSession} type="button" variant="ghost">
              Stop
            </Button>
          ) : null}
          <Button disabled={!selectedWorktree} onClick={startSession} type="button" variant="primary">
            Start
          </Button>
        </div>
      </div>
      {sessions.length ? (
        <div className="session-tabs" data-nav-scope="session-tabs">
          {sessions.map((session) => (
            <button
              className={session.id === selectedSession?.id ? "is-active" : ""}
              data-nav-item="true"
              key={session.id}
              onClick={() => setSelectedSessionId(session.id)}
              onKeyDown={(event) => handleScopedNavigation(event, "horizontal")}
              type="button"
            >
              <span className="session-tabs__icon">{session.agent === "terminal" ? <SquareTerminal size={13} /> : <Bot size={13} />}</span>
              <span>{session.agent}</span>
              <ChevronRight size={14} />
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
