package navigation

import (
	"errors"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type FocusPatch struct {
	RepoID     *string
	WorktreeID *string
	SessionID  *string
	Mode       *domain.UIMode
}

type Patch = FocusPatch

type Service struct {
	store *state.Store
}

func NewService(store *state.Store) *Service {
	return &Service{store: store}
}

func (s *Service) Focus(patch FocusPatch) (domain.WorkspaceFocus, error) {
	next, err := s.store.Update(func(current *domain.State) error {
		if patch.RepoID != nil {
			if *patch.RepoID != "" && !hasRepo(current.Repos, *patch.RepoID) {
				return errors.New("repo not found")
			}
			current.Selection.RepoID = *patch.RepoID
			if *patch.RepoID == "" {
				current.Selection.WorktreeID = ""
				current.Selection.SessionID = ""
			}
		}
		if patch.WorktreeID != nil {
			if *patch.WorktreeID != "" && !hasWorktree(current.Worktrees, *patch.WorktreeID) {
				return errors.New("worktree not found")
			}
			current.Selection.WorktreeID = *patch.WorktreeID
			if *patch.WorktreeID == "" {
				current.Selection.SessionID = ""
			} else {
				worktree, _ := findWorktree(current.Worktrees, *patch.WorktreeID)
				current.Selection.RepoID = worktree.RepoID
			}
		}
		if patch.SessionID != nil {
			if *patch.SessionID != "" && !hasSession(current.Sessions, *patch.SessionID) {
				return errors.New("session not found")
			}
			current.Selection.SessionID = *patch.SessionID
			if *patch.SessionID != "" {
				session, _ := findSession(current.Sessions, *patch.SessionID)
				current.Selection.WorktreeID = session.WorktreeID
				worktree, ok := findWorktree(current.Worktrees, session.WorktreeID)
				if !ok {
					return errors.New("session worktree not found")
				}
				current.Selection.RepoID = worktree.RepoID
			}
		}
		if patch.Mode != nil {
			switch *patch.Mode {
			case domain.UIModeEffort, domain.UIModeAgent:
				current.Selection.Mode = *patch.Mode
			default:
				return errors.New("invalid mode")
			}
		}
		reconcileSelection(current)
		return nil
	})
	if err != nil {
		return domain.WorkspaceFocus{}, err
	}
	return next.Selection, nil
}

func (s *Service) Update(patch FocusPatch) (domain.WorkspaceFocus, error) {
	return s.Focus(patch)
}

func reconcileSelection(current *domain.State) {
	selection := &current.Selection

	if selection.RepoID != "" && !hasRepo(current.Repos, selection.RepoID) {
		selection.RepoID = ""
		selection.WorktreeID = ""
		selection.SessionID = ""
		return
	}

	if selection.WorktreeID != "" {
		worktree, ok := findWorktree(current.Worktrees, selection.WorktreeID)
		if !ok {
			selection.WorktreeID = ""
			selection.SessionID = ""
		} else {
			selection.RepoID = worktree.RepoID
		}
	}

	if selection.WorktreeID != "" && !worktreeBelongsToRepo(current.Worktrees, selection.WorktreeID, selection.RepoID) {
		selection.WorktreeID = preferredWorktreeID(current.Worktrees, selection.RepoID)
		selection.SessionID = preferredSessionID(current.Sessions, selection.WorktreeID)
	}

	if selection.SessionID == "" && selection.WorktreeID != "" {
		selection.SessionID = preferredSessionID(current.Sessions, selection.WorktreeID)
	}

	if selection.SessionID != "" && !sessionBelongsToWorktree(current.Sessions, selection.SessionID, selection.WorktreeID) {
		selection.SessionID = preferredSessionID(current.Sessions, selection.WorktreeID)
	}
}

func hasRepo(repos []domain.Repo, repoID string) bool {
	for _, repo := range repos {
		if repo.ID == repoID {
			return true
		}
	}
	return false
}

func hasWorktree(worktrees []domain.Worktree, worktreeID string) bool {
	_, ok := findWorktree(worktrees, worktreeID)
	return ok
}

func hasSession(sessions []domain.Session, sessionID string) bool {
	_, ok := findSession(sessions, sessionID)
	return ok
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

func worktreeBelongsToRepo(worktrees []domain.Worktree, worktreeID string, repoID string) bool {
	worktree, ok := findWorktree(worktrees, worktreeID)
	return ok && worktree.RepoID == repoID
}

func sessionBelongsToWorktree(sessions []domain.Session, sessionID string, worktreeID string) bool {
	session, ok := findSession(sessions, sessionID)
	return ok && session.WorktreeID == worktreeID
}

func preferredWorktreeID(worktrees []domain.Worktree, repoID string) string {
	bestID := ""
	var bestUpdated int64
	for _, worktree := range worktrees {
		if worktree.RepoID != repoID {
			continue
		}
		updated := worktree.UpdatedAt.UnixNano()
		if bestID == "" || updated > bestUpdated {
			bestID = worktree.ID
			bestUpdated = updated
		}
	}
	return bestID
}

func preferredSessionID(sessions []domain.Session, worktreeID string) string {
	var best domain.Session
	found := false
	for _, session := range sessions {
		if session.WorktreeID != worktreeID {
			continue
		}
		if !found || compareSessions(session, best) < 0 {
			best = session
			found = true
		}
	}
	if !found {
		return ""
	}
	return best.ID
}

func compareSessions(left domain.Session, right domain.Session) int {
	leftManaged := 1
	rightManaged := 1
	if isDesktopManagedSession(left) {
		leftManaged = 0
	}
	if isDesktopManagedSession(right) {
		rightManaged = 0
	}
	if leftManaged != rightManaged {
		return leftManaged - rightManaged
	}
	leftUpdated := left.UpdatedAt.UnixNano()
	rightUpdated := right.UpdatedAt.UnixNano()
	switch {
	case leftUpdated > rightUpdated:
		return -1
	case leftUpdated < rightUpdated:
		return 1
	default:
		return 0
	}
}

func isDesktopManagedSession(session domain.Session) bool {
	return len(session.SessionName) >= len("mico-desktop-") && session.SessionName[:len("mico-desktop-")] == "mico-desktop-"
}
