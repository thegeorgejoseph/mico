package worktrees

import (
	"context"
	"errors"
	"path/filepath"
	"strings"
	"sync"
	"testing"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/repos"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
)

type fakeRunner struct {
	mu    sync.Mutex
	calls []string
}

func (f *fakeRunner) Run(_ context.Context, dir string, name string, args ...string) (repos.CommandResult, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.calls = append(f.calls, dir+"|"+name+"|"+strings.Join(args, " "))
	return repos.CommandResult{}, nil
}

func TestCreateWorktreeRunsGitWorktreeAdd(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	runner := &fakeRunner{}
	notifier := notifications.NewService(store)
	repoService := repos.NewService(store, runner, notifier)
	repo, err := repoService.Add(context.Background(), repos.AddRepoRequest{Path: t.TempDir(), Name: "mico"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	root := filepath.Join(t.TempDir(), "worktrees")
	service := NewService(store, repoService, runner, notifier, root)

	worktree, err := service.Create(context.Background(), CreateRequest{
		RepoID: repo.ID,
		Branch: "feature/desktop",
		Base:   "main",
	})
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}

	if worktree.Status != "ready" {
		t.Fatalf("Status = %q, want ready", worktree.Status)
	}
	if want := filepath.Join(root, "mico", "feature-desktop"); worktree.Path != want {
		t.Fatalf("Path = %q, want %q", worktree.Path, want)
	}
	lastCall := runner.calls[len(runner.calls)-1]
	if !strings.Contains(lastCall, "worktree add -b feature/desktop") {
		t.Fatalf("git worktree call missing expected args: %s", lastCall)
	}
}

func TestCreateWorktreeFromExistingBranchUsesExistingBranchFlow(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	runner := &fakeRunner{}
	notifier := notifications.NewService(store)
	repoService := repos.NewService(store, runner, notifier)
	repo, err := repoService.Add(context.Background(), repos.AddRepoRequest{Path: t.TempDir(), Name: "mico"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	root := filepath.Join(t.TempDir(), "worktrees")
	service := NewService(store, repoService, runner, notifier, root)

	_, err = service.Create(context.Background(), CreateRequest{
		RepoID:   repo.ID,
		Branch:   "feature/existing",
		Existing: true,
	})
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}

	lastCall := runner.calls[len(runner.calls)-1]
	if !strings.Contains(lastCall, "worktree add") || strings.Contains(lastCall, " -b feature/existing ") {
		t.Fatalf("expected existing branch worktree add, got %s", lastCall)
	}
}

type blockingRunner struct {
	mu          sync.Mutex
	calls       []string
	worktreeAdd chan struct{}
	release     chan struct{}
}

func (f *blockingRunner) Run(_ context.Context, dir string, name string, args ...string) (repos.CommandResult, error) {
	call := dir + "|" + name + "|" + strings.Join(args, " ")
	f.mu.Lock()
	f.calls = append(f.calls, call)
	f.mu.Unlock()
	if strings.Contains(call, "|git|worktree add") {
		f.worktreeAdd <- struct{}{}
		<-f.release
	}
	return repos.CommandResult{}, nil
}

func TestCreateWorktreeSerializesDuplicateRequests(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	runner := &blockingRunner{
		worktreeAdd: make(chan struct{}, 1),
		release:     make(chan struct{}),
	}
	notifier := notifications.NewService(store)
	repoService := repos.NewService(store, runner, notifier)
	repo, err := repoService.Add(context.Background(), repos.AddRepoRequest{Path: t.TempDir(), Name: "mico"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	service := NewService(store, repoService, runner, notifier, filepath.Join(t.TempDir(), "worktrees"))

	errs := make(chan error, 2)
	go func() {
		_, createErr := service.Create(context.Background(), CreateRequest{RepoID: repo.ID, Branch: "feature/desktop", Base: "main"})
		errs <- createErr
	}()
	<-runner.worktreeAdd
	go func() {
		_, createErr := service.Create(context.Background(), CreateRequest{RepoID: repo.ID, Branch: "feature/desktop", Base: "main"})
		errs <- createErr
	}()

	close(runner.release)

	firstErr := <-errs
	secondErr := <-errs
	if firstErr != nil && secondErr != nil {
		t.Fatalf("expected one create to succeed, got errs %v / %v", firstErr, secondErr)
	}
	var duplicateErr error
	if firstErr != nil {
		duplicateErr = firstErr
	} else {
		duplicateErr = secondErr
	}
	if duplicateErr == nil || !errors.Is(duplicateErr, errors.New("worktree is already tracked")) && !strings.Contains(duplicateErr.Error(), "worktree is already tracked") {
		t.Fatalf("duplicate error = %v, want already tracked", duplicateErr)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if len(loaded.Worktrees) != 1 {
		t.Fatalf("worktrees = %+v", loaded.Worktrees)
	}
	runner.mu.Lock()
	defer runner.mu.Unlock()
	count := 0
	for _, call := range runner.calls {
		if strings.Contains(call, "|git|worktree add") {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("git worktree add calls = %d, want 1", count)
	}
}
