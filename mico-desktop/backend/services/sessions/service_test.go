package sessions

import (
	"context"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/repos"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/worktrees"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type fakeRunner struct{}

func (fakeRunner) Run(context.Context, string, string, ...string) (repos.CommandResult, error) {
	return repos.CommandResult{}, nil
}

type fakeStarter struct {
	dir     string
	command []string
}

func (f *fakeStarter) Start(_ context.Context, dir string, command []string) error {
	f.dir = dir
	f.command = command
	return nil
}

type fakeTerminal struct {
	mu          sync.Mutex
	captures    map[string][]string
	dir         string
	command     []string
	sessionName string
	sessions    map[string]bool
	sent        string
	stopped     []string
}

func (f *fakeTerminal) Start(_ context.Context, sessionName string, dir string, command []string) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.sessionName = sessionName
	f.dir = dir
	f.command = append([]string(nil), command...)
	if f.sessions == nil {
		f.sessions = map[string]bool{}
	}
	f.sessions[sessionName] = true
	return nil
}

func (f *fakeTerminal) Has(sessionName string) bool {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.sessions[sessionName]
}

func (f *fakeTerminal) Capture(sessionName string, _ int) ([]string, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.captures != nil {
		if lines, ok := f.captures[sessionName]; ok {
			return append([]string(nil), lines...), nil
		}
	}
	return []string{"hello"}, nil
}

func (f *fakeTerminal) Send(_ string, text string) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.sent = text
	return nil
}

func (f *fakeTerminal) Stop(sessionName string) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.sessions != nil {
		delete(f.sessions, sessionName)
	}
	f.stopped = append(f.stopped, sessionName)
	return nil
}

func TestStartSessionPersistsCommandAndMarksWorktreeRunning(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	repoService := repos.NewService(store, fakeRunner{}, notifier)
	repo, err := repoService.Add(context.Background(), repos.AddRepoRequest{Path: t.TempDir(), Name: "repo"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	worktreeService := worktrees.NewService(store, repoService, fakeRunner{}, notifier, filepath.Join(t.TempDir(), "worktrees"))
	worktree, err := worktreeService.Create(context.Background(), worktrees.CreateRequest{RepoID: repo.ID, Branch: "desktop", Base: "main"})
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}
	terminal := &fakeTerminal{}
	service := NewServiceWithTerminal(store, worktreeService, terminal, notifier)

	session, err := service.Start(context.Background(), StartRequest{WorktreeID: worktree.ID, Agent: domain.AgentCodex})
	if err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	if session.Agent != domain.AgentCodex || len(session.Command) != 3 || session.Command[0] != "codex" || session.Command[1] != "-c" || session.Command[2] != "check_for_update_on_startup=false" {
		t.Fatalf("session = %+v", session)
	}
	if terminal.dir != worktree.Path {
		t.Fatalf("terminal dir = %q, want %q", terminal.dir, worktree.Path)
	}
	if session.SessionName == "" {
		t.Fatal("session should include durable tmux session name")
	}

	updated, err := worktreeService.Find(worktree.ID)
	if err != nil {
		t.Fatalf("Find() error = %v", err)
	}
	if updated.Status != domain.WorktreeRunning {
		t.Fatalf("worktree status = %q, want running", updated.Status)
	}
}

func TestStartSessionReusesImportedLegacySessionWhenTmuxStillExists(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	now := time.Unix(100, 0).UTC()
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{{
			ID:        "repo-1",
			Name:      "repo",
			Path:      t.TempDir(),
			CreatedAt: now,
		}},
		Worktrees: []domain.Worktree{{
			ID:        "wt-legacy",
			RepoID:    "repo-1",
			Branch:    "feature/legacy",
			Base:      "main",
			Path:      filepath.Join(t.TempDir(), "worktree"),
			Status:    domain.WorktreeRunning,
			CreatedAt: now,
			UpdatedAt: now,
		}},
		Sessions: []domain.Session{{
			ID:          "ses-legacy",
			WorktreeID:  "wt-legacy",
			Agent:       domain.AgentCodex,
			Command:     []string{"codex", "-c", "check_for_update_on_startup=false"},
			SessionName: "mico-repo-feature-legacy",
			Status:      domain.SessionRunning,
			CreatedAt:   now,
			UpdatedAt:   now,
		}},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	repoService := repos.NewService(store, fakeRunner{}, notifier)
	worktreeService := worktrees.NewService(store, repoService, fakeRunner{}, notifier, filepath.Join(t.TempDir(), "worktrees"))
	terminal := &fakeTerminal{
		sessions: map[string]bool{"mico-repo-feature-legacy": true},
	}
	service := NewServiceWithTerminal(store, worktreeService, terminal, notifier)

	session, err := service.Start(context.Background(), StartRequest{WorktreeID: "wt-legacy", Agent: domain.AgentCodex})
	if err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	if session.ID != "ses-legacy" {
		t.Fatalf("session ID = %q, want imported legacy session", session.ID)
	}
	if terminal.sessionName != "" {
		t.Fatalf("terminal should not start a new tmux session, got %q", terminal.sessionName)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if len(loaded.Sessions) != 1 {
		t.Fatalf("sessions = %+v", loaded.Sessions)
	}
}

func TestImportedWorktreeCanStartFreshDesktopSessionWhenLegacyTmuxIsGone(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	now := time.Unix(100, 0).UTC()
	worktreePath := filepath.Join(t.TempDir(), "worktree")
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{{
			ID:        "repo-1",
			Name:      "repo",
			Path:      t.TempDir(),
			CreatedAt: now,
		}},
		Worktrees: []domain.Worktree{{
			ID:        "wt-legacy",
			RepoID:    "repo-1",
			Branch:    "feature/legacy",
			Base:      "main",
			Path:      worktreePath,
			Status:    domain.WorktreeStopped,
			CreatedAt: now,
			UpdatedAt: now,
		}},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	repoService := repos.NewService(store, fakeRunner{}, notifier)
	worktreeService := worktrees.NewService(store, repoService, fakeRunner{}, notifier, filepath.Join(t.TempDir(), "worktrees"))
	terminal := &fakeTerminal{}
	service := NewServiceWithTerminal(store, worktreeService, terminal, notifier)

	session, err := service.Start(context.Background(), StartRequest{WorktreeID: "wt-legacy", Agent: domain.AgentTerminal})
	if err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	if session.WorktreeID != "wt-legacy" {
		t.Fatalf("session worktree = %q, want imported worktree", session.WorktreeID)
	}
	if terminal.dir != worktreePath {
		t.Fatalf("terminal dir = %q, want %q", terminal.dir, worktreePath)
	}
	if session.SessionName == "" || session.SessionName == "mico-repo-feature-legacy" {
		t.Fatalf("expected a new desktop session name, got %q", session.SessionName)
	}
}

func TestStopSessionMarksSessionExited(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	repoService := repos.NewService(store, fakeRunner{}, notifier)
	repo, err := repoService.Add(context.Background(), repos.AddRepoRequest{Path: t.TempDir(), Name: "repo"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	worktreeService := worktrees.NewService(store, repoService, fakeRunner{}, notifier, filepath.Join(t.TempDir(), "worktrees"))
	worktree, err := worktreeService.Create(context.Background(), worktrees.CreateRequest{RepoID: repo.ID, Branch: "desktop", Base: "main"})
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}
	terminal := &fakeTerminal{}
	service := NewServiceWithTerminal(store, worktreeService, terminal, notifier)
	session, err := service.Start(context.Background(), StartRequest{WorktreeID: worktree.ID, Agent: domain.AgentTerminal})
	if err != nil {
		t.Fatalf("Start() error = %v", err)
	}

	stopped, err := service.Stop(session.ID)
	if err != nil {
		t.Fatalf("Stop() error = %v", err)
	}
	if stopped.Status != domain.SessionExited {
		t.Fatalf("session status = %q, want exited", stopped.Status)
	}
	if len(terminal.stopped) != 1 || terminal.stopped[0] != session.SessionName {
		t.Fatalf("stopped sessions = %+v", terminal.stopped)
	}
}

func TestResumeSessionRecreatesStoppedTmuxSession(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	now := time.Unix(100, 0).UTC()
	worktreePath := filepath.Join(t.TempDir(), "worktree")
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{{
			ID:        "repo-1",
			Name:      "repo",
			Path:      t.TempDir(),
			CreatedAt: now,
		}},
		Worktrees: []domain.Worktree{{
			ID:        "wt-1",
			RepoID:    "repo-1",
			Branch:    "feature/resume",
			Base:      "main",
			Path:      worktreePath,
			Status:    domain.WorktreeStopped,
			CreatedAt: now,
			UpdatedAt: now,
		}},
		Sessions: []domain.Session{{
			ID:          "ses-1",
			WorktreeID:  "wt-1",
			Agent:       domain.AgentCodex,
			Command:     []string{"codex"},
			SessionName: "mico-repo-feature-resume",
			Status:      domain.SessionExited,
			CreatedAt:   now,
			UpdatedAt:   now,
		}},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}
	repoService := repos.NewService(store, fakeRunner{}, notifier)
	worktreeService := worktrees.NewService(store, repoService, fakeRunner{}, notifier, filepath.Join(t.TempDir(), "worktrees"))
	terminal := &fakeTerminal{}
	service := NewServiceWithTerminal(store, worktreeService, terminal, notifier)

	resumed, err := service.Resume(context.Background(), "ses-1")
	if err != nil {
		t.Fatalf("Resume() error = %v", err)
	}
	if resumed.Status != domain.SessionRunning {
		t.Fatalf("status = %q, want running", resumed.Status)
	}
	if terminal.sessionName != "mico-repo-feature-resume" {
		t.Fatalf("sessionName = %q", terminal.sessionName)
	}
	if terminal.dir != worktreePath {
		t.Fatalf("dir = %q, want %q", terminal.dir, worktreePath)
	}
	if got := strings.Join(terminal.command, " "); got != "codex -c check_for_update_on_startup=false" {
		t.Fatalf("command = %q", got)
	}
}

type blockingTerminal struct {
	fakeTerminal
	started chan struct{}
	release chan struct{}
}

func (f *blockingTerminal) Start(ctx context.Context, sessionName string, dir string, command []string) error {
	f.started <- struct{}{}
	<-f.release
	return f.fakeTerminal.Start(ctx, sessionName, dir, command)
}

func TestStartSessionSerializesDuplicateRequests(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	repoService := repos.NewService(store, fakeRunner{}, notifier)
	repo, err := repoService.Add(context.Background(), repos.AddRepoRequest{Path: t.TempDir(), Name: "repo"})
	if err != nil {
		t.Fatalf("Add() error = %v", err)
	}
	worktreeService := worktrees.NewService(store, repoService, fakeRunner{}, notifier, filepath.Join(t.TempDir(), "worktrees"))
	worktree, err := worktreeService.Create(context.Background(), worktrees.CreateRequest{RepoID: repo.ID, Branch: "desktop", Base: "main"})
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}
	terminal := &blockingTerminal{
		started: make(chan struct{}, 1),
		release: make(chan struct{}),
	}
	service := NewServiceWithTerminal(store, worktreeService, terminal, notifier)

	sessionsOut := make(chan domain.Session, 2)
	errs := make(chan error, 2)
	go func() {
		session, startErr := service.Start(context.Background(), StartRequest{WorktreeID: worktree.ID, Agent: domain.AgentCodex})
		sessionsOut <- session
		errs <- startErr
	}()
	<-terminal.started
	go func() {
		session, startErr := service.Start(context.Background(), StartRequest{WorktreeID: worktree.ID, Agent: domain.AgentCodex})
		sessionsOut <- session
		errs <- startErr
	}()
	close(terminal.release)

	sessionA := <-sessionsOut
	errA := <-errs
	sessionB := <-sessionsOut
	errB := <-errs
	if errA != nil || errB != nil {
		t.Fatalf("Start() errors = %v / %v", errA, errB)
	}
	if sessionA.SessionName != sessionB.SessionName {
		t.Fatalf("session names = %q / %q, want same durable session", sessionA.SessionName, sessionB.SessionName)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if len(loaded.Sessions) != 1 {
		t.Fatalf("sessions = %+v", loaded.Sessions)
	}

	terminal.mu.Lock()
	defer terminal.mu.Unlock()
	if !strings.Contains(terminal.sessionName, "mico-desktop-") {
		t.Fatalf("sessionName = %q, want desktop tmux session", terminal.sessionName)
	}
}

func TestStartSessionRestartsCodexSessionBlockedOnUpdater(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	now := time.Unix(100, 0).UTC()
	worktreePath := filepath.Join(t.TempDir(), "worktree")
	if err := store.Save(domain.State{
		Version: 1,
		Repos: []domain.Repo{{
			ID:        "repo-1",
			Name:      "repo",
			Path:      t.TempDir(),
			CreatedAt: now,
		}},
		Worktrees: []domain.Worktree{{
			ID:        "wt-legacy",
			RepoID:    "repo-1",
			Branch:    "feature/legacy",
			Base:      "main",
			Path:      worktreePath,
			Status:    domain.WorktreeRunning,
			CreatedAt: now,
			UpdatedAt: now,
		}},
		Sessions: []domain.Session{{
			ID:          "ses-legacy",
			WorktreeID:  "wt-legacy",
			Agent:       domain.AgentCodex,
			Command:     []string{"codex"},
			SessionName: "mico-repo-feature-legacy",
			Status:      domain.SessionRunning,
			CreatedAt:   now,
			UpdatedAt:   now,
		}},
		Selection: domain.UISelection{Mode: domain.UIModeEffort},
	}); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	repoService := repos.NewService(store, fakeRunner{}, notifier)
	worktreeService := worktrees.NewService(store, repoService, fakeRunner{}, notifier, filepath.Join(t.TempDir(), "worktrees"))
	terminal := &fakeTerminal{
		captures: map[string][]string{
			"mico-repo-feature-legacy": {
				"Update available! 0.116.0 -> 0.122.0",
				"3. Skip until next version",
				"Press enter to continue",
			},
		},
		sessions: map[string]bool{"mico-repo-feature-legacy": true},
	}
	service := NewServiceWithTerminal(store, worktreeService, terminal, notifier)

	session, err := service.Start(context.Background(), StartRequest{WorktreeID: "wt-legacy", Agent: domain.AgentCodex})
	if err != nil {
		t.Fatalf("Start() error = %v", err)
	}
	if session.ID != "ses-legacy" {
		t.Fatalf("session ID = %q, want original", session.ID)
	}
	if session.SessionName != "mico-desktop-wt-legacy-codex" {
		t.Fatalf("session name = %q", session.SessionName)
	}
	if len(terminal.stopped) != 1 || terminal.stopped[0] != "mico-repo-feature-legacy" {
		t.Fatalf("stopped = %+v", terminal.stopped)
	}
	if got := strings.Join(session.Command, " "); got != "codex -c check_for_update_on_startup=false" {
		t.Fatalf("command = %q", got)
	}
}
