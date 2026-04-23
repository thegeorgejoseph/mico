package logs

import (
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func TestRecordPrependsLogEvent(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := NewService(store)

	service.Record(domain.LogInfo, "test", "first", "one", nil)
	service.Record(domain.LogError, "test", "second", "two", map[string]string{"key": "value"})

	logs, err := service.List(10)
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}
	if len(logs) != 2 {
		t.Fatalf("len(logs) = %d, want 2", len(logs))
	}
	if logs[0].Action != "second" || logs[0].Fields["key"] != "value" {
		t.Fatalf("logs[0] = %+v", logs[0])
	}
}

func TestRecordWritesNonDebugLogsToStdout(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := NewService(store)

	output := captureStdout(t, func() {
		service.Record(domain.LogWarn, "http", "POST /api/worktrees", "HTTP request completed.", map[string]string{"status": "400"})
	})

	if !strings.Contains(output, `"level":"warn"`) {
		t.Fatalf("stdout = %q, want warning log JSON", output)
	}
	if !strings.Contains(output, `"status":"400"`) {
		t.Fatalf("stdout = %q, want fields", output)
	}
}

func TestRecordDoesNotWriteDebugLogsToStdout(t *testing.T) {
	store := state.NewStore(filepath.Join(t.TempDir(), "state.json"))
	service := NewService(store)

	output := captureStdout(t, func() {
		service.Record(domain.LogDebug, "http", "GET /api/sessions/id/terminal", "HTTP request completed.", nil)
	})

	if output != "" {
		t.Fatalf("stdout = %q, want empty", output)
	}
}

func TestRecordDoesNotPersistLogsIntoStateFile(t *testing.T) {
	dir := t.TempDir()
	store := state.NewStore(filepath.Join(dir, "state.json"))
	service := NewService(store)

	if _, err := store.Load(); err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	before, err := os.ReadFile(filepath.Join(dir, "state.json"))
	if err != nil {
		t.Fatalf("ReadFile(before) error = %v", err)
	}

	service.Record(domain.LogInfo, "test", "action", "message", nil)

	after, err := os.ReadFile(filepath.Join(dir, "state.json"))
	if err != nil {
		t.Fatalf("ReadFile(after) error = %v", err)
	}
	if string(before) != string(after) {
		t.Fatalf("state.json changed after log record")
	}
}

func TestRecordPersistsEventsInLogFile(t *testing.T) {
	dir := t.TempDir()
	store := state.NewStore(filepath.Join(dir, "state.json"))
	service := NewService(store)

	service.Record(domain.LogInfo, "test", "first", "one", nil)
	service.Record(domain.LogError, "test", "second", "two", nil)

	reloaded := NewService(store)
	logs, err := reloaded.List(10)
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}
	if len(logs) != 2 {
		t.Fatalf("len(logs) = %d, want 2", len(logs))
	}
	if logs[0].Action != "second" || logs[1].Action != "first" {
		t.Fatalf("logs = %+v", logs)
	}
}

func captureStdout(t *testing.T, fn func()) string {
	t.Helper()
	reader, writer, err := os.Pipe()
	if err != nil {
		t.Fatalf("Pipe() error = %v", err)
	}
	oldStdout := os.Stdout
	os.Stdout = writer
	defer func() {
		os.Stdout = oldStdout
	}()
	fn()
	if err := writer.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	defer reader.Close()
	output, err := io.ReadAll(reader)
	if err != nil {
		t.Fatalf("ReadAll() error = %v", err)
	}
	return string(output)
}
