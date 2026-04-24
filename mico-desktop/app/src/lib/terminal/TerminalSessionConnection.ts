import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";

import type { Session } from "../../types";

const TERMINAL_IDLE_STATUS = "Start or select a session.";

export class TerminalSessionConnection {
  private decoder = new TextDecoder();
  private disposed = false;
  private fitAddon: FitAddon | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private socket: WebSocket | null = null;
  private terminal: Terminal | null = null;

  constructor(
    private readonly host: HTMLDivElement,
    private readonly setStatus: (status: string) => void,
  ) {}

  connect(session: Session | null) {
    this.dispose();
    if (!session) {
      this.setStatus(TERMINAL_IDLE_STATUS);
      return;
    }

    this.disposed = false;
    this.setStatus("Connecting terminal...");
    this.terminal = new Terminal({
      allowProposedApi: false,
      convertEol: true,
      cursorBlink: true,
      disableStdin: false,
      fontFamily: '"SFMono-Regular", ui-monospace, Menlo, Consolas, monospace',
      fontSize: 12,
      lineHeight: 1.35,
      scrollback: 8000,
      theme: resolveTerminalTheme(this.host),
    });
    this.fitAddon = new FitAddon();
    this.socket = new WebSocket(window.mico.terminalSocketURL(session.id));
    this.socket.binaryType = "arraybuffer";

    this.terminal.loadAddon(this.fitAddon);
    this.terminal.open(this.host);
    this.terminal.focus();

    const inputDisposable = this.terminal.onData((data) => {
      if (this.socket?.readyState === WebSocket.OPEN) {
        this.socket.send(JSON.stringify({ type: "input", data }));
      }
    });

    const fitAndReport = () => {
      this.fitAddon?.fit();
      if (this.socket?.readyState === WebSocket.OPEN && this.terminal) {
        this.socket.send(JSON.stringify({ type: "resize", cols: this.terminal.cols, rows: this.terminal.rows }));
      }
      this.terminal?.scrollToBottom();
    };

    this.socket.addEventListener("open", () => {
      this.setStatus("");
      fitAndReport();
    });
    this.socket.addEventListener("message", async (event) => {
      if (this.disposed || !this.terminal) {
        return;
      }
      if (typeof event.data === "string") {
        this.terminal.write(event.data);
      } else if (event.data instanceof ArrayBuffer) {
        this.terminal.write(this.decoder.decode(event.data, { stream: true }));
      } else if (event.data instanceof Blob) {
        this.terminal.write(await event.data.text());
      }
      this.terminal.scrollToBottom();
    });
    this.socket.addEventListener("close", () => {
      if (!this.disposed) {
        this.setStatus("Terminal disconnected.");
      }
    });
    this.socket.addEventListener("error", () => {
      if (!this.disposed) {
        this.setStatus("Terminal connection failed.");
      }
    });

    this.resizeObserver = new ResizeObserver(fitAndReport);
    this.resizeObserver.observe(this.host);
    window.setTimeout(fitAndReport, 0);

    this.cleanup = () => {
      inputDisposable.dispose();
    };
  }

  private cleanup: (() => void) | null = null;

  dispose() {
    this.disposed = true;
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    this.cleanup?.();
    this.cleanup = null;
    this.terminal?.dispose();
    this.terminal = null;
    if (this.socket && (this.socket.readyState === WebSocket.OPEN || this.socket.readyState === WebSocket.CONNECTING)) {
      this.socket.close();
    }
    this.socket = null;
    this.fitAddon = null;
  }
}

function resolveTerminalTheme(host: HTMLElement) {
  const styles = window.getComputedStyle(host);
  return {
    background: styles.getPropertyValue("--terminal-theme-background").trim(),
    black: styles.getPropertyValue("--terminal-theme-black").trim(),
    blue: styles.getPropertyValue("--terminal-theme-blue").trim(),
    brightBlack: styles.getPropertyValue("--terminal-theme-bright-black").trim(),
    brightBlue: styles.getPropertyValue("--terminal-theme-bright-blue").trim(),
    brightCyan: styles.getPropertyValue("--terminal-theme-bright-cyan").trim(),
    brightGreen: styles.getPropertyValue("--terminal-theme-bright-green").trim(),
    brightMagenta: styles.getPropertyValue("--terminal-theme-bright-magenta").trim(),
    brightRed: styles.getPropertyValue("--terminal-theme-bright-red").trim(),
    brightWhite: styles.getPropertyValue("--terminal-theme-bright-white").trim(),
    brightYellow: styles.getPropertyValue("--terminal-theme-bright-yellow").trim(),
    cursor: styles.getPropertyValue("--terminal-theme-cursor").trim(),
    cyan: styles.getPropertyValue("--terminal-theme-cyan").trim(),
    foreground: styles.getPropertyValue("--terminal-theme-foreground").trim(),
    green: styles.getPropertyValue("--terminal-theme-green").trim(),
    magenta: styles.getPropertyValue("--terminal-theme-magenta").trim(),
    red: styles.getPropertyValue("--terminal-theme-red").trim(),
    selectionBackground: styles.getPropertyValue("--terminal-theme-selection-background").trim(),
    white: styles.getPropertyValue("--terminal-theme-white").trim(),
    yellow: styles.getPropertyValue("--terminal-theme-yellow").trim(),
  };
}
