package agent

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"strings"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Provider interface {
	Run(ctx context.Context, prompt string) (string, error)
}

type Resolver interface {
	Resolve(kind domain.AgentKind) (Provider, error)
}

type CommandExecutor interface {
	CombinedOutput(ctx context.Context, command []string) (string, error)
}

type ExecCommandExecutor struct{}

func (ExecCommandExecutor) CombinedOutput(ctx context.Context, command []string) (string, error) {
	if len(command) == 0 {
		return "", errors.New("command is required")
	}
	cmd := exec.CommandContext(ctx, command[0], command[1:]...)
	output, err := cmd.CombinedOutput()
	return string(output), err
}

type StaticResolver struct {
	providers map[domain.AgentKind]Provider
}

func NewResolver(executor CommandExecutor) *StaticResolver {
	return &StaticResolver{
		providers: map[domain.AgentKind]Provider{
			domain.AgentCodex:  CodexProvider{executor: executor},
			domain.AgentClaude: ClaudeProvider{executor: executor},
		},
	}
}

func NewResolverWithProviders(providers map[domain.AgentKind]Provider) *StaticResolver {
	copyOfProviders := make(map[domain.AgentKind]Provider, len(providers))
	for kind, provider := range providers {
		copyOfProviders[kind] = provider
	}
	return &StaticResolver{providers: copyOfProviders}
}

func (r *StaticResolver) Resolve(kind domain.AgentKind) (Provider, error) {
	provider, ok := r.providers[kind]
	if !ok {
		return nil, errors.New("choose codex or claude for agent mode")
	}
	return provider, nil
}

type CodexProvider struct {
	executor CommandExecutor
}

func (p CodexProvider) Run(ctx context.Context, prompt string) (string, error) {
	schemaPath, cleanup, err := writeOutputSchema()
	if err != nil {
		return "", err
	}
	defer cleanup()

	outputPath, outputCleanup, err := createOutputFile("mico-agent-codex-*.json")
	if err != nil {
		return "", err
	}
	defer outputCleanup()

	command := []string{
		"codex",
		"exec",
		"--skip-git-repo-check",
		"--output-schema",
		schemaPath,
		"--output-last-message",
		outputPath,
		prompt,
	}
	output, runErr := p.executor.CombinedOutput(ctx, command)
	lastMessage, readErr := os.ReadFile(outputPath)
	if readErr == nil && strings.TrimSpace(string(lastMessage)) != "" {
		if runErr != nil {
			return string(lastMessage), runErr
		}
		return string(lastMessage), nil
	}
	return output, runErr
}

type ClaudeProvider struct {
	executor CommandExecutor
}

func (p ClaudeProvider) Run(ctx context.Context, prompt string) (string, error) {
	schemaJSON, err := outputSchema()
	if err != nil {
		return "", err
	}
	command := []string{
		"claude",
		"-p",
		"--json-schema",
		schemaJSON,
		prompt,
	}
	return p.executor.CombinedOutput(ctx, command)
}
