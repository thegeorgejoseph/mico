import { useEffect, useRef, useState } from "react";
import "@xterm/xterm/css/xterm.css";

import { THEMES, type Session, type ThemeName } from "../types";
import { TerminalSessionConnection } from "../lib/terminal/TerminalSessionConnection";

interface TerminalViewProps {
  session: Session | null;
  theme: ThemeName;
}

export function TerminalView({ session, theme }: TerminalViewProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const [status, setStatus] = useState(session ? "Connecting terminal..." : "Start or select a session.");

  useEffect(() => {
    if (!session || !hostRef.current) {
      setStatus("Start or select a session.");
      return undefined;
    }
    const connection = new TerminalSessionConnection(hostRef.current, setStatus);
    connection.connect(session);
    return () => connection.dispose();
  }, [session, theme]);

  return (
    <div className={`terminal-viewport theme-${theme === THEMES.LIGHT ? THEMES.LIGHT : THEMES.DARK}`}>
      <div className="terminal-xterm">
        <div className="terminal-xterm__host" ref={hostRef} />
      </div>
      {status ? <div className="terminal-status">{status}</div> : null}
    </div>
  );
}
