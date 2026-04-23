package agent

import (
	"context"
	"errors"
	"strconv"
	"strings"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/navigation"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Service struct {
	store      *state.Store
	resolver   Resolver
	navigation *navigation.Service
}

func NewService(store *state.Store, resolver Resolver) *Service {
	return &Service{
		store:      store,
		resolver:   resolver,
		navigation: navigation.NewService(store),
	}
}

func NewServiceWithExecutor(store *state.Store, executor CommandExecutor) *Service {
	return NewService(store, NewResolver(executor))
}

func (s *Service) Run(ctx context.Context, request RunRequest) (RunResponse, error) {
	if request.Provider != domain.AgentCodex && request.Provider != domain.AgentClaude {
		return RunResponse{}, errors.New("choose codex or claude for agent mode")
	}
	if strings.TrimSpace(request.Message) == "" {
		return RunResponse{}, errors.New("agent message is required")
	}

	current, err := s.store.Load()
	if err != nil {
		return RunResponse{}, err
	}
	prompt, err := buildPrompt(request.Message, current)
	if err != nil {
		return RunResponse{}, err
	}

	provider, err := s.resolver.Resolve(request.Provider)
	if err != nil {
		return RunResponse{}, err
	}
	raw, err := provider.Run(ctx, prompt)
	if err != nil {
		return RunResponse{Provider: request.Provider, Message: strings.TrimSpace(raw)}, err
	}

	call, err := parseToolCall(raw)
	if err != nil {
		return RunResponse{Provider: request.Provider, Message: strings.TrimSpace(raw)}, errors.New("agent did not return a valid action: " + excerpt(raw))
	}
	result, focus, err := s.executeTool(call)
	if err != nil {
		return RunResponse{}, err
	}
	return RunResponse{
		Provider: request.Provider,
		Message:  firstNonEmpty(call.Reason, "Done."),
		ToolCall: &call,
		Result:   &result,
		State:    focus,
	}, nil
}

func (s *Service) executeTool(call ToolCall) (ToolResult, *domain.WorkspaceFocus, error) {
	current, err := s.store.Load()
	if err != nil {
		return ToolResult{}, nil, err
	}

	switch call.Tool {
	case ToolSelectRepo:
		repoID := call.SelectRepo.RepoID
		if !hasRepo(current.Repos, repoID) {
			return ToolResult{}, nil, errors.New("repo not found")
		}
		mode := domain.UIModeAgent
		return s.updateFocus(navigation.FocusPatch{RepoID: &repoID, Mode: &mode}, "Selected repo.")
	case ToolSelectWorktree:
		worktreeID := call.SelectWorktree.WorktreeID
		worktree, ok := findWorktree(current.Worktrees, worktreeID)
		if !ok {
			return ToolResult{}, nil, errors.New("worktree not found")
		}
		mode := domain.UIModeAgent
		return s.updateFocus(navigation.FocusPatch{WorktreeID: &worktree.ID, Mode: &mode}, "Selected worktree.")
	case ToolSelectSession:
		sessionID := call.SelectSession.SessionID
		session, ok := findSession(current.Sessions, sessionID)
		if !ok {
			return ToolResult{}, nil, errors.New("session not found")
		}
		if _, ok := findWorktree(current.Worktrees, session.WorktreeID); !ok {
			return ToolResult{}, nil, errors.New("session worktree not found")
		}
		mode := domain.UIModeAgent
		return s.updateFocus(navigation.FocusPatch{SessionID: &session.ID, Mode: &mode}, "Selected session.")
	case ToolListWorktrees:
		return ToolResult{OK: true, Message: "Found " + strconv.Itoa(len(current.Worktrees)) + " worktrees."}, &current.Selection, nil
	case ToolListRepos:
		return ToolResult{OK: true, Message: "Found " + strconv.Itoa(len(current.Repos)) + " repos."}, &current.Selection, nil
	case ToolListSessions:
		return ToolResult{OK: true, Message: "Found " + strconv.Itoa(len(current.Sessions)) + " sessions."}, &current.Selection, nil
	default:
		return ToolResult{}, nil, errors.New("unsupported tool: " + string(call.Tool))
	}
}

func (s *Service) updateFocus(patch navigation.FocusPatch, message string) (ToolResult, *domain.WorkspaceFocus, error) {
	next, err := s.navigation.Focus(patch)
	if err != nil {
		return ToolResult{}, nil, err
	}
	return ToolResult{OK: true, Message: message}, &next, nil
}

func hasRepo(repos []domain.Repo, repoID string) bool {
	for _, repo := range repos {
		if repo.ID == repoID {
			return true
		}
	}
	return false
}

func findWorktree(worktrees []domain.Worktree, worktreeID string) (domain.Worktree, bool) {
	for _, worktree := range worktrees {
		if worktree.ID == worktreeID {
			return worktree, true
		}
	}
	return domain.Worktree{}, false
}

func findSession(sessions []domain.Session, sessionID string) (domain.Session, bool) {
	for _, session := range sessions {
		if session.ID == sessionID {
			return session, true
		}
	}
	return domain.Session{}, false
}
