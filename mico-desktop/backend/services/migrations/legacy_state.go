package migrations

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

const legacyStateMigrationID = "2026-04-22-import-legacy-mico-state"

type LegacyStateMigration struct {
	path string
}

func NewLegacyStateMigration(path string) LegacyStateMigration {
	return LegacyStateMigration{path: path}
}

func (m LegacyStateMigration) ID() string {
	return legacyStateMigrationID
}

func (m LegacyStateMigration) Run(current *domain.State) error {
	if m.path == "" {
		return nil
	}
	data, err := os.ReadFile(m.path)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}

	var legacy legacyState
	if err := json.Unmarshal(data, &legacy); err != nil {
		return err
	}

	reposByID := map[string]bool{}
	for _, repo := range current.Repos {
		reposByID[repo.ID] = true
	}
	for _, repo := range legacy.Repos {
		if reposByID[repo.ID] {
			continue
		}
		current.Repos = append(current.Repos, domain.Repo{
			ID:        repo.ID,
			Name:      firstNonEmpty(repo.DisplayName, filepath.Base(repo.Path)),
			Path:      repo.Path,
			CreatedAt: time.Unix(0, 0).UTC(),
		})
		reposByID[repo.ID] = true
	}

	worktreesByID := map[string]bool{}
	for _, worktree := range current.Worktrees {
		worktreesByID[worktree.ID] = true
	}
	sessionsByName := map[string]bool{}
	for _, session := range current.Sessions {
		sessionsByName[session.SessionName] = true
	}

	for _, workstream := range legacy.Workstreams {
		created := epoch(workstream.CreatedAtEpochSecs)
		if !worktreesByID[workstream.ID] {
			current.Worktrees = append(current.Worktrees, domain.Worktree{
				ID:        workstream.ID,
				RepoID:    workstream.RepoID,
				Branch:    workstream.Branch,
				Base:      firstNonEmpty(workstream.BaseBranch, workstream.Branch),
				Path:      workstream.WorktreePath,
				Status:    worktreeStatus(workstream.Status),
				CreatedAt: created,
				UpdatedAt: epoch(workstream.StatusChangedAtEpochSecs),
			})
			worktreesByID[workstream.ID] = true
		}

		for _, session := range workstream.Sessions {
			if session.SessionName == "" || sessionsByName[session.SessionName] {
				continue
			}
			agentKind := agent(session.AgentPreset)
			current.Sessions = append(current.Sessions, domain.Session{
				ID:          session.ID,
				WorktreeID:  workstream.ID,
				Agent:       agentKind,
				Command:     command(agentKind),
				SessionName: session.SessionName,
				Status:      sessionStatus(session.Status),
				CreatedAt:   epoch(session.CreatedAtEpochSecs),
				UpdatedAt:   epoch(session.StatusChangedAtEpochSecs),
			})
			sessionsByName[session.SessionName] = true
		}

		if len(workstream.Sessions) == 0 && workstream.SessionName != "" && !sessionsByName[workstream.SessionName] {
			agentKind := agent(workstream.AgentPreset)
			current.Sessions = append(current.Sessions, domain.Session{
				ID:          workstream.ID + "-session",
				WorktreeID:  workstream.ID,
				Agent:       agentKind,
				Command:     command(agentKind),
				SessionName: workstream.SessionName,
				Status:      sessionStatus(workstream.Status),
				CreatedAt:   created,
				UpdatedAt:   epoch(workstream.StatusChangedAtEpochSecs),
			})
			sessionsByName[workstream.SessionName] = true
		}
	}

	return nil
}

func DefaultLegacyStatePath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".mico", "state.json")
}

type legacyState struct {
	Repos       []legacyRepo       `json:"repos"`
	Workstreams []legacyWorkstream `json:"workstreams"`
}

type legacyRepo struct {
	ID          string `json:"id"`
	Path        string `json:"path"`
	DisplayName string `json:"display_name"`
}

type legacyWorkstream struct {
	ID                       string          `json:"id"`
	RepoID                   string          `json:"repo_id"`
	BaseBranch               string          `json:"base_branch"`
	Branch                   string          `json:"branch"`
	WorktreePath             string          `json:"worktree_path"`
	SessionName              string          `json:"session_name"`
	AgentPreset              string          `json:"agent_preset"`
	Status                   string          `json:"status"`
	CreatedAtEpochSecs       int64           `json:"created_at_epoch_secs"`
	StatusChangedAtEpochSecs int64           `json:"status_changed_at_epoch_secs"`
	Sessions                 []legacySession `json:"sessions"`
}

type legacySession struct {
	ID                       string `json:"id"`
	SessionName              string `json:"session_name"`
	AgentPreset              string `json:"agent_preset"`
	Status                   string `json:"status"`
	CreatedAtEpochSecs       int64  `json:"created_at_epoch_secs"`
	StatusChangedAtEpochSecs int64  `json:"status_changed_at_epoch_secs"`
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}

func epoch(seconds int64) time.Time {
	if seconds <= 0 {
		return time.Unix(0, 0).UTC()
	}
	return time.Unix(seconds, 0).UTC()
}

func worktreeStatus(raw string) domain.WorktreeStatus {
	if raw == "Stopped" || raw == "stopped" {
		return domain.WorktreeStopped
	}
	return domain.WorktreeRunning
}

func sessionStatus(raw string) domain.SessionStatus {
	if raw == "Stopped" || raw == "stopped" {
		return domain.SessionExited
	}
	return domain.SessionRunning
}

func agent(raw string) domain.AgentKind {
	switch raw {
	case string(domain.AgentCodex):
		return domain.AgentCodex
	case string(domain.AgentClaude):
		return domain.AgentClaude
	default:
		return domain.AgentTerminal
	}
}

func command(agent domain.AgentKind) []string {
	switch agent {
	case domain.AgentCodex:
		return []string{"codex"}
	case domain.AgentClaude:
		return []string{"claude"}
	default:
		shell := os.Getenv("SHELL")
		if shell == "" {
			shell = "/bin/zsh"
		}
		return []string{shell}
	}
}
