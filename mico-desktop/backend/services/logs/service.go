package logs

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

const (
	maxEvents        = 120
	maxSessionEvents = 10
)

type Service struct {
	path   string
	events []domain.LogEvent
	mu     sync.RWMutex
	now    func() time.Time
}

func NewService(store *state.Store) *Service {
	service := &Service{
		path: filepath.Join(filepath.Dir(store.Path()), "logs.jsonl"),
		now:  time.Now,
	}
	service.load()
	return service
}

func (s *Service) List(limit int) ([]domain.LogEvent, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if limit <= 0 || limit > len(s.events) {
		return cloneEvents(s.events), nil
	}
	return cloneEvents(s.events[:limit]), nil
}

func (s *Service) Record(level domain.LogLevel, scope string, action string, message string, fields map[string]string) {
	event := domain.LogEvent{
		ID:        domain.NewID("log"),
		Level:     level,
		Scope:     scope,
		Action:    action,
		Message:   message,
		Fields:    copyFields(fields),
		CreatedAt: s.now(),
	}
	if level != domain.LogDebug {
		writeStdout(event)
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	s.events = append([]domain.LogEvent{event}, s.events...)
	s.events = trimEvents(s.events)
	_ = s.persist()
}

func (s *Service) load() {
	file, err := os.Open(s.path)
	if err != nil {
		return
	}
	defer file.Close()

	var loaded []domain.LogEvent
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		var event domain.LogEvent
		if err := json.Unmarshal(scanner.Bytes(), &event); err == nil {
			loaded = append(loaded, event)
		}
	}
	if len(loaded) > maxEvents {
		loaded = loaded[len(loaded)-maxEvents:]
	}
	events := make([]domain.LogEvent, 0, len(loaded))
	for index := len(loaded) - 1; index >= 0; index -= 1 {
		event := loaded[index]
		event.Fields = copyFields(event.Fields)
		events = append(events, event)
	}
	s.events = events
	s.events = trimEvents(s.events)
}

func (s *Service) persist() error {
	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return err
	}
	file, err := os.OpenFile(s.path, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o644)
	if err != nil {
		return err
	}
	defer file.Close()

	chronological := make([]domain.LogEvent, len(s.events))
	for index := range s.events {
		chronological[index] = s.events[len(s.events)-1-index]
	}
	for _, event := range chronological {
		data, err := json.Marshal(event)
		if err != nil {
			return err
		}
		data = append(data, '\n')
		if _, err := file.Write(data); err != nil {
			return err
		}
	}
	return nil
}

func writeStdout(event domain.LogEvent) {
	data, err := json.Marshal(event)
	if err != nil {
		fmt.Printf("{\"level\":\"error\",\"scope\":\"logs\",\"action\":\"marshal\",\"message\":%q}\n", err.Error())
		return
	}
	fmt.Println(string(data))
}

func cloneEvents(events []domain.LogEvent) []domain.LogEvent {
	next := make([]domain.LogEvent, len(events))
	for index := range events {
		next[index] = events[index]
		next[index].Fields = copyFields(events[index].Fields)
	}
	return next
}

func copyFields(fields map[string]string) map[string]string {
	if len(fields) == 0 {
		return nil
	}
	next := make(map[string]string, len(fields))
	for key, value := range fields {
		next[key] = value
	}
	return next
}

func trimEvents(events []domain.LogEvent) []domain.LogEvent {
	trimmed := make([]domain.LogEvent, 0, minInt(maxEvents, len(events)))
	sessionCounts := map[string]int{}
	for _, event := range events {
		sessionID := event.Fields["sessionId"]
		if sessionID != "" {
			if sessionCounts[sessionID] >= maxSessionEvents {
				continue
			}
			sessionCounts[sessionID] += 1
		}
		trimmed = append(trimmed, event)
		if len(trimmed) == maxEvents {
			break
		}
	}
	sort.SliceStable(trimmed, func(left, right int) bool {
		return trimmed[left].CreatedAt.After(trimmed[right].CreatedAt)
	})
	return trimmed
}

func minInt(left int, right int) int {
	if left < right {
		return left
	}
	return right
}
