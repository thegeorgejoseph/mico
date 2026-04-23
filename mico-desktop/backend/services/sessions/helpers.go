package sessions

import (
	"os"
	"strings"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

func tmuxSessionName(worktreeID string, agent domain.AgentKind) string {
	clean := strings.NewReplacer("_", "-", "/", "-", ".", "-").Replace(worktreeID)
	return "mico-desktop-" + clean + "-" + string(agent)
}

func shouldSendStartupCommand(command []string) bool {
	if len(command) == 0 || command[0] == "" {
		return false
	}
	shell := getShell()
	return !(len(command) == 1 && command[0] == shell)
}

func getShell() string {
	shell := os.Getenv("SHELL")
	if shell == "" {
		shell = "/bin/zsh"
	}
	return shell
}
