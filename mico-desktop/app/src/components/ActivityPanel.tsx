import { useMemo } from "react";
import { Bot, ChevronRight, FolderGit2, GitBranch, RefreshCw, SquareTerminal } from "lucide-react";

import { capitalize } from "../lib/format";
import type { LogEvent } from "../types";

interface ActivityPanelProps {
  logs: LogEvent[];
  onToggle: () => void;
  open: boolean;
}

export function ActivityPanel({ logs, onToggle, open }: ActivityPanelProps) {
  const visibleLogs = useMemo(() => logs.filter((event) => event.level !== "debug").slice(0, 18), [logs]);
  const latest = visibleLogs[0];
  const latestSummary = latest ? formatActivityEvent(latest) : null;

  return (
    <section className={`activity-panel ${open ? "is-open" : "is-collapsed"}`}>
      <button className="activity-toggle" onClick={onToggle} type="button" title={open ? "Hide activity" : "Show activity"}>
        <span>
          <strong>Activity</strong>
          <em>{latestSummary ? latestSummary.body : "No recent activity"}</em>
        </span>
        <small>{open ? "Hide" : "Show"}</small>
      </button>
      {open ? (
        <div className="activity-list">
          {visibleLogs.map((event) => {
            const summary = formatActivityEvent(event);
            return (
              <article className={`activity-item activity-item--${event.level}`} key={event.id}>
                <div className="activity-item__title">
                  <span className="activity-item__icon">{summary.icon}</span>
                  <strong>{summary.title}</strong>
                </div>
                <span>{summary.body}</span>
              </article>
            );
          })}
        </div>
      ) : null}
    </section>
  );
}

function formatActivityEvent(event: LogEvent) {
  const fields = event.fields ?? {};
  if (event.scope === "agent" && event.action === "run") {
    const provider = fields.provider ? capitalize(fields.provider) : "Agent";
    const tool = fields.tool ? fields.tool.replaceAll("_", " ") : "action";
    return {
      icon: <Bot size={14} />,
      title: `${provider} command`,
      body: fields.reason || `Applied ${tool}.`,
    };
  }
  if (event.scope === "sessions" && event.action === "start") {
    return {
      icon: <SquareTerminal size={14} />,
      title: `Started ${capitalize(fields.agent ?? "session")} session`,
      body: fields.sessionName || event.message,
    };
  }
  if (event.scope === "worktrees" && event.action === "create") {
    return {
      icon: <GitBranch size={14} />,
      title: event.level === "error" ? "Worktree creation failed" : `Created ${fields.branch ?? "worktree"}`,
      body: event.level === "error" ? event.message : `Based on ${fields.base ?? "base branch"}.`,
    };
  }
  if (event.scope === "repos" && event.action === "add") {
    return {
      icon: <FolderGit2 size={14} />,
      title: "Project added",
      body: fields.path || event.message,
    };
  }
  if (event.scope === "repos" && event.action === "refresh") {
    return {
      icon: <RefreshCw size={14} />,
      title: "Project refreshed",
      body: fields.repoId ? `Fetched latest refs for ${fields.repoId}.` : event.message,
    };
  }
  if (event.scope === "sessions" && event.action === "stop") {
    return {
      icon: <SquareTerminal size={14} />,
      title: "Session stopped",
      body: fields.sessionName || event.message,
    };
  }
  if (event.scope === "sessions" && event.action === "resume") {
    return {
      icon: <SquareTerminal size={14} />,
      title: "Session resumed",
      body: fields.sessionName || event.message,
    };
  }
  if (event.scope === "terminal" && event.action === "attach") {
    return {
      icon: <SquareTerminal size={14} />,
      title: "Terminal connected",
      body: fields.sessionName || event.message,
    };
  }
  return {
    icon: <ChevronRight size={14} />,
    title: `${capitalize(event.scope)} · ${event.action.replaceAll("_", " ")}`,
    body: event.message,
  };
}
