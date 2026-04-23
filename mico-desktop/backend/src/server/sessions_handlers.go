package server

import (
	"net/http"
	"strconv"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/sessions"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func (a *App) registerSessionRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/sessions", a.listSessions)
	mux.HandleFunc("POST /api/sessions", a.startSession)
	mux.HandleFunc("POST /api/sessions/{id}/stop", a.stopSession)
	mux.HandleFunc("POST /api/sessions/{id}/resume", a.resumeSession)
	mux.HandleFunc("GET /api/sessions/{id}/terminal", a.captureTerminal)
	mux.HandleFunc("GET /api/sessions/{id}/terminal/ws", a.attachTerminalWebsocket)
	mux.HandleFunc("POST /api/sessions/{id}/terminal/input", a.sendTerminalInput)
}

func (a *App) listSessions(w http.ResponseWriter, _ *http.Request) {
	items, err := a.sessions.List()
	writeResult(w, items, err)
}

func (a *App) startSession(w http.ResponseWriter, r *http.Request) {
	var request sessions.StartRequest
	if !decodeJSON(w, r, &request) {
		return
	}
	session, err := a.sessions.Start(r.Context(), request)
	if err != nil {
		a.logs.Record(domain.LogError, "sessions", "start", err.Error(), map[string]string{"worktreeId": request.WorktreeID, "agent": string(request.Agent)})
	} else {
		a.logs.Record(domain.LogInfo, "sessions", "start", "Session started.", map[string]string{"sessionId": session.ID, "sessionName": session.SessionName, "agent": string(session.Agent)})
	}
	writeResultStatus(w, http.StatusCreated, session, err)
}

func (a *App) stopSession(w http.ResponseWriter, r *http.Request) {
	session, err := a.sessions.Stop(r.PathValue("id"))
	if err != nil {
		a.logs.Record(domain.LogError, "sessions", "stop", err.Error(), map[string]string{"sessionId": r.PathValue("id")})
		writeError(w, err)
		return
	}
	a.logs.Record(domain.LogInfo, "sessions", "stop", "Session stopped.", map[string]string{"sessionId": session.ID, "sessionName": session.SessionName})
	writeJSON(w, http.StatusOK, session)
}

func (a *App) resumeSession(w http.ResponseWriter, r *http.Request) {
	session, err := a.sessions.Resume(r.Context(), r.PathValue("id"))
	if err != nil {
		a.logs.Record(domain.LogError, "sessions", "resume", err.Error(), map[string]string{"sessionId": r.PathValue("id")})
		writeError(w, err)
		return
	}
	a.logs.Record(domain.LogInfo, "sessions", "resume", "Session resumed.", map[string]string{"sessionId": session.ID, "sessionName": session.SessionName})
	writeJSON(w, http.StatusOK, session)
}

func (a *App) captureTerminal(w http.ResponseWriter, r *http.Request) {
	lines := 160
	if raw := r.URL.Query().Get("lines"); raw != "" {
		parsed, err := strconv.Atoi(raw)
		if err != nil {
			writeErrorStatus(w, http.StatusBadRequest, err)
			return
		}
		lines = parsed
	}
	output, err := a.sessions.Capture(r.PathValue("id"), lines)
	if err != nil {
		a.logs.Record(domain.LogError, "terminal", "capture", err.Error(), map[string]string{"sessionId": r.PathValue("id")})
	}
	writeResult(w, map[string][]string{"lines": output}, err)
}

func (a *App) sendTerminalInput(w http.ResponseWriter, r *http.Request) {
	var request struct {
		Text string `json:"text"`
	}
	if !decodeJSON(w, r, &request) {
		return
	}
	err := a.sessions.Send(r.PathValue("id"), request.Text)
	if err != nil {
		a.logs.Record(domain.LogError, "terminal", "send", err.Error(), map[string]string{"sessionId": r.PathValue("id")})
		writeError(w, err)
		return
	}
	a.logs.Record(domain.LogInfo, "terminal", "send", "Terminal input sent.", map[string]string{"sessionId": r.PathValue("id")})
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}
