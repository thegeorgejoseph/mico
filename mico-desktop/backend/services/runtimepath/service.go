package runtimepath

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

type shellPathLoader func(shell string) (string, error)

type Service struct {
	loadShellPath shellPathLoader
}

func NewService() *Service {
	return &Service{loadShellPath: loginShellPath}
}

func (s *Service) Configure() error {
	merged := mergePathLists(
		strings.Split(strings.TrimSpace(os.Getenv("PATH")), string(os.PathListSeparator)),
		s.platformCandidates(),
		s.loginShellCandidates(),
	)
	if len(merged) == 0 {
		return nil
	}
	return os.Setenv("PATH", strings.Join(merged, string(os.PathListSeparator)))
}

func (s *Service) platformCandidates() []string {
	home, err := os.UserHomeDir()
	if err != nil {
		home = ""
	}

	candidates := []string{}
	if runtime.GOOS == "darwin" {
		candidates = append(candidates,
			"/opt/homebrew/bin",
			"/opt/homebrew/sbin",
			"/usr/local/bin",
			"/usr/local/sbin",
			"/Library/Apple/usr/bin",
			"/usr/bin",
			"/bin",
			"/usr/sbin",
			"/sbin",
		)
	}
	if home != "" {
		candidates = append(candidates,
			filepath.Join(home, ".cargo", "bin"),
			filepath.Join(home, ".bun", "bin"),
			filepath.Join(home, ".local", "bin"),
			filepath.Join(home, ".opencode", "bin"),
		)
	}
	return candidates
}

func (s *Service) loginShellCandidates() []string {
	shell := strings.TrimSpace(os.Getenv("SHELL"))
	if shell == "" {
		if runtime.GOOS == "darwin" {
			shell = "/bin/zsh"
		} else {
			shell = "/bin/sh"
		}
	}
	pathValue, err := s.loadShellPath(shell)
	if err != nil {
		return nil
	}
	return strings.Split(strings.TrimSpace(pathValue), string(os.PathListSeparator))
}

func loginShellPath(shell string) (string, error) {
	output, err := exec.Command(shell, "-l", "-c", "printf %s \"$PATH\"").Output()
	if err != nil {
		return "", err
	}
	return string(output), nil
}

func mergePathLists(groups ...[]string) []string {
	seen := map[string]struct{}{}
	merged := []string{}
	for _, group := range groups {
		for _, entry := range group {
			trimmed := strings.TrimSpace(entry)
			if trimmed == "" {
				continue
			}
			if _, ok := seen[trimmed]; ok {
				continue
			}
			seen[trimmed] = struct{}{}
			merged = append(merged, trimmed)
		}
	}
	return merged
}
