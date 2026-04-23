package worktrees

import (
	"errors"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func (s *Service) MarkStatus(id string, status domain.WorktreeStatus) error {
	_, err := s.store.Update(func(next *domain.State) error {
		for index := range next.Worktrees {
			if next.Worktrees[index].ID == id {
				next.Worktrees[index].Status = status
				next.Worktrees[index].UpdatedAt = s.now()
				return nil
			}
		}
		return errors.New("worktree not found")
	})
	return err
}
