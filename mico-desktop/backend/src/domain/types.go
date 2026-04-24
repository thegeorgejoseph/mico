package domain

import "time"

type AgentKind string

const (
	AgentTerminal AgentKind = "terminal"
	AgentCodex    AgentKind = "codex"
	AgentClaude   AgentKind = "claude"
)

func (kind AgentKind) Valid() bool {
	switch kind {
	case AgentTerminal, AgentCodex, AgentClaude:
		return true
	default:
		return false
	}
}

type Repo struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	Path      string    `json:"path"`
	CreatedAt time.Time `json:"createdAt"`
}

type WorktreeStatus string

const (
	WorktreeReady   WorktreeStatus = "ready"
	WorktreeRunning WorktreeStatus = "running"
	WorktreeStopped WorktreeStatus = "stopped"
)

type Worktree struct {
	ID        string         `json:"id"`
	RepoID    string         `json:"repoId"`
	Branch    string         `json:"branch"`
	Base      string         `json:"base"`
	Path      string         `json:"path"`
	Status    WorktreeStatus `json:"status"`
	CreatedAt time.Time      `json:"createdAt"`
	UpdatedAt time.Time      `json:"updatedAt"`
}

type SessionStatus string

const (
	SessionRunning SessionStatus = "running"
	SessionExited  SessionStatus = "exited"
	SessionFailed  SessionStatus = "failed"
)

type Session struct {
	ID          string        `json:"id"`
	WorktreeID  string        `json:"worktreeId"`
	Agent       AgentKind     `json:"agent"`
	Command     []string      `json:"command"`
	SessionName string        `json:"sessionName"`
	Status      SessionStatus `json:"status"`
	CreatedAt   time.Time     `json:"createdAt"`
	UpdatedAt   time.Time     `json:"updatedAt"`
	ExitCode    *int          `json:"exitCode,omitempty"`
}

type NotificationLevel string

const (
	NotificationInfo    NotificationLevel = "info"
	NotificationSuccess NotificationLevel = "success"
	NotificationWarning NotificationLevel = "warning"
	NotificationError   NotificationLevel = "error"
)

type Notification struct {
	ID        string            `json:"id"`
	Level     NotificationLevel `json:"level"`
	Title     string            `json:"title"`
	Body      string            `json:"body"`
	Seen      bool              `json:"seen"`
	CreatedAt time.Time         `json:"createdAt"`
}

type WorkspaceFocus struct {
	RepoID     string `json:"repoId"`
	WorktreeID string `json:"worktreeId"`
	SessionID  string `json:"sessionId"`
	Mode       UIMode `json:"mode"`
}

type UISelection = WorkspaceFocus

type AppliedMigration struct {
	ID        string    `json:"id"`
	AppliedAt time.Time `json:"appliedAt"`
}

type State struct {
	Version       int                `json:"version"`
	Repos         []Repo             `json:"repos"`
	Worktrees     []Worktree         `json:"worktrees"`
	Sessions      []Session          `json:"sessions"`
	Notifications []Notification     `json:"notifications"`
	Selection     WorkspaceFocus     `json:"selection"`
	Logs          []LogEvent         `json:"logs"`
	Migrations    []AppliedMigration `json:"migrations,omitempty"`
}

type UIMode string

const (
	UIModeEffort UIMode = "effort"
	UIModeAgent  UIMode = "agent"
)

type LogLevel string

const (
	LogDebug LogLevel = "debug"
	LogInfo  LogLevel = "info"
	LogWarn  LogLevel = "warn"
	LogError LogLevel = "error"
)

type LogEvent struct {
	ID        string            `json:"id"`
	Level     LogLevel          `json:"level"`
	Scope     string            `json:"scope"`
	Action    string            `json:"action"`
	Message   string            `json:"message"`
	Fields    map[string]string `json:"fields,omitempty"`
	CreatedAt time.Time         `json:"createdAt"`
}
