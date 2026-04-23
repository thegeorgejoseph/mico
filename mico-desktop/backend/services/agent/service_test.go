package agent

import (
	"context"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type fakeRunner struct {
	output string
	err    error
}

func (f fakeRunner) Run(context.Context, string) (string, error) {
	return f.output, f.err
}

func TestRunExecutesSelectWorktreeTool(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{{
			ID:        "repo-id",
			Name:      "Raven",
			Path:      "/tmp/repo",
			CreatedAt: time.Unix(1, 0),
		}},
		Worktrees: []domain.Worktree{{
			ID:        "worktree-id",
			RepoID:    "repo-id",
			Branch:    "testing-x",
			Base:      "main",
			Path:      "/tmp/worktree",
			Status:    domain.WorktreeReady,
			CreatedAt: time.Unix(1, 0),
			UpdatedAt: time.Unix(1, 0),
		}},
		Sessions: []domain.Session{
			{
				ID:          "legacy-session",
				WorktreeID:  "worktree-id",
				Agent:       domain.AgentTerminal,
				Command:     []string{"/bin/zsh"},
				SessionName: "legacy-session",
				Status:      domain.SessionRunning,
				CreatedAt:   time.Unix(1, 0),
				UpdatedAt:   time.Unix(1, 0),
			},
			{
				ID:          "desktop-session",
				WorktreeID:  "worktree-id",
				Agent:       domain.AgentCodex,
				Command:     []string{"codex"},
				SessionName: "mico-desktop-worktree-id-codex",
				Status:      domain.SessionRunning,
				CreatedAt:   time.Unix(2, 0),
				UpdatedAt:   time.Unix(2, 0),
			},
		},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}
	service := newTestService(store, fakeRunner{
		output: `{"tool":"select_worktree","reason":"testing-x matches","selectWorktree":{"worktreeId":"worktree-id"}}`,
	})

	response, err := service.Run(context.Background(), RunRequest{Provider: domain.AgentCodex, Message: "switch to testing"})
	if err != nil {
		t.Fatalf("Run() error = %v", err)
	}
	if response.State == nil || response.State.WorktreeID != "worktree-id" {
		t.Fatalf("selection = %+v", response.State)
	}
	if response.State.RepoID != "repo-id" {
		t.Fatalf("repo selection = %+v", response.State)
	}
	if response.State.SessionID != "desktop-session" {
		t.Fatalf("session selection = %+v", response.State)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if loaded.Selection.WorktreeID != "worktree-id" || loaded.Selection.Mode != domain.UIModeAgent {
		t.Fatalf("persisted selection = %+v", loaded.Selection)
	}
}

func TestParseToolCallExtractsTaggedAction(t *testing.T) {
	call, err := parseToolCall("prefix noise\n<MICO_ACTION>{\"tool\":\"list_worktrees\",\"reason\":\"need more context\",\"listWorktrees\":{}}</MICO_ACTION>\nsuffix noise")
	if err != nil {
		t.Fatalf("parseToolCall() error = %v", err)
	}
	if call.Tool != "list_worktrees" {
		t.Fatalf("tool = %q, want list_worktrees", call.Tool)
	}
}

func TestParseToolCallExtractsLastValidToolFromNoisyTranscript(t *testing.T) {
	raw := `OpenAI Codex v0.116.0
user
You are controlling mico desktop through typed actions.
Available tools:
- {"tool":"select_worktree","reason":"why this worktree matches","selectWorktree":{"worktreeId":"..."}}
Current mico state:
{
  "selection": {
    "repoId": "repo-a",
    "worktreeId": "wt-old",
    "sessionId": "",
    "mode": "agent"
  },
  "repos": [
    { "id": "repo-a", "name": "raven", "path": "/tmp/raven", "createdAt": "2026-01-01T00:00:00Z" }
  ],
  "worktrees": [
    { "id": "wt-old", "repoId": "repo-a", "branch": "main", "base": "main", "path": "/tmp/raven-main", "status": "running", "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z" },
    { "id": "wt-raven", "repoId": "repo-a", "branch": "testing-raven", "base": "main", "path": "/tmp/raven-testing", "status": "running", "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z" }
  ]
}
codex
{"tool":"select_worktree","reason":"The worktree with branch testing-raven is the direct match.","selectWorktree":{"worktreeId":"wt-raven"}}`

	call, err := parseToolCall(raw)
	if err != nil {
		t.Fatalf("parseToolCall() error = %v", err)
	}
	if call.Tool != "select_worktree" {
		t.Fatalf("tool = %q, want select_worktree", call.Tool)
	}
	if call.SelectWorktree == nil || call.SelectWorktree.WorktreeID != "wt-raven" {
		t.Fatalf("call = %+v", call)
	}
}

func TestRunExecutesSelectionFromNoisyTranscript(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{{
			ID:        "repo-id",
			Name:      "Raven",
			Path:      "/tmp/repo",
			CreatedAt: time.Unix(1, 0),
		}},
		Worktrees: []domain.Worktree{
			{
				ID:        "wt-old",
				RepoID:    "repo-id",
				Branch:    "main",
				Base:      "main",
				Path:      "/tmp/main",
				Status:    domain.WorktreeReady,
				CreatedAt: time.Unix(1, 0),
				UpdatedAt: time.Unix(1, 0),
			},
			{
				ID:        "wt-raven",
				RepoID:    "repo-id",
				Branch:    "testing-raven",
				Base:      "main",
				Path:      "/tmp/testing-raven",
				Status:    domain.WorktreeReady,
				CreatedAt: time.Unix(2, 0),
				UpdatedAt: time.Unix(2, 0),
			},
		},
		Sessions: []domain.Session{
			{
				ID:          "legacy-session",
				WorktreeID:  "wt-raven",
				Agent:       domain.AgentTerminal,
				Command:     []string{"/bin/zsh"},
				SessionName: "legacy-session",
				Status:      domain.SessionRunning,
				CreatedAt:   time.Unix(1, 0),
				UpdatedAt:   time.Unix(1, 0),
			},
			{
				ID:          "desktop-session",
				WorktreeID:  "wt-raven",
				Agent:       domain.AgentCodex,
				Command:     []string{"codex"},
				SessionName: "mico-desktop-wt-raven-codex",
				Status:      domain.SessionRunning,
				CreatedAt:   time.Unix(3, 0),
				UpdatedAt:   time.Unix(3, 0),
			},
		},
		Selection: domain.UISelection{
			RepoID:     "repo-id",
			WorktreeID: "wt-old",
			Mode:       domain.UIModeEffort,
		},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	service := newTestService(store, fakeRunner{
		output: `codex transcript noise
Current mico state:
{"selection":{"repoId":"repo-id","worktreeId":"wt-old","sessionId":"","mode":"agent"}}
{"tool":"select_worktree","reason":"testing-raven matches directly","selectWorktree":{"worktreeId":"wt-raven"}}`,
	})

	response, err := service.Run(context.Background(), RunRequest{Provider: domain.AgentCodex, Message: "switch to my testing-raven worktree"})
	if err != nil {
		t.Fatalf("Run() error = %v", err)
	}
	if response.State == nil || response.State.WorktreeID != "wt-raven" {
		t.Fatalf("selection = %+v", response.State)
	}
	if response.State.SessionID != "desktop-session" {
		t.Fatalf("session selection = %+v", response.State)
	}
}

func TestRunFailsWhenProviderDoesNotReturnValidAction(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := newTestService(store, fakeRunner{
		output: "I think the right thing to do is switch to testing-raven, but I did not emit an action.",
	})

	_, err := service.Run(context.Background(), RunRequest{Provider: domain.AgentCodex, Message: "switch to testing-raven"})
	if err == nil {
		t.Fatal("Run() error = nil, want invalid action error")
	}
	if !strings.Contains(err.Error(), "agent did not return a valid action") {
		t.Fatalf("error = %v", err)
	}
}

func newTestService(store *state.Store, provider fakeRunner) *Service {
	return NewService(store, NewResolverWithProviders(map[domain.AgentKind]Provider{
		domain.AgentCodex:  provider,
		domain.AgentClaude: provider,
	}))
}
