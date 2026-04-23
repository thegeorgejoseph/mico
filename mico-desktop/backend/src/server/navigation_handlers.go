package server

import (
	"net/http"
	"strconv"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/navigation"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func (a *App) registerNavigationRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/navigation/search", a.searchNavigation)
	mux.HandleFunc("PUT /api/navigation/focus", a.updateFocus)
	mux.HandleFunc("PUT /api/selection", a.updateFocus)
}

func (a *App) searchNavigation(w http.ResponseWriter, r *http.Request) {
	limit := 8
	if raw := r.URL.Query().Get("limit"); raw != "" {
		parsed, err := strconv.Atoi(raw)
		if err != nil {
			writeErrorStatus(w, http.StatusBadRequest, err)
			return
		}
		limit = parsed
	}
	results, err := a.navigation.Search(r.URL.Query().Get("q"), limit)
	writeResult(w, results, err)
}

func (a *App) updateFocus(w http.ResponseWriter, r *http.Request) {
	var request struct {
		RepoID     *string `json:"repoId"`
		WorktreeID *string `json:"worktreeId"`
		SessionID  *string `json:"sessionId"`
		Mode       *string `json:"mode"`
	}
	if !decodeJSON(w, r, &request) {
		return
	}
	var mode *domain.UIMode
	if request.Mode != nil {
		nextMode := domain.UIMode(*request.Mode)
		mode = &nextMode
	}
	current, err := a.navigation.Focus(navigation.FocusPatch{
		RepoID:     request.RepoID,
		WorktreeID: request.WorktreeID,
		SessionID:  request.SessionID,
		Mode:       mode,
	})
	if err == nil {
		a.logs.Record(domain.LogDebug, "navigation", "focus", "Workspace focus updated.", map[string]string{
			"repoId":     current.RepoID,
			"worktreeId": current.WorktreeID,
			"sessionId":  current.SessionID,
			"mode":       string(current.Mode),
		})
	}
	writeResult(w, current, err)
}
