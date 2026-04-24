package doctor

import (
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

type Status string

const (
	StatusOK      Status = "ok"
	StatusWarning Status = "warning"
	StatusError   Status = "error"
)

type Check struct {
	Name     string `json:"name"`
	Status   Status `json:"status"`
	Detail   string `json:"detail"`
	Help     string `json:"help"`
	Required bool   `json:"required"`
}

type Report struct {
	Checks []Check `json:"checks"`
}

type CommandProbe interface {
	LookPath(name string) (string, error)
	Run(name string, args ...string) (string, error)
}

type AppProbe interface {
	Exists(path string) bool
}

type Service struct {
	commands CommandProbe
	apps     AppProbe
}

type OSCommandProbe struct{}

func (OSCommandProbe) LookPath(name string) (string, error) {
	return exec.LookPath(name)
}

func (OSCommandProbe) Run(name string, args ...string) (string, error) {
	output, err := exec.Command(name, args...).CombinedOutput()
	return strings.TrimSpace(string(output)), err
}

type OSAppProbe struct{}

func (OSAppProbe) Exists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func NewService() *Service {
	return &Service{
		commands: OSCommandProbe{},
		apps:     OSAppProbe{},
	}
}

func (s *Service) Report() Report {
	checks := []Check{
		s.probeCommand("git", []string{"--version"}, true, "Install Git and make sure it is available on your PATH."),
		s.probeCommand("tmux", []string{"-V"}, true, "Install tmux so mico can manage durable sessions."),
		s.probeCommand("codex", nil, false, "Install the Codex CLI and authenticate locally to use Codex inside mico."),
		s.probeCommand("claude", nil, false, "Install Claude Code and authenticate locally to use Claude inside mico."),
	}

	if runtime.GOOS == "darwin" {
		checks = append(checks, s.probeCommand("osascript", []string{"-e", "return \"ok\""}, false, "macOS notifications depend on osascript being available."))
		checks = append(checks, s.probeApp("iTerm", []string{
			"/Applications/iTerm.app",
			filepath.Join(userHomeDir(), "Applications", "iTerm.app"),
		}, false, "Install iTerm if you want external terminal integration later on."))
	}

	return Report{Checks: checks}
}

func (s *Service) probeCommand(name string, versionArgs []string, required bool, help string) Check {
	path, err := s.commands.LookPath(name)
	if err != nil {
		return Check{
			Name:     name,
			Status:   missingStatus(required),
			Detail:   "Not found on PATH.",
			Help:     help,
			Required: required,
		}
	}

	detail := path
	if len(versionArgs) > 0 {
		output, runErr := s.commands.Run(name, versionArgs...)
		if runErr == nil {
			if output != "" {
				detail = output
			}
		}
	}

	return Check{
		Name:     name,
		Status:   StatusOK,
		Detail:   detail,
		Help:     "Ready.",
		Required: required,
	}
}

func (s *Service) probeApp(name string, candidates []string, required bool, help string) Check {
	for _, candidate := range candidates {
		if candidate == "" {
			continue
		}
		if s.apps.Exists(candidate) {
			return Check{
				Name:     name,
				Status:   StatusOK,
				Detail:   candidate,
				Help:     "Ready.",
				Required: required,
			}
		}
	}

	return Check{
		Name:     name,
		Status:   missingStatus(required),
		Detail:   "Not installed in /Applications or ~/Applications.",
		Help:     help,
		Required: required,
	}
}

func missingStatus(required bool) Status {
	if required {
		return StatusError
	}
	return StatusWarning
}

func userHomeDir() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return home
}
