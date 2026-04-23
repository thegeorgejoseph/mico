package agent

import (
	"errors"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type ToolName string

const (
	ToolSelectRepo     ToolName = "select_repo"
	ToolSelectWorktree ToolName = "select_worktree"
	ToolSelectSession  ToolName = "select_session"
	ToolListRepos      ToolName = "list_repos"
	ToolListWorktrees  ToolName = "list_worktrees"
	ToolListSessions   ToolName = "list_sessions"
)

type RunRequest struct {
	Provider domain.AgentKind `json:"provider"`
	Message  string           `json:"message"`
}

type RunResponse struct {
	Provider domain.AgentKind       `json:"provider"`
	Message  string                 `json:"message"`
	ToolCall *ToolCall              `json:"toolCall,omitempty"`
	Result   *ToolResult            `json:"result,omitempty"`
	State    *domain.WorkspaceFocus `json:"selection,omitempty"`
}

type ToolCall struct {
	Tool           ToolName            `json:"tool"`
	Reason         string              `json:"reason"`
	SelectRepo     *SelectRepoTool     `json:"selectRepo,omitempty"`
	SelectWorktree *SelectWorktreeTool `json:"selectWorktree,omitempty"`
	SelectSession  *SelectSessionTool  `json:"selectSession,omitempty"`
	ListRepos      *ListReposTool      `json:"listRepos,omitempty"`
	ListWorktrees  *ListWorktreesTool  `json:"listWorktrees,omitempty"`
	ListSessions   *ListSessionsTool   `json:"listSessions,omitempty"`
}

type SelectRepoTool struct {
	RepoID string `json:"repoId"`
}

type SelectWorktreeTool struct {
	WorktreeID string `json:"worktreeId"`
}

type SelectSessionTool struct {
	SessionID string `json:"sessionId"`
}

type ListReposTool struct{}

type ListWorktreesTool struct{}

type ListSessionsTool struct{}

type ToolResult struct {
	OK      bool   `json:"ok"`
	Message string `json:"message"`
}

func (call ToolCall) Validate() error {
	switch call.Tool {
	case ToolSelectRepo:
		if call.SelectRepo == nil || call.SelectRepo.RepoID == "" {
			return errors.New("select_repo requires selectRepo.repoId")
		}
		return call.requireNoOtherPayloads(call.SelectRepo)
	case ToolSelectWorktree:
		if call.SelectWorktree == nil || call.SelectWorktree.WorktreeID == "" {
			return errors.New("select_worktree requires selectWorktree.worktreeId")
		}
		return call.requireNoOtherPayloads(call.SelectWorktree)
	case ToolSelectSession:
		if call.SelectSession == nil || call.SelectSession.SessionID == "" {
			return errors.New("select_session requires selectSession.sessionId")
		}
		return call.requireNoOtherPayloads(call.SelectSession)
	case ToolListRepos:
		if call.ListRepos == nil {
			return errors.New("list_repos requires listRepos")
		}
		return call.requireNoOtherPayloads(call.ListRepos)
	case ToolListWorktrees:
		if call.ListWorktrees == nil {
			return errors.New("list_worktrees requires listWorktrees")
		}
		return call.requireNoOtherPayloads(call.ListWorktrees)
	case ToolListSessions:
		if call.ListSessions == nil {
			return errors.New("list_sessions requires listSessions")
		}
		return call.requireNoOtherPayloads(call.ListSessions)
	default:
		return errors.New("unsupported tool")
	}
}

func (call ToolCall) requireNoOtherPayloads(allowed any) error {
	if call.SelectRepo != nil && allowed != call.SelectRepo {
		return errors.New("tool payloads must stay isolated")
	}
	if call.SelectWorktree != nil && allowed != call.SelectWorktree {
		return errors.New("tool payloads must stay isolated")
	}
	if call.SelectSession != nil && allowed != call.SelectSession {
		return errors.New("tool payloads must stay isolated")
	}
	if call.ListRepos != nil && allowed != call.ListRepos {
		return errors.New("tool payloads must stay isolated")
	}
	if call.ListWorktrees != nil && allowed != call.ListWorktrees {
		return errors.New("tool payloads must stay isolated")
	}
	if call.ListSessions != nil && allowed != call.ListSessions {
		return errors.New("tool payloads must stay isolated")
	}
	return nil
}
