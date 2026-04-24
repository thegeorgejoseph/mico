package sessions

import (
	"context"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func (s *Service) Capture(sessionID string, lines int) ([]string, error) {
	session, err := s.Find(sessionID)
	if err != nil {
		return nil, err
	}
	return s.terminal.Capture(session.SessionName, lines)
}

func (s *Service) Send(sessionID string, text string) error {
	session, err := s.Find(sessionID)
	if err != nil {
		return err
	}
	return s.terminal.Send(session.SessionName, text)
}

func (s *Service) Stop(sessionID string) (domain.Session, error) {
	session, err := s.Find(sessionID)
	if err != nil {
		return domain.Session{}, err
	}
	if err := s.terminal.Stop(session.SessionName); err != nil {
		return domain.Session{}, err
	}
	now := s.now()
	updated, err := s.store.Update(func(next *domain.State) error {
		worktreeRunning := false
		for index := range next.Sessions {
			if next.Sessions[index].ID == sessionID {
				next.Sessions[index].Status = domain.SessionExited
				next.Sessions[index].UpdatedAt = now
				session = next.Sessions[index]
				continue
			}
			if next.Sessions[index].WorktreeID == session.WorktreeID && next.Sessions[index].Status == domain.SessionRunning && s.terminal.Has(next.Sessions[index].SessionName) {
				worktreeRunning = true
			}
		}
		for index := range next.Worktrees {
			if next.Worktrees[index].ID == session.WorktreeID {
				if worktreeRunning {
					next.Worktrees[index].Status = domain.WorktreeRunning
				} else {
					next.Worktrees[index].Status = domain.WorktreeStopped
				}
				next.Worktrees[index].UpdatedAt = now
			}
		}
		return nil
	})
	if err != nil {
		return domain.Session{}, err
	}
	_ = updated
	_, _ = s.notifications.Push(domain.NotificationWarning, "Session stopped", string(session.Agent))
	return session, nil
}

func (s *Service) Resume(ctx context.Context, sessionID string) (domain.Session, error) {
	session, err := s.Find(sessionID)
	if err != nil {
		return domain.Session{}, err
	}
	worktree, err := s.worktrees.Find(session.WorktreeID)
	if err != nil {
		return domain.Session{}, err
	}
	command := commandFor(session.Agent)
	if err := s.terminal.Start(ctx, session.SessionName, worktree.Path, command); err != nil {
		_, _ = s.notifications.Push(domain.NotificationError, "Session resume failed", err.Error())
		return domain.Session{}, err
	}
	now := s.now()
	_, err = s.store.Update(func(next *domain.State) error {
		for index := range next.Sessions {
			if next.Sessions[index].ID == sessionID {
				next.Sessions[index].Command = command
				next.Sessions[index].Status = domain.SessionRunning
				next.Sessions[index].UpdatedAt = now
				session = next.Sessions[index]
			}
		}
		for index := range next.Worktrees {
			if next.Worktrees[index].ID == session.WorktreeID {
				next.Worktrees[index].Status = domain.WorktreeRunning
				next.Worktrees[index].UpdatedAt = now
			}
		}
		return nil
	})
	if err != nil {
		return domain.Session{}, err
	}
	_, _ = s.notifications.Push(domain.NotificationInfo, "Session resumed", string(session.Agent))
	return session, nil
}
