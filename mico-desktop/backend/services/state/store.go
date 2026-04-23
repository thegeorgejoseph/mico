package state

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Store struct {
	path string
	mu   sync.Mutex
}

func NewStore(path string) *Store {
	return &Store{path: path}
}

func (s *Store) Path() string {
	return s.path
}

func DefaultState() domain.State {
	return domain.State{
		Version:       1,
		Repos:         []domain.Repo{},
		Worktrees:     []domain.Worktree{},
		Sessions:      []domain.Session{},
		Notifications: []domain.Notification{},
		Selection:     domain.UISelection{Mode: domain.UIModeEffort},
		Logs:          []domain.LogEvent{},
		Migrations:    []domain.AppliedMigration{},
	}
}

func (s *Store) Load() (domain.State, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	current, err := s.loadUnlocked()
	if err != nil {
		return domain.State{}, err
	}
	return cloneState(current), nil
}

func (s *Store) Save(next domain.State) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.saveUnlocked(cloneState(next))
}

func (s *Store) Update(mutator func(*domain.State) error) (domain.State, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	current, err := s.loadUnlocked()
	if err != nil {
		return domain.State{}, err
	}
	if err := mutator(&current); err != nil {
		return domain.State{}, err
	}
	if err := s.saveUnlocked(current); err != nil {
		return domain.State{}, err
	}
	return cloneState(current), nil
}

func (s *Store) loadUnlocked() (domain.State, error) {
	data, err := os.ReadFile(s.path)
	if errors.Is(err, os.ErrNotExist) {
		next := DefaultState()
		if err := s.saveUnlocked(next); err != nil {
			return domain.State{}, err
		}
		return next, nil
	}
	if err != nil {
		return domain.State{}, err
	}

	var loaded domain.State
	if err := json.Unmarshal(data, &loaded); err != nil {
		return domain.State{}, err
	}
	if normalize(&loaded) {
		if err := s.saveUnlocked(loaded); err != nil {
			return domain.State{}, err
		}
	}
	return loaded, nil
}

func (s *Store) saveUnlocked(next domain.State) error {
	normalize(&next)
	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(next, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return os.WriteFile(s.path, data, 0o644)
}

func normalize(next *domain.State) bool {
	changed := false
	if next.Version == 0 {
		next.Version = 1
		changed = true
	}
	if next.Repos == nil {
		next.Repos = []domain.Repo{}
		changed = true
	}
	if next.Worktrees == nil {
		next.Worktrees = []domain.Worktree{}
		changed = true
	}
	if next.Sessions == nil {
		next.Sessions = []domain.Session{}
		changed = true
	}
	if next.Notifications == nil {
		next.Notifications = []domain.Notification{}
		changed = true
	} else {
		visible := next.Notifications[:0]
		for _, notification := range next.Notifications {
			if notification.Seen {
				changed = true
				continue
			}
			visible = append(visible, notification)
		}
		next.Notifications = visible
	}
	if next.Logs == nil {
		next.Logs = []domain.LogEvent{}
		changed = true
	}
	if next.Migrations == nil {
		next.Migrations = []domain.AppliedMigration{}
		changed = true
	}
	if next.Selection.Mode == "" {
		next.Selection.Mode = domain.UIModeEffort
		changed = true
	}
	return changed
}

func cloneState(current domain.State) domain.State {
	next := current
	next.Repos = append([]domain.Repo{}, current.Repos...)
	next.Worktrees = append([]domain.Worktree{}, current.Worktrees...)
	next.Notifications = append([]domain.Notification{}, current.Notifications...)
	next.Migrations = append([]domain.AppliedMigration{}, current.Migrations...)
	next.Logs = make([]domain.LogEvent, len(current.Logs))
	for index := range current.Logs {
		next.Logs[index] = current.Logs[index]
		next.Logs[index].Fields = cloneFields(current.Logs[index].Fields)
	}
	next.Sessions = make([]domain.Session, len(current.Sessions))
	for index := range current.Sessions {
		next.Sessions[index] = current.Sessions[index]
		next.Sessions[index].Command = append([]string{}, current.Sessions[index].Command...)
	}
	return next
}

func cloneFields(current map[string]string) map[string]string {
	if len(current) == 0 {
		return nil
	}
	next := make(map[string]string, len(current))
	for key, value := range current {
		next[key] = value
	}
	return next
}
