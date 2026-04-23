package sessions

import (
	"context"
	"errors"
	"strings"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type StartRequest struct {
	WorktreeID string           `json:"worktreeId"`
	Agent      domain.AgentKind `json:"agent"`
}

func (s *Service) Start(ctx context.Context, request StartRequest) (domain.Session, error) {
	if !request.Agent.Valid() {
		return domain.Session{}, errors.New("unsupported agent")
	}
	worktree, err := s.worktrees.Find(request.WorktreeID)
	if err != nil {
		return domain.Session{}, err
	}
	command := commandFor(request.Agent)
	sessionName := tmuxSessionName(worktree.ID, request.Agent)
	unlock := s.lockKey(worktree.ID + "::" + string(request.Agent))
	defer unlock()

	current, err := s.store.Load()
	if err != nil {
		return domain.Session{}, err
	}
	var existingSession *domain.Session
	for _, candidate := range current.Sessions {
		if candidate.WorktreeID == worktree.ID && candidate.Agent == request.Agent {
			sessionCopy := candidate
			existingSession = &sessionCopy
			if s.terminal.Has(sessionCopy.SessionName) {
				if !s.sessionNeedsRestart(sessionCopy) {
					return sessionCopy, nil
				}
				if err := s.terminal.Stop(sessionCopy.SessionName); err != nil {
					return domain.Session{}, err
				}
			}
			break
		}
	}

	if err := s.terminal.Start(ctx, sessionName, worktree.Path, command); err != nil {
		_, _ = s.notifications.Push(domain.NotificationError, "Session failed", err.Error())
		return domain.Session{}, err
	}

	now := s.now()
	session := domain.Session{
		ID:          domain.NewID("ses"),
		WorktreeID:  worktree.ID,
		Agent:       request.Agent,
		Command:     command,
		SessionName: sessionName,
		Status:      domain.SessionRunning,
		CreatedAt:   now,
		UpdatedAt:   now,
	}
	if existingSession != nil {
		session = domain.Session{
			ID:          existingSession.ID,
			WorktreeID:  worktree.ID,
			Agent:       request.Agent,
			Command:     command,
			SessionName: sessionName,
			Status:      domain.SessionRunning,
			CreatedAt:   existingSession.CreatedAt,
			UpdatedAt:   now,
		}
	}
	_, err = s.store.Update(func(next *domain.State) error {
		replaced := false
		for index := range next.Sessions {
			if next.Sessions[index].ID == session.ID {
				next.Sessions[index] = session
				replaced = true
				break
			}
		}
		if !replaced {
			next.Sessions = append(next.Sessions, session)
		}
		for index := range next.Worktrees {
			if next.Worktrees[index].ID == worktree.ID {
				next.Worktrees[index].Status = domain.WorktreeRunning
				next.Worktrees[index].UpdatedAt = now
			}
		}
		return nil
	})
	if err != nil {
		return domain.Session{}, err
	}
	_, _ = s.notifications.Push(domain.NotificationInfo, "Session started", string(request.Agent))
	return session, nil
}

func commandFor(agent domain.AgentKind) []string {
	switch agent {
	case domain.AgentCodex:
		return []string{"codex", "-c", "check_for_update_on_startup=false"}
	case domain.AgentClaude:
		return []string{"claude"}
	default:
		shell := strings.TrimSpace(getShell())
		if shell == "" {
			shell = "/bin/zsh"
		}
		return []string{shell}
	}
}

func (s *Service) sessionNeedsRestart(session domain.Session) bool {
	if session.Agent != domain.AgentCodex {
		return false
	}
	lines, err := s.terminal.Capture(session.SessionName, 80)
	if err != nil {
		return false
	}
	transcript := strings.Join(lines, "\n")
	return strings.Contains(transcript, "Update available!") &&
		strings.Contains(transcript, "Skip until next version") &&
		strings.Contains(transcript, "Press enter to continue")
}
