package agent

import (
	"bytes"
	"encoding/json"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func buildPrompt(message string, current domain.State) (string, error) {
	var context bytes.Buffer
	encoder := json.NewEncoder(&context)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(struct {
		Selection domain.WorkspaceFocus `json:"selection"`
		Repos     []domain.Repo         `json:"repos"`
		Worktrees []domain.Worktree     `json:"worktrees"`
		Sessions  []domain.Session      `json:"sessions"`
	}{
		Selection: current.Selection,
		Repos:     current.Repos,
		Worktrees: current.Worktrees,
		Sessions:  current.Sessions,
	}); err != nil {
		return "", err
	}

	return `You are controlling mico desktop through typed actions.
Return exactly one action block and no prose outside the block.

Action block format:
<MICO_ACTION>{"tool":"select_worktree","reason":"why this worktree matches","selectWorktree":{"worktreeId":"..."}}</MICO_ACTION>

Available tools:
- {"tool":"select_repo","reason":"why this repo matches","selectRepo":{"repoId":"..."}}
- {"tool":"select_worktree","reason":"why this worktree matches","selectWorktree":{"worktreeId":"..."}}
- {"tool":"select_session","reason":"why this session matches","selectSession":{"sessionId":"..."}}
- {"tool":"list_repos","reason":"when repo context is missing","listRepos":{}}
- {"tool":"list_worktrees","reason":"when no worktree is a safe match yet","listWorktrees":{}}
- {"tool":"list_sessions","reason":"when session context is missing","listSessions":{}}

Rules:
- Choose the single best matching action.
- Keep tool payloads isolated. Only send the payload that matches the selected tool.
- Prefer exact branch and worktree name matches.
- If the user asks to switch, select the worktree or session directly instead of explaining.
- If you cannot identify a safe target, use one of the list_* tools.

Current mico state:
` + context.String() + `
User request:
` + message, nil
}
