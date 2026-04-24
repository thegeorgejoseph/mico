package migrations

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
)

func TestLegacyStateMigrationImportsRustCliStateOnce(t *testing.T) {
	dir := t.TempDir()
	legacyPath := filepath.Join(dir, "state.json")
	payload := `{
  "version": 3,
  "repos": [{"id":"repo-id","path":"/tmp/repo","display_name":"Repo"}],
  "workstreams": [{
    "id":"workstream-id",
    "repo_id":"repo-id",
    "base_branch":"main",
    "branch":"feature/x",
    "worktree_path":"/tmp/worktrees/repo/feature-x",
    "session_name":"mico-repo-feature-x",
    "agent_preset":"codex",
    "status":"Running",
    "created_at_epoch_secs":10,
    "status_changed_at_epoch_secs":11,
    "sessions":[{"id":"session-id","session_name":"mico-repo-feature-x","agent_preset":"codex","status":"Running","created_at_epoch_secs":10,"status_changed_at_epoch_secs":11}]
  }]
}`
	if err := os.WriteFile(legacyPath, []byte(payload), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	store := state.NewStore(filepath.Join(dir, "desktop.json"))
	runner := NewRunner(store, NewLegacyStateMigration(legacyPath))
	if err := runner.Apply(); err != nil {
		t.Fatalf("Apply() error = %v", err)
	}
	if err := runner.Apply(); err != nil {
		t.Fatalf("Apply() second pass error = %v", err)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if len(loaded.Repos) != 1 || loaded.Repos[0].ID != "repo-id" {
		t.Fatalf("repos = %+v", loaded.Repos)
	}
	if len(loaded.Worktrees) != 1 || loaded.Worktrees[0].Branch != "feature/x" {
		t.Fatalf("worktrees = %+v", loaded.Worktrees)
	}
	if len(loaded.Sessions) != 1 || loaded.Sessions[0].SessionName != "mico-repo-feature-x" {
		t.Fatalf("sessions = %+v", loaded.Sessions)
	}
	if len(loaded.Migrations) != 1 || loaded.Migrations[0].ID != legacyStateMigrationID {
		t.Fatalf("migrations = %+v", loaded.Migrations)
	}
}
