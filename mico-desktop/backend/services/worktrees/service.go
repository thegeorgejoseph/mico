package worktrees

import (
	"errors"
	"sync"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/repos"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Service struct {
	store         *state.Store
	repos         *repos.Service
	runner        repos.CommandRunner
	notifications *notifications.Service
	root          string
	now           func() time.Time
	locks         sync.Map
}

func NewService(store *state.Store, repos *repos.Service, runner repos.CommandRunner, notifications *notifications.Service, root string) *Service {
	return &Service{
		store:         store,
		repos:         repos,
		runner:        runner,
		notifications: notifications,
		root:          root,
		now:           time.Now,
	}
}

func NewServiceWithClock(store *state.Store, repos *repos.Service, runner repos.CommandRunner, notifications *notifications.Service, root string, now func() time.Time) *Service {
	service := NewService(store, repos, runner, notifications, root)
	service.now = now
	return service
}

func (s *Service) List() ([]domain.Worktree, error) {
	current, err := s.store.Load()
	if err != nil {
		return nil, err
	}
	return current.Worktrees, nil
}

func (s *Service) Find(id string) (domain.Worktree, error) {
	current, err := s.store.Load()
	if err != nil {
		return domain.Worktree{}, err
	}
	for _, worktree := range current.Worktrees {
		if worktree.ID == id {
			return worktree, nil
		}
	}
	return domain.Worktree{}, errors.New("worktree not found")
}
