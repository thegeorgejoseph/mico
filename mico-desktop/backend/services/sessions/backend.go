package sessions

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"strconv"
	"strings"
)

type ProcessStarter interface {
	Start(ctx context.Context, dir string, command []string) error
}

type TerminalBackend interface {
	Capture(sessionName string, lines int) ([]string, error)
	Has(sessionName string) bool
	Send(sessionName string, text string) error
	Start(ctx context.Context, sessionName string, dir string, command []string) error
	Stop(sessionName string) error
}

type ExecStarter struct{}

func (ExecStarter) Start(ctx context.Context, dir string, command []string) error {
	if len(command) == 0 {
		return errors.New("command is required")
	}
	cmd := exec.CommandContext(ctx, command[0], command[1:]...)
	cmd.Dir = dir
	cmd.Env = os.Environ()
	if err := cmd.Start(); err != nil {
		return err
	}
	go func() {
		_ = cmd.Wait()
	}()
	return nil
}

type TmuxBackend struct{}

func (TmuxBackend) Start(ctx context.Context, sessionName string, dir string, command []string) error {
	backend := TmuxBackend{}
	if sessionName == "" {
		return errors.New("session name is required")
	}
	if len(command) == 0 {
		return errors.New("command is required")
	}
	if backend.Has(sessionName) {
		return nil
	}
	if err := exec.CommandContext(ctx, "tmux", "new-session", "-d", "-e", "DISABLE_AUTO_UPDATE=true", "-s", sessionName, "-c", dir).Run(); err != nil {
		return err
	}
	if shouldSendStartupCommand(command) {
		return exec.CommandContext(ctx, "tmux", "send-keys", "-t", sessionName, strings.Join(command, " "), "C-m").Run()
	}
	return nil
}

func (TmuxBackend) Has(sessionName string) bool {
	return exec.Command("tmux", "has-session", "-t", sessionName).Run() == nil
}

func (TmuxBackend) Capture(sessionName string, lines int) ([]string, error) {
	backend := TmuxBackend{}
	if lines <= 0 {
		lines = 120
	}
	if !backend.Has(sessionName) {
		return []string{"session is not running"}, nil
	}
	output, err := exec.Command("tmux", "capture-pane", "-p", "-J", "-S", "-"+strconv.Itoa(lines), "-t", sessionName).Output()
	if err != nil {
		return nil, err
	}
	return strings.Split(strings.TrimRight(string(output), "\n"), "\n"), nil
}

func (TmuxBackend) Send(sessionName string, text string) error {
	backend := TmuxBackend{}
	if strings.TrimSpace(text) == "" {
		return nil
	}
	if !backend.Has(sessionName) {
		return errors.New("session is not running")
	}
	if err := exec.Command("tmux", "send-keys", "-t", sessionName, "-l", text).Run(); err != nil {
		return err
	}
	return exec.Command("tmux", "send-keys", "-t", sessionName, "Enter").Run()
}

func (TmuxBackend) Stop(sessionName string) error {
	backend := TmuxBackend{}
	if sessionName == "" {
		return errors.New("session name is required")
	}
	if !backend.Has(sessionName) {
		return nil
	}
	return exec.Command("tmux", "kill-session", "-t", sessionName).Run()
}
