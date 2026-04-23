package sessions

import (
	"errors"
	"sync"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/worktrees"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Service struct {
	store         *state.Store
	worktrees     *worktrees.Service
	starter       ProcessStarter
	terminal      TerminalBackend
	notifications *notifications.Service
	now           func() time.Time
	locks         sync.Map
}

func NewService(store *state.Store, worktrees *worktrees.Service, starter ProcessStarter, notifications *notifications.Service) *Service {
	return &Service{
		store:         store,
		worktrees:     worktrees,
		starter:       starter,
		terminal:      TmuxBackend{},
		notifications: notifications,
		now:           time.Now,
	}
}

func NewServiceWithTerminal(store *state.Store, worktrees *worktrees.Service, terminal TerminalBackend, notifications *notifications.Service) *Service {
	return &Service{
		store:         store,
		worktrees:     worktrees,
		terminal:      terminal,
		notifications: notifications,
		now:           time.Now,
	}
}

func NewServiceWithClock(store *state.Store, worktrees *worktrees.Service, starter ProcessStarter, notifications *notifications.Service, now func() time.Time) *Service {
	service := NewService(store, worktrees, starter, notifications)
	service.now = now
	return service
}

func (s *Service) List() ([]domain.Session, error) {
	current, err := s.store.Load()
	if err != nil {
		return nil, err
	}
	return current.Sessions, nil
}

func (s *Service) Find(sessionID string) (domain.Session, error) {
	current, err := s.store.Load()
	if err != nil {
		return domain.Session{}, err
	}
	for _, session := range current.Sessions {
		if session.ID == sessionID {
			return session, nil
		}
	}
	return domain.Session{}, errors.New("session not found")
}

func (s *Service) lockKey(key string) func() {
	lockValue, _ := s.locks.LoadOrStore(key, &sync.Mutex{})
	lock := lockValue.(*sync.Mutex)
	lock.Lock()
	return lock.Unlock
}
