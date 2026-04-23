package notifications

import (
	"errors"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Service struct {
	store *state.Store
	now   func() time.Time
}

func NewService(store *state.Store) *Service {
	return &Service{store: store, now: time.Now}
}

func NewServiceWithClock(store *state.Store, now func() time.Time) *Service {
	return &Service{store: store, now: now}
}

func (s *Service) List() ([]domain.Notification, error) {
	current, err := s.store.Load()
	if err != nil {
		return nil, err
	}
	return current.Notifications, nil
}

func (s *Service) Push(level domain.NotificationLevel, title string, body string) (domain.Notification, error) {
	if title == "" {
		return domain.Notification{}, errors.New("notification title is required")
	}

	notification := domain.Notification{
		ID:        domain.NewID("ntf"),
		Level:     level,
		Title:     title,
		Body:      body,
		CreatedAt: s.now(),
	}

	_, err := s.store.Update(func(next *domain.State) error {
		next.Notifications = append([]domain.Notification{notification}, next.Notifications...)
		return nil
	})
	return notification, err
}

func (s *Service) Dismiss(id string) error {
	_, err := s.store.Update(func(next *domain.State) error {
		for index := range next.Notifications {
			if next.Notifications[index].ID == id {
				next.Notifications = append(next.Notifications[:index], next.Notifications[index+1:]...)
				return nil
			}
		}
		return errors.New("notification not found")
	})
	return err
}

func (s *Service) MarkSeen(id string) error {
	return s.Dismiss(id)
}
