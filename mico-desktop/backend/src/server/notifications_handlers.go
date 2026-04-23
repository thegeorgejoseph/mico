package server

import (
	"net/http"
)

func (a *App) registerNotificationRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/notifications", a.listNotifications)
	mux.HandleFunc("DELETE /api/notifications/{id}", a.dismissNotification)
	mux.HandleFunc("POST /api/notifications/{id}/seen", a.dismissNotification)
}

func (a *App) listNotifications(w http.ResponseWriter, _ *http.Request) {
	items, err := a.notifications.List()
	writeResult(w, items, err)
}

func (a *App) dismissNotification(w http.ResponseWriter, r *http.Request) {
	err := a.notifications.Dismiss(r.PathValue("id"))
	if err != nil {
		writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}
