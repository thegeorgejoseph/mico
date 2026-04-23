import { Rocket, X } from "lucide-react";

import { Button } from "./Button";
import { TextField } from "./Field";
import { handleScopedNavigation } from "../lib/navigation";
import type { AgentKind } from "../types";

interface CommandPaletteProps {
  errorMessage: string;
  input: string;
  isRunning: boolean;
  onClose: () => void;
  onInputChange: (value: string) => void;
  onProviderChange: (value: AgentKind) => void;
  onRun: () => void;
  open: boolean;
  provider: AgentKind;
}

export function CommandPalette({ errorMessage, input, isRunning, onClose, onInputChange, onProviderChange, onRun, open, provider }: CommandPaletteProps) {
  if (!open) {
    return null;
  }

  return (
    <div className="command-palette-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="command-palette" role="dialog" aria-label="Command palette" onMouseDown={(event) => event.stopPropagation()}>
        <form
          className="command-palette__form"
          onSubmit={(event) => {
            event.preventDefault();
            onRun();
          }}
        >
          <div className="command-palette__header">
            <div className="command-palette__title">
              <Rocket size={16} />
              <h2>Mission Control</h2>
            </div>
            <button className="toolbar-icon" disabled={isRunning} onClick={onClose} type="button" aria-label="Close command palette" title="Close Mission command palette">
              <X size={16} />
            </button>
          </div>
          <div className="command-palette__providers" data-nav-scope="command-providers">
            <button className={provider === "codex" ? "is-active" : ""} data-nav-item="true" onClick={() => onProviderChange("codex")} onKeyDown={(event) => handleScopedNavigation(event, "horizontal")} type="button">
              Codex
            </button>
            <button className={provider === "claude" ? "is-active" : ""} data-nav-item="true" onClick={() => onProviderChange("claude")} onKeyDown={(event) => handleScopedNavigation(event, "horizontal")} type="button">
              Claude
            </button>
          </div>
          <label className="command-palette__field">
            <Rocket size={16} />
            <TextField
              autoFocus
              onChange={(event) => onInputChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  onRun();
                }
              }}
              placeholder="Switch to my testing-raven worktree"
              value={input}
            />
          </label>
          {errorMessage ? (
            <p className="command-palette__feedback command-palette__feedback--error" role="alert">
              Agent run failed: {errorMessage}
            </p>
          ) : null}
          <div className="command-palette__footer">
            <p>{isRunning ? `Running ${provider}...` : "Selections and agent actions show up in Activity automatically."}</p>
            <Button disabled={!input.trim() || isRunning} onClick={onRun} type="button" variant="primary">
              {isRunning ? "Running..." : "Run"}
            </Button>
          </div>
        </form>
      </section>
    </div>
  );
}
