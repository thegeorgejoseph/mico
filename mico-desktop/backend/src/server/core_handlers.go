package server

import (
	"net/http"
	"strconv"
)

func (a *App) registerCoreRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/health", a.health)
	mux.HandleFunc("GET /api/state", a.state)
	mux.HandleFunc("GET /api/doctor", a.doctorReport)
	mux.HandleFunc("GET /api/logs", a.listLogs)
}

func (a *App) health(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (a *App) state(w http.ResponseWriter, _ *http.Request) {
	current, err := a.store.Load()
	writeResult(w, current, err)
}

func (a *App) doctorReport(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, a.doctor.Report())
}

func (a *App) listLogs(w http.ResponseWriter, r *http.Request) {
	limit := 100
	if raw := r.URL.Query().Get("limit"); raw != "" {
		parsed, err := strconv.Atoi(raw)
		if err != nil {
			writeErrorStatus(w, http.StatusBadRequest, err)
			return
		}
		limit = parsed
	}
	events, err := a.logs.List(limit)
	writeResult(w, events, err)
}
