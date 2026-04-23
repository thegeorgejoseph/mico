package navigation

import (
	"path/filepath"
	"testing"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func TestUpdateDerivesRepoAndSessionFromWorktree(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	now := time.Unix(10, 0).UTC()
	if err := store.Save(domain.State{
		Version:   1,
		Repos:     []domain.Repo{{ID: "repo-1", Name: "repo", Path: "/tmp/repo", CreatedAt: now}},
		Worktrees: []domain.Worktree{{ID: "wt-1", RepoID: "repo-1", Branch: "feature", Base: "main", Path: "/tmp/wt-1", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now}},
		Sessions: []domain.Session{
			{ID: "legacy", WorktreeID: "wt-1", Agent: domain.AgentTerminal, Command: []string{"/bin/zsh"}, SessionName: "legacy", Status: domain.SessionRunning, CreatedAt: now, UpdatedAt: now},
			{ID: "desktop", WorktreeID: "wt-1", Agent: domain.AgentCodex, Command: []string{"codex"}, SessionName: "mico-desktop-wt-1-codex", Status: domain.SessionRunning, CreatedAt: now.Add(time.Second), UpdatedAt: now.Add(time.Second)},
		},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	service := NewService(store)
	worktreeID := "wt-1"
	next, err := service.Update(Patch{WorktreeID: &worktreeID})
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	if next.RepoID != "repo-1" || next.WorktreeID != "wt-1" || next.SessionID != "desktop" {
		t.Fatalf("selection = %+v", next)
	}
}

func TestUpdateDerivesRepoAndWorktreeFromSession(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	now := time.Unix(10, 0).UTC()
	if err := store.Save(domain.State{
		Version:   1,
		Repos:     []domain.Repo{{ID: "repo-1", Name: "repo", Path: "/tmp/repo", CreatedAt: now}},
		Worktrees: []domain.Worktree{{ID: "wt-1", RepoID: "repo-1", Branch: "feature", Base: "main", Path: "/tmp/wt-1", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now}},
		Sessions: []domain.Session{
			{ID: "desktop", WorktreeID: "wt-1", Agent: domain.AgentCodex, Command: []string{"codex"}, SessionName: "mico-desktop-wt-1-codex", Status: domain.SessionRunning, CreatedAt: now, UpdatedAt: now},
		},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	service := NewService(store)
	sessionID := "desktop"
	next, err := service.Update(Patch{SessionID: &sessionID})
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	if next.RepoID != "repo-1" || next.WorktreeID != "wt-1" || next.SessionID != "desktop" {
		t.Fatalf("selection = %+v", next)
	}
}

func TestUpdateRejectsUnknownSession(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := NewService(store)
	sessionID := "missing"

	_, err := service.Update(Patch{SessionID: &sessionID})
	if err == nil || err.Error() != "session not found" {
		t.Fatalf("error = %v, want session not found", err)
	}
}

func TestUpdateReconcilesInvalidSessionForSelectedRepo(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	now := time.Unix(10, 0).UTC()
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{
			{ID: "repo-1", Name: "one", Path: "/tmp/repo-1", CreatedAt: now},
			{ID: "repo-2", Name: "two", Path: "/tmp/repo-2", CreatedAt: now},
		},
		Worktrees: []domain.Worktree{
			{ID: "wt-1", RepoID: "repo-1", Branch: "main", Base: "main", Path: "/tmp/wt-1", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now},
			{ID: "wt-2", RepoID: "repo-2", Branch: "feature", Base: "main", Path: "/tmp/wt-2", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now.Add(time.Second)},
		},
		Sessions: []domain.Session{
			{ID: "desktop", WorktreeID: "wt-2", Agent: domain.AgentCodex, Command: []string{"codex"}, SessionName: "mico-desktop-wt-2-codex", Status: domain.SessionRunning, CreatedAt: now, UpdatedAt: now},
		},
		Selection: domain.UISelection{RepoID: "repo-1", WorktreeID: "wt-1", SessionID: "desktop", Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	service := NewService(store)
	repoID := "repo-1"
	next, err := service.Update(Patch{RepoID: &repoID})
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	if next.RepoID != "repo-1" || next.WorktreeID != "wt-1" || next.SessionID != "" {
		t.Fatalf("selection = %+v", next)
	}
}

func TestSearchRanksExactWorktreeMatchesFirst(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	now := time.Unix(10, 0).UTC()
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{
			{ID: "repo-1", Name: "raven", Path: "/tmp/raven", CreatedAt: now},
			{ID: "repo-2", Name: "mico", Path: "/tmp/mico", CreatedAt: now},
		},
		Worktrees: []domain.Worktree{
			{ID: "wt-1", RepoID: "repo-1", Branch: "main", Base: "main", Path: "/tmp/raven-main", Status: domain.WorktreeReady, CreatedAt: now, UpdatedAt: now},
			{ID: "wt-2", RepoID: "repo-1", Branch: "testing-raven", Base: "main", Path: "/tmp/raven-testing", Status: domain.WorktreeReady, CreatedAt: now, UpdatedAt: now},
		},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	service := NewService(store)
	results, err := service.Search("testing-raven", 8)
	if err != nil {
		t.Fatalf("Search() error = %v", err)
	}
	if len(results) == 0 || results[0].Kind != SearchResultWorktree || results[0].ID != "wt-2" {
		t.Fatalf("results = %+v", results)
	}
}
