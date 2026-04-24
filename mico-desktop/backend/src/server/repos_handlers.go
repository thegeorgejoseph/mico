package server

import (
	"net/http"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/repos"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func (a *App) registerRepoRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/repos", a.listRepos)
	mux.HandleFunc("POST /api/repos", a.addRepo)
	mux.HandleFunc("GET /api/repos/{id}/branches", a.branches)
	mux.HandleFunc("POST /api/repos/{id}/refresh", a.refreshRepo)
}

func (a *App) listRepos(w http.ResponseWriter, _ *http.Request) {
	repositories, err := a.repos.List()
	writeResult(w, repositories, err)
}

func (a *App) addRepo(w http.ResponseWriter, r *http.Request) {
	var request repos.AddRepoRequest
	if !decodeJSON(w, r, &request) {
		return
	}
	repo, err := a.repos.Add(r.Context(), request)
	if err != nil {
		a.logs.Record(domain.LogError, "repos", "add", err.Error(), map[string]string{"path": request.Path})
	} else {
		a.logs.Record(domain.LogInfo, "repos", "add", "Repository added.", map[string]string{"repoId": repo.ID, "path": repo.Path})
	}
	writeResultStatus(w, http.StatusCreated, repo, err)
}

func (a *App) branches(w http.ResponseWriter, r *http.Request) {
	branches, err := a.repos.Branches(r.Context(), r.PathValue("id"))
	writeResult(w, branches, err)
}

func (a *App) refreshRepo(w http.ResponseWriter, r *http.Request) {
	err := a.repos.Refresh(r.Context(), r.PathValue("id"))
	if err != nil {
		a.logs.Record(domain.LogError, "repos", "refresh", err.Error(), map[string]string{"repoId": r.PathValue("id")})
		writeError(w, err)
		return
	}
	a.logs.Record(domain.LogInfo, "repos", "refresh", "Repository refreshed.", map[string]string{"repoId": r.PathValue("id")})
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}
