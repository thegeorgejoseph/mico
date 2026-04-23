import { AlertCircle, Bot, CheckCircle2, LoaderCircle, SquareTerminal } from "lucide-react";

import type { Session } from "../types";

export type StatusTone = "working" | "ok" | "error";

export interface StatusNotice {
  message: string;
  tone: StatusTone;
}

interface StatusChipProps {
  notice: StatusNotice | null;
}

export function StatusChip({ notice }: StatusChipProps) {
  if (!notice) {
    return null;
  }
  return (
    <div className={`status-chip status-chip--${notice.tone}`}>
      <span className="status-chip__icon">{statusNoticeIcon(notice.tone)}</span>
      <span>{notice.message}</span>
    </div>
  );
}

interface AmbientChipProps {
  notice: StatusNotice | null;
  session: Session | null;
}

export function AmbientChip({ notice, session }: AmbientChipProps) {
  if (notice) {
    return <StatusChip notice={notice} />;
  }
  if (!session) {
    return null;
  }

  const statusLabel = session.status === "running" ? "running" : session.status;
  const icon = session.agent === "terminal" ? <SquareTerminal size={13} /> : <Bot size={13} />;

  return (
    <div className={`status-chip status-chip--ambient status-chip--session status-chip--${session.status}`}>
      <span className="status-chip__icon">{icon}</span>
      <span>{`${statusLabel} ${session.agent}`}</span>
    </div>
  );
}

function statusNoticeIcon(tone: StatusTone) {
  switch (tone) {
    case "working":
      return <LoaderCircle size={13} />;
    case "ok":
      return <CheckCircle2 size={13} />;
    case "error":
      return <AlertCircle size={13} />;
  }
}
