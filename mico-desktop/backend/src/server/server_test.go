package server

import (
	"bufio"
	"context"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/agent"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/doctor"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/logs"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/navigation"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/repos"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/sessions"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/worktrees"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type fakeRunner struct{}

func (fakeRunner) Run(context.Context, string, string, ...string) (repos.CommandResult, error) {
	return repos.CommandResult{Stdout: "main\nfeature"}, nil
}

type emptyBranchesRunner struct{}

func (emptyBranchesRunner) Run(context.Context, string, string, ...string) (repos.CommandResult, error) {
	return repos.CommandResult{}, nil
}

type fakeStarter struct{}

func (fakeStarter) Start(context.Context, string, []string) error {
	return nil
}

func newTestApp(t *testing.T) *App {
	t.Helper()
	runner := fakeRunner{}
	return newTestAppWithRunner(t, runner)
}

func newTestAppWithRunner(t *testing.T, runner repos.CommandRunner) *App {
	t.Helper()
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	notifier := notifications.NewService(store)
	repoService := repos.NewService(store, runner, notifier)
	worktreeService := worktrees.NewService(store, repoService, runner, notifier, filepath.Join(t.TempDir(), "worktrees"))
	sessionService := sessions.NewService(store, worktreeService, fakeStarter{}, notifier)
	agentService := agent.NewServiceWithExecutor(store, agent.ExecCommandExecutor{})
	doctorService := doctor.NewService()
	logService := logs.NewService(store)
	navigationService := navigation.NewService(store)
	return NewApp(store, repoService, worktreeService, sessionService, agentService, doctorService, logService, notifier, navigationService)
}

func TestHealth(t *testing.T) {
	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodGet, "/api/health", nil)

	newTestApp(t).Handler().ServeHTTP(response, request)

	if response.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", response.Code)
	}
	if !strings.Contains(response.Body.String(), `"ok"`) {
		t.Fatalf("body = %s", response.Body.String())
	}
}

func TestAddRepoEndpoint(t *testing.T) {
	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPost, "/api/repos", strings.NewReader(`{"path":"/tmp/mico","name":"mico"}`))
	request.Header.Set("Content-Type", "application/json")

	newTestApp(t).Handler().ServeHTTP(response, request)

	if response.Code != http.StatusCreated {
		t.Fatalf("status = %d, body = %s", response.Code, response.Body.String())
	}
	if !strings.Contains(response.Body.String(), `"name":"mico"`) {
		t.Fatalf("body = %s", response.Body.String())
	}
}

func TestBranchesEndpointReturnsArrayForRepoWithoutBranches(t *testing.T) {
	app := newTestAppWithRunner(t, emptyBranchesRunner{})

	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPost, "/api/repos", strings.NewReader(`{"path":"/tmp/empty-repo","name":"empty-repo"}`))
	request.Header.Set("Content-Type", "application/json")

	app.Handler().ServeHTTP(response, request)
	if response.Code != http.StatusCreated {
		t.Fatalf("add repo status = %d, body = %s", response.Code, response.Body.String())
	}

	var repo domain.Repo
	if err := json.Unmarshal(response.Body.Bytes(), &repo); err != nil {
		t.Fatalf("unmarshal repo error = %v", err)
	}

	branchesResponse := httptest.NewRecorder()
	branchesRequest := httptest.NewRequest(http.MethodGet, "/api/repos/"+repo.ID+"/branches", nil)
	app.Handler().ServeHTTP(branchesResponse, branchesRequest)

	if branchesResponse.Code != http.StatusOK {
		t.Fatalf("branches status = %d, body = %s", branchesResponse.Code, branchesResponse.Body.String())
	}
	if strings.TrimSpace(branchesResponse.Body.String()) != "[]" {
		t.Fatalf("branches body = %s, want []", branchesResponse.Body.String())
	}
}

func TestSelectionEndpointCanonicalizesSessionToWorktreeAndRepo(t *testing.T) {
	app := newTestApp(t)
	now := time.Unix(100, 0).UTC()
	if _, err := app.store.Update(func(current *domain.State) error {
		current.Repos = []domain.Repo{
			{ID: "repo-1", Name: "one", Path: "/tmp/repo-1", CreatedAt: now},
			{ID: "repo-2", Name: "two", Path: "/tmp/repo-2", CreatedAt: now},
		}
		current.Worktrees = []domain.Worktree{
			{ID: "wt-1", RepoID: "repo-1", Branch: "main", Base: "main", Path: "/tmp/wt-1", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now},
			{ID: "wt-2", RepoID: "repo-2", Branch: "feature", Base: "main", Path: "/tmp/wt-2", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now.Add(time.Second)},
		}
		current.Sessions = []domain.Session{
			{ID: "desktop", WorktreeID: "wt-2", Agent: domain.AgentCodex, Command: []string{"codex"}, SessionName: "mico-desktop-wt-2-codex", Status: domain.SessionRunning, CreatedAt: now, UpdatedAt: now},
		}
		current.Selection = domain.UISelection{RepoID: "repo-1", WorktreeID: "wt-1", Mode: domain.UIModeEffort}
		return nil
	}); err != nil {
		t.Fatalf("seed state error = %v", err)
	}

	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPut, "/api/selection", strings.NewReader(`{"sessionId":"desktop","mode":"agent"}`))
	request.Header.Set("Content-Type", "application/json")

	app.Handler().ServeHTTP(response, request)

	if response.Code != http.StatusOK {
		t.Fatalf("status = %d, body = %s", response.Code, response.Body.String())
	}
	body := response.Body.String()
	for _, want := range []string{`"repoId":"repo-2"`, `"worktreeId":"wt-2"`, `"sessionId":"desktop"`, `"mode":"agent"`} {
		if !strings.Contains(body, want) {
			t.Fatalf("body = %s, missing %s", body, want)
		}
	}
}

func TestSelectionEndpointRejectsUnknownSession(t *testing.T) {
	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPut, "/api/selection", strings.NewReader(`{"sessionId":"missing"}`))
	request.Header.Set("Content-Type", "application/json")

	newTestApp(t).Handler().ServeHTTP(response, request)

	if response.Code != http.StatusNotFound {
		t.Fatalf("status = %d, body = %s", response.Code, response.Body.String())
	}
	if !strings.Contains(response.Body.String(), `"session not found"`) {
		t.Fatalf("body = %s", response.Body.String())
	}
}

func TestRepoWorktreeSessionSelectionFlow(t *testing.T) {
	app := newTestApp(t)

	addRepoResponse := httptest.NewRecorder()
	addRepoRequest := httptest.NewRequest(http.MethodPost, "/api/repos", strings.NewReader(`{"path":"/tmp/mico","name":"mico"}`))
	addRepoRequest.Header.Set("Content-Type", "application/json")
	app.Handler().ServeHTTP(addRepoResponse, addRepoRequest)
	if addRepoResponse.Code != http.StatusCreated {
		t.Fatalf("add repo status = %d, body = %s", addRepoResponse.Code, addRepoResponse.Body.String())
	}

	var repo domain.Repo
	if err := json.Unmarshal(addRepoResponse.Body.Bytes(), &repo); err != nil {
		t.Fatalf("unmarshal repo error = %v", err)
	}

	createWorktreeResponse := httptest.NewRecorder()
	createWorktreeRequest := httptest.NewRequest(http.MethodPost, "/api/worktrees", strings.NewReader(`{"repoId":"`+repo.ID+`","branch":"feature/test","base":"main"}`))
	createWorktreeRequest.Header.Set("Content-Type", "application/json")
	app.Handler().ServeHTTP(createWorktreeResponse, createWorktreeRequest)
	if createWorktreeResponse.Code != http.StatusCreated {
		t.Fatalf("create worktree status = %d, body = %s", createWorktreeResponse.Code, createWorktreeResponse.Body.String())
	}

	var worktree domain.Worktree
	if err := json.Unmarshal(createWorktreeResponse.Body.Bytes(), &worktree); err != nil {
		t.Fatalf("unmarshal worktree error = %v", err)
	}

	startSessionResponse := httptest.NewRecorder()
	startSessionRequest := httptest.NewRequest(http.MethodPost, "/api/sessions", strings.NewReader(`{"worktreeId":"`+worktree.ID+`","agent":"codex"}`))
	startSessionRequest.Header.Set("Content-Type", "application/json")
	app.Handler().ServeHTTP(startSessionResponse, startSessionRequest)
	if startSessionResponse.Code != http.StatusCreated {
		t.Fatalf("start session status = %d, body = %s", startSessionResponse.Code, startSessionResponse.Body.String())
	}

	var session domain.Session
	if err := json.Unmarshal(startSessionResponse.Body.Bytes(), &session); err != nil {
		t.Fatalf("unmarshal session error = %v", err)
	}

	selectResponse := httptest.NewRecorder()
	selectRequest := httptest.NewRequest(http.MethodPut, "/api/selection", strings.NewReader(`{"sessionId":"`+session.ID+`","mode":"effort"}`))
	selectRequest.Header.Set("Content-Type", "application/json")
	app.Handler().ServeHTTP(selectResponse, selectRequest)
	if selectResponse.Code != http.StatusOK {
		t.Fatalf("select status = %d, body = %s", selectResponse.Code, selectResponse.Body.String())
	}

	var selection domain.UISelection
	if err := json.Unmarshal(selectResponse.Body.Bytes(), &selection); err != nil {
		t.Fatalf("unmarshal selection error = %v", err)
	}
	if selection.RepoID != repo.ID || selection.WorktreeID != worktree.ID || selection.SessionID != session.ID || selection.Mode != domain.UIModeEffort {
		t.Fatalf("selection = %+v", selection)
	}

	stateResponse := httptest.NewRecorder()
	stateRequest := httptest.NewRequest(http.MethodGet, "/api/state", nil)
	app.Handler().ServeHTTP(stateResponse, stateRequest)
	if stateResponse.Code != http.StatusOK {
		t.Fatalf("state status = %d, body = %s", stateResponse.Code, stateResponse.Body.String())
	}
	if !strings.Contains(stateResponse.Body.String(), `"sessionId":"`+session.ID+`"`) {
		t.Fatalf("state body = %s", stateResponse.Body.String())
	}
}

func TestRunAgentReturnsErrorForInvalidActionOutput(t *testing.T) {
	app := newTestApp(t)
	app.agents = newFakeAgentService(app.store, fakeAgentRunner{output: "no valid action here"})

	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPost, "/api/agent/run", strings.NewReader(`{"provider":"codex","message":"switch to testing-raven"}`))
	request.Header.Set("Content-Type", "application/json")

	app.Handler().ServeHTTP(response, request)

	if response.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, body = %s", response.Code, response.Body.String())
	}
	if !strings.Contains(response.Body.String(), `"agent did not return a valid action:`) {
		t.Fatalf("body = %s", response.Body.String())
	}
}

func TestRunAgentSelectsWorktreeAndReturnsCanonicalSelection(t *testing.T) {
	app := newTestApp(t)
	now := time.Unix(100, 0).UTC()
	if _, err := app.store.Update(func(current *domain.State) error {
		current.Repos = []domain.Repo{
			{ID: "repo-raven", Name: "raven", Path: "/tmp/raven", CreatedAt: now},
		}
		current.Worktrees = []domain.Worktree{
			{ID: "wt-old", RepoID: "repo-raven", Branch: "main", Base: "main", Path: "/tmp/raven-main", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now},
			{ID: "wt-raven", RepoID: "repo-raven", Branch: "testing-raven", Base: "main", Path: "/tmp/raven-testing", Status: domain.WorktreeRunning, CreatedAt: now, UpdatedAt: now.Add(time.Second)},
		}
		current.Sessions = []domain.Session{
			{ID: "legacy-session", WorktreeID: "wt-raven", Agent: domain.AgentTerminal, Command: []string{"/bin/zsh"}, SessionName: "legacy-session", Status: domain.SessionRunning, CreatedAt: now, UpdatedAt: now},
			{ID: "desktop-session", WorktreeID: "wt-raven", Agent: domain.AgentCodex, Command: []string{"codex"}, SessionName: "mico-desktop-wt-raven-codex", Status: domain.SessionRunning, CreatedAt: now.Add(time.Second), UpdatedAt: now.Add(time.Second)},
		}
		current.Selection = domain.UISelection{
			RepoID:     "repo-raven",
			WorktreeID: "wt-old",
			Mode:       domain.UIModeEffort,
		}
		return nil
	}); err != nil {
		t.Fatalf("seed state error = %v", err)
	}

	app.agents = newFakeAgentService(app.store, fakeAgentRunner{
		output: `{"tool":"select_worktree","reason":"testing-raven matches directly","selectWorktree":{"worktreeId":"wt-raven"}}`,
	})

	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPost, "/api/agent/run", strings.NewReader(`{"provider":"codex","message":"switch to my testing-raven worktree"}`))
	request.Header.Set("Content-Type", "application/json")

	app.Handler().ServeHTTP(response, request)

	if response.Code != http.StatusOK {
		t.Fatalf("status = %d, body = %s", response.Code, response.Body.String())
	}
	body := response.Body.String()
	for _, want := range []string{
		`"toolCall":{"tool":"select_worktree"`,
		`"repoId":"repo-raven"`,
		`"worktreeId":"wt-raven"`,
		`"sessionId":"desktop-session"`,
		`"mode":"agent"`,
	} {
		if !strings.Contains(body, want) {
			t.Fatalf("body = %s, missing %s", body, want)
		}
	}
}

func TestRecoverMiddlewareReturnsInternalServerErrorOnPanic(t *testing.T) {
	app := newTestApp(t)
	handler := app.withRecover(http.HandlerFunc(func(http.ResponseWriter, *http.Request) {
		panic("boom")
	}))

	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodGet, "/api/panic", nil)
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusInternalServerError {
		t.Fatalf("status = %d, body = %s", response.Code, response.Body.String())
	}
	if !strings.Contains(response.Body.String(), `"internal server error"`) {
		t.Fatalf("body = %s", response.Body.String())
	}
}

type fakeAgentRunner struct {
	output string
	err    error
}

func (f fakeAgentRunner) Run(context.Context, string) (string, error) {
	return f.output, f.err
}

func newFakeAgentService(store *state.Store, provider fakeAgentRunner) *agent.Service {
	return agent.NewService(store, agent.NewResolverWithProviders(map[domain.AgentKind]agent.Provider{
		domain.AgentCodex:  provider,
		domain.AgentClaude: provider,
	}))
}

func TestHandleTerminalMessageWritesInput(t *testing.T) {
	reader, writer, err := os.Pipe()
	if err != nil {
		t.Fatalf("Pipe() error = %v", err)
	}
	defer reader.Close()
	defer writer.Close()

	done := make(chan string, 1)
	go func() {
		buffer := make([]byte, len("hello"))
		n, _ := io.ReadFull(reader, buffer)
		done <- string(buffer[:n])
	}()

	if err := handleTerminalMessage(writer, []byte(`{"type":"input","data":"hello"}`)); err != nil {
		t.Fatalf("handleTerminalMessage() error = %v", err)
	}

	select {
	case got := <-done:
		if got != "hello" {
			t.Fatalf("input = %q, want hello", got)
		}
	case <-time.After(time.Second):
		t.Fatal("timed out reading terminal input")
	}
}

func TestHandleTerminalMessageWritesRawBytes(t *testing.T) {
	reader, writer, err := os.Pipe()
	if err != nil {
		t.Fatalf("Pipe() error = %v", err)
	}
	defer reader.Close()
	defer writer.Close()

	done := make(chan string, 1)
	go func() {
		buffer := make([]byte, len("raw"))
		n, _ := io.ReadFull(reader, buffer)
		done <- string(buffer[:n])
	}()

	if err := handleTerminalMessage(writer, []byte("raw")); err != nil {
		t.Fatalf("handleTerminalMessage() error = %v", err)
	}

	select {
	case got := <-done:
		if got != "raw" {
			t.Fatalf("input = %q, want raw", got)
		}
	case <-time.After(time.Second):
		t.Fatal("timed out reading raw terminal input")
	}
}

func TestTerminalClientEnvOverridesDumbTerm(t *testing.T) {
	env := terminalClientEnv([]string{
		"PATH=/usr/bin:/bin",
		"TERM=dumb",
		"COLORTERM=",
		"HOME=/tmp/test-home",
	})

	joined := strings.Join(env, "\n")
	for _, want := range []string{
		"PATH=/usr/bin:/bin",
		"HOME=/tmp/test-home",
		"TERM=xterm-256color",
		"COLORTERM=truecolor",
	} {
		if !strings.Contains(joined, want) {
			t.Fatalf("env = %q, missing %q", joined, want)
		}
	}
	if strings.Contains(joined, "TERM=dumb") {
		t.Fatalf("env = %q, still contains TERM=dumb", joined)
	}
}

type fakeHijackWriter struct {
	header       http.Header
	hijackCalled bool
}

func (w *fakeHijackWriter) Header() http.Header {
	if w.header == nil {
		w.header = make(http.Header)
	}
	return w.header
}

func (w *fakeHijackWriter) Write([]byte) (int, error) {
	return 0, nil
}

func (w *fakeHijackWriter) WriteHeader(int) {}

func (w *fakeHijackWriter) Hijack() (net.Conn, *bufio.ReadWriter, error) {
	w.hijackCalled = true
	return nil, bufio.NewReadWriter(bufio.NewReader(strings.NewReader("")), bufio.NewWriter(io.Discard)), nil
}

func TestStatusRecorderForwardsHijacker(t *testing.T) {
	writer := &fakeHijackWriter{}
	recorder := &statusRecorder{ResponseWriter: writer, status: http.StatusOK}

	_, _, err := recorder.Hijack()
	if err != nil {
		t.Fatalf("Hijack() error = %v", err)
	}
	if !writer.hijackCalled {
		t.Fatal("expected wrapped Hijack to be called")
	}
}
