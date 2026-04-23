package notifications

import (
	"path/filepath"
	"testing"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func TestPushPrependsNotification(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := NewServiceWithClock(store, func() time.Time { return time.Unix(10, 0).UTC() })

	first, err := service.Push(domain.NotificationInfo, "Started", "one")
	if err != nil {
		t.Fatalf("Push() first error = %v", err)
	}
	second, err := service.Push(domain.NotificationSuccess, "Ready", "two")
	if err != nil {
		t.Fatalf("Push() second error = %v", err)
	}

	got, err := service.List()
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}
	if len(got) != 2 {
		t.Fatalf("len(List()) = %d, want 2", len(got))
	}
	if got[0].ID != second.ID || got[1].ID != first.ID {
		t.Fatalf("notifications not prepended: %+v", got)
	}
}

func TestDismissRemovesNotification(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := NewService(store)

	notification, err := service.Push(domain.NotificationInfo, "Started", "")
	if err != nil {
		t.Fatalf("Push() error = %v", err)
	}
	if err := service.Dismiss(notification.ID); err != nil {
		t.Fatalf("Dismiss() error = %v", err)
	}

	got, err := service.List()
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}
	if len(got) != 0 {
		t.Fatalf("notifications = %+v, want empty list after dismiss", got)
	}
}
