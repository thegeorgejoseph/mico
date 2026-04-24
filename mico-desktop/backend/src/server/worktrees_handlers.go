package server

import (
	"net/http"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/worktrees"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func (a *App) registerWorktreeRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/worktrees", a.listWorktrees)
	mux.HandleFunc("POST /api/worktrees", a.createWorktree)
}

func (a *App) listWorktrees(w http.ResponseWriter, _ *http.Request) {
	items, err := a.worktrees.List()
	writeResult(w, items, err)
}

func (a *App) createWorktree(w http.ResponseWriter, r *http.Request) {
	var request worktrees.CreateRequest
	if !decodeJSON(w, r, &request) {
		return
	}
	worktree, err := a.worktrees.Create(r.Context(), request)
	if err != nil {
		a.logs.Record(domain.LogError, "worktrees", "create", err.Error(), map[string]string{"repoId": request.RepoID, "branch": request.Branch, "base": request.Base})
	} else {
		a.logs.Record(domain.LogInfo, "worktrees", "create", "Worktree created.", map[string]string{"worktreeId": worktree.ID, "branch": worktree.Branch})
	}
	writeResultStatus(w, http.StatusCreated, worktree, err)
}
