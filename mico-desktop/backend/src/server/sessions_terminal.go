package server

import (
	"encoding/json"
	"io"
	"net/http"
	"os"
	"os/exec"
	"strings"

	"github.com/creack/pty"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
	"golang.org/x/net/websocket"
)

func (a *App) attachTerminalWebsocket(w http.ResponseWriter, r *http.Request) {
	session, err := a.sessions.Find(r.PathValue("id"))
	if err != nil {
		a.logs.Record(domain.LogError, "terminal", "websocket", err.Error(), map[string]string{"sessionId": r.PathValue("id")})
		writeError(w, err)
		return
	}

	handler := websocket.Handler(func(conn *websocket.Conn) {
		defer conn.Close()

		cmd := exec.CommandContext(r.Context(), "tmux", "attach", "-t", session.SessionName)
		// The embedded xterm client needs a real terminal type here.
		// GUI launches often inherit TERM=dumb, which makes tmux refuse to draw.
		cmd.Env = terminalClientEnv(os.Environ())
		tty, err := pty.StartWithSize(cmd, &pty.Winsize{Cols: 120, Rows: 34})
		if err != nil {
			a.logs.Record(domain.LogError, "terminal", "pty.start", err.Error(), map[string]string{"sessionId": session.ID, "sessionName": session.SessionName})
			return
		}
		defer tty.Close()
		if cmd.Process != nil {
			defer cmd.Process.Kill()
		}
		a.logs.Record(domain.LogInfo, "terminal", "attach", "Terminal websocket attached.", map[string]string{"sessionId": session.ID, "sessionName": session.SessionName})

		done := make(chan struct{})
		go func() {
			defer close(done)
			buffer := make([]byte, 4096)
			for {
				n, readErr := tty.Read(buffer)
				if n > 0 {
					if err := websocket.Message.Send(conn, buffer[:n]); err != nil {
						return
					}
				}
				if readErr != nil {
					return
				}
			}
		}()

		for {
			select {
			case <-done:
				return
			case <-r.Context().Done():
				return
			default:
			}
			var data []byte
			if err := websocket.Message.Receive(conn, &data); err != nil {
				return
			}
			if err := handleTerminalMessage(tty, data); err != nil {
				a.logs.Record(domain.LogWarn, "terminal", "input", err.Error(), map[string]string{"sessionId": session.ID})
				return
			}
		}
	})

	handler.ServeHTTP(w, r)
}

type terminalMessage struct {
	Type string `json:"type"`
	Data string `json:"data,omitempty"`
	Cols uint16 `json:"cols,omitempty"`
	Rows uint16 `json:"rows,omitempty"`
}

func handleTerminalMessage(tty *os.File, data []byte) error {
	var message terminalMessage
	if err := json.Unmarshal(data, &message); err != nil {
		_, writeErr := tty.Write(data)
		return writeErr
	}
	switch message.Type {
	case "input":
		_, err := io.WriteString(tty, message.Data)
		return err
	case "resize":
		if message.Cols == 0 || message.Rows == 0 {
			return nil
		}
		return pty.Setsize(tty, &pty.Winsize{Cols: message.Cols, Rows: message.Rows})
	default:
		return nil
	}
}

func terminalClientEnv(base []string) []string {
	filtered := make([]string, 0, len(base)+2)
	for _, entry := range base {
		if strings.HasPrefix(entry, "TERM=") || strings.HasPrefix(entry, "COLORTERM=") {
			continue
		}
		filtered = append(filtered, entry)
	}
	return append(filtered, "TERM=xterm-256color", "COLORTERM=truecolor")
}
