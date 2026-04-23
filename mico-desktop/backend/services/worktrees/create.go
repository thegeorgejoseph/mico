package worktrees

import (
	"context"
	"errors"
	"path/filepath"
	"strings"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type CreateRequest struct {
	RepoID   string `json:"repoId"`
	Branch   string `json:"branch"`
	Base     string `json:"base"`
	Existing bool   `json:"existing,omitempty"`
}

func (s *Service) Create(ctx context.Context, request CreateRequest) (domain.Worktree, error) {
	if strings.TrimSpace(request.RepoID) == "" {
		return domain.Worktree{}, errors.New("repo id is required")
	}
	if strings.TrimSpace(request.Branch) == "" {
		return domain.Worktree{}, errors.New("branch is required")
	}
	if !request.Existing && strings.TrimSpace(request.Base) == "" {
		return domain.Worktree{}, errors.New("base is required")
	}

	repo, err := s.repos.Find(request.RepoID)
	if err != nil {
		return domain.Worktree{}, err
	}

	worktreePath := filepath.Join(s.root, slug(repo.Name), slug(request.Branch))
	unlock := s.lockKey(repo.ID + "::" + request.Branch)
	defer unlock()

	current, err := s.store.Load()
	if err != nil {
		return domain.Worktree{}, err
	}
	for _, existing := range current.Worktrees {
		if existing.Path == worktreePath {
			return domain.Worktree{}, errors.New("worktree is already tracked")
		}
	}
	if request.Existing {
		if err := s.addExistingBranchWorktree(ctx, repo.Path, request.Branch, worktreePath); err != nil {
			return domain.Worktree{}, err
		}
	} else {
		if _, err := s.runner.Run(ctx, repo.Path, "git", "worktree", "add", "-b", request.Branch, worktreePath, request.Base); err != nil {
			return domain.Worktree{}, err
		}
	}

	now := s.now()
	worktree := domain.Worktree{
		ID:        domain.NewID("wt"),
		RepoID:    repo.ID,
		Branch:    request.Branch,
		Base:      request.Base,
		Path:      worktreePath,
		Status:    domain.WorktreeReady,
		CreatedAt: now,
		UpdatedAt: now,
	}

	_, err = s.store.Update(func(next *domain.State) error {
		for _, existing := range next.Worktrees {
			if existing.Path == worktree.Path {
				return errors.New("worktree is already tracked")
			}
		}
		next.Worktrees = append(next.Worktrees, worktree)
		return nil
	})
	if err != nil {
		return domain.Worktree{}, err
	}
	_, _ = s.notifications.Push(domain.NotificationSuccess, "Worktree created", worktree.Branch)
	return worktree, nil
}

func (s *Service) addExistingBranchWorktree(ctx context.Context, repoPath string, branch string, worktreePath string) error {
	if _, err := s.runner.Run(ctx, repoPath, "git", "show-ref", "--verify", "--quiet", "refs/heads/"+branch); err == nil {
		_, err = s.runner.Run(ctx, repoPath, "git", "worktree", "add", worktreePath, branch)
		return err
	}
	if _, err := s.runner.Run(ctx, repoPath, "git", "show-ref", "--verify", "--quiet", "refs/remotes/origin/"+branch); err == nil {
		_, err = s.runner.Run(ctx, repoPath, "git", "worktree", "add", "--track", "-b", branch, worktreePath, "origin/"+branch)
		return err
	}
	return errors.New("branch not found locally or on origin")
}
