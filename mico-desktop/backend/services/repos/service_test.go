package repos

import (
	"context"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
)

type fakeRunner struct {
	results map[string]CommandResult
	calls   []string
}

func (f *fakeRunner) Run(_ context.Context, dir string, name string, args ...string) (CommandResult, error) {
	call := dir + "|" + name + "|" + joinArgs(args)
	f.calls = append(f.calls, call)
	if result, ok := f.results[joinArgs(args)]; ok {
		return result, nil
	}
	return CommandResult{}, nil
}

func joinArgs(args []string) string {
	out := ""
	for index, arg := range args {
		if index > 0 {
			out += " "
		}
		out += arg
	}
	return out
}

func TestAddRepoPersistsAbsolutePath(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := NewServiceWithClock(
		store,
		&fakeRunner{},
		notifications.NewService(store),
		func() time.Time { return time.Unix(20, 0).UTC() },
	)

	repoPath := filepath.Join(t.TempDir(), "mico")
	repo, err := service.Add(context.Background(), AddRepoRequest{Path: repoPath})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	if repo.Name != "mico" {
		t.Fatalf("repo.Name = %q, want mico", repo.Name)
	}
	if !filepath.IsAbs(repo.Path) {
		t.Fatalf("repo.Path = %q, want absolute path", repo.Path)
	}
}

func TestBranchesAreSorted(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	runner := &fakeRunner{results: map[string]CommandResult{
		"branch --all --format=%(refname:short)": {Stdout: "zeta\nmain\nalpha\n"},
	}}
	service := NewService(store, runner, notifications.NewService(store))

	repo, err := service.Add(context.Background(), AddRepoRequest{Path: t.TempDir(), Name: "repo"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	got, err := service.Branches(context.Background(), repo.ID)
	if err != nil {
		t.Fatalf("Branches() error = %v", err)
	}
	want := []string{"alpha", "main", "zeta"}
	if !slices.Equal(got, want) {
		t.Fatalf("Branches() = %v, want %v", got, want)
	}
}

func TestBranchesReturnsEmptySliceForRepoWithoutBranches(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	runner := &fakeRunner{results: map[string]CommandResult{
		"branch --all --format=%(refname:short)": {Stdout: ""},
	}}
	service := NewService(store, runner, notifications.NewService(store))

	repo, err := service.Add(context.Background(), AddRepoRequest{Path: t.TempDir(), Name: "repo"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	got, err := service.Branches(context.Background(), repo.ID)
	if err != nil {
		t.Fatalf("Branches() error = %v", err)
	}
	if got == nil {
		t.Fatal("Branches() returned nil, want empty slice")
	}
	if len(got) != 0 {
		t.Fatalf("Branches() = %v, want empty slice", got)
	}
}

func TestRefreshFetchesLatestRefs(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	runner := &fakeRunner{}
	service := NewService(store, runner, notifications.NewService(store))

	repo, err := service.Add(context.Background(), AddRepoRequest{Path: t.TempDir(), Name: "repo"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	if err := service.Refresh(context.Background(), repo.ID); err != nil {
		t.Fatalf("Refresh() error = %v", err)
	}
	lastCall := runner.calls[len(runner.calls)-1]
	if want := "git|fetch --all --prune"; !strings.Contains(lastCall, want) {
		t.Fatalf("refresh call = %q, want %q", lastCall, want)
	}
}
