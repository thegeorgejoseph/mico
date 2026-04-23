package state

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func TestStoreCreatesDefaultState(t *testing.T) {
	store := NewStore(filepath.Join(t.TempDir(), "state.json"))

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if loaded.Version != 1 {
		t.Fatalf("Version = %d, want 1", loaded.Version)
	}
	if len(loaded.Repos) != 0 || len(loaded.Worktrees) != 0 || len(loaded.Sessions) != 0 {
		t.Fatalf("default state should be empty: %+v", loaded)
	}
}

func TestStoreUpdatePersistsMutation(t *testing.T) {
	store := NewStore(filepath.Join(t.TempDir(), "state.json"))

	_, err := store.Update(func(next *domain.State) error {
		next.Repos = append(next.Repos, domain.Repo{ID: "repo_1", Name: "mico", Path: "/tmp/mico"})
		return nil
	})
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if len(loaded.Repos) != 1 || loaded.Repos[0].Name != "mico" {
		t.Fatalf("persisted repos = %+v", loaded.Repos)
	}
}

func TestStoreLoadReturnsDetachedCopy(t *testing.T) {
	store := NewStore(filepath.Join(t.TempDir(), "state.json"))
	now := time.Unix(10, 0).UTC()
	if err := store.Save(domain.State{
		Version: 1,
		Sessions: []domain.Session{{
			ID:          "ses_1",
			WorktreeID:  "wt_1",
			Agent:       domain.AgentCodex,
			Command:     []string{"codex"},
			SessionName: "mico-test",
			Status:      domain.SessionRunning,
			CreatedAt:   now,
			UpdatedAt:   now,
		}},
		Logs: []domain.LogEvent{{
			ID:        "log_1",
			Level:     domain.LogInfo,
			Scope:     "test",
			Action:    "load",
			Message:   "hello",
			Fields:    map[string]string{"one": "1"},
			CreatedAt: now,
		}},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	loaded.Sessions[0].Command[0] = "claude"
	loaded.Logs[0].Fields["one"] = "2"

	again, err := store.Load()
	if err != nil {
		t.Fatalf("second Load() error = %v", err)
	}
	if got := again.Sessions[0].Command[0]; got != "codex" {
		t.Fatalf("session command = %q, want codex", got)
	}
	if got := again.Logs[0].Fields["one"]; got != "1" {
		t.Fatalf("log field = %q, want 1", got)
	}
}

func TestStoreLoadDropsLegacySeenNotifications(t *testing.T) {
	path := filepath.Join(t.TempDir(), "state.json")
	now := time.Unix(20, 0).UTC()
	raw := domain.State{
		Version: 1,
		Notifications: []domain.Notification{
			{ID: "ntf_seen", Title: "Old", Seen: true, CreatedAt: now},
			{ID: "ntf_live", Title: "Live", CreatedAt: now},
		},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}
	data, err := json.Marshal(raw)
	if err != nil {
		t.Fatalf("Marshal() error = %v", err)
	}
	if err := os.WriteFile(path, append(data, '\n'), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	store := NewStore(path)
	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if len(loaded.Notifications) != 1 || loaded.Notifications[0].ID != "ntf_live" {
		t.Fatalf("notifications = %+v", loaded.Notifications)
	}

	persistedData, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}
	if string(persistedData) == "" || string(persistedData) == string(append(data, '\n')) {
		t.Fatalf("state file was not rewritten after dropping seen notifications: %s", string(persistedData))
	}
	if bytes.Contains(persistedData, []byte("ntf_seen")) {
		t.Fatalf("state file still contains dismissed notification: %s", string(persistedData))
	}
}

func TestStoreLoadNormalizesNullCollectionsFromDisk(t *testing.T) {
	path := filepath.Join(t.TempDir(), "state.json")
	raw := []byte("{\n  \"version\": 1,\n  \"repos\": null,\n  \"worktrees\": null,\n  \"sessions\": null,\n  \"notifications\": null,\n  \"selection\": {\n    \"repoId\": \"\",\n    \"worktreeId\": \"\",\n    \"sessionId\": \"\",\n    \"mode\": \"\"\n  },\n  \"logs\": null,\n  \"migrations\": null\n}\n")
	if err := os.WriteFile(path, raw, 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	store := NewStore(path)
	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if loaded.Repos == nil || loaded.Worktrees == nil || loaded.Sessions == nil || loaded.Notifications == nil || loaded.Logs == nil || loaded.Migrations == nil {
		t.Fatalf("collections should be normalized: %+v", loaded)
	}
	if loaded.Selection.Mode != domain.UIModeEffort {
		t.Fatalf("selection mode = %q, want %q", loaded.Selection.Mode, domain.UIModeEffort)
	}

	persistedData, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}
	for _, snippet := range [][]byte{
		[]byte(`"repos": []`),
		[]byte(`"worktrees": []`),
		[]byte(`"sessions": []`),
		[]byte(`"notifications": []`),
		[]byte(`"logs": []`),
	} {
		if !bytes.Contains(persistedData, snippet) {
			t.Fatalf("persisted state missing %s: %s", string(snippet), string(persistedData))
		}
	}
	if bytes.Contains(persistedData, []byte(`"migrations": null`)) {
		t.Fatalf("persisted state should not encode null migrations: %s", string(persistedData))
	}
}
