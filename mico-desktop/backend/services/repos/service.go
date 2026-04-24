package repos

import (
	"context"
	"errors"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Service struct {
	store         *state.Store
	runner        CommandRunner
	notifications *notifications.Service
	now           func() time.Time
}

func NewService(store *state.Store, runner CommandRunner, notifications *notifications.Service) *Service {
	return &Service{store: store, runner: runner, notifications: notifications, now: time.Now}
}

func NewServiceWithClock(store *state.Store, runner CommandRunner, notifications *notifications.Service, now func() time.Time) *Service {
	return &Service{store: store, runner: runner, notifications: notifications, now: now}
}

type AddRepoRequest struct {
	Path string `json:"path"`
	Name string `json:"name,omitempty"`
}

func (s *Service) List() ([]domain.Repo, error) {
	current, err := s.store.Load()
	if err != nil {
		return nil, err
	}
	return current.Repos, nil
}

func (s *Service) Add(ctx context.Context, request AddRepoRequest) (domain.Repo, error) {
	if strings.TrimSpace(request.Path) == "" {
		return domain.Repo{}, errors.New("repo path is required")
	}
	abs, err := filepath.Abs(request.Path)
	if err != nil {
		return domain.Repo{}, err
	}
	if _, err := s.runner.Run(ctx, abs, "git", "rev-parse", "--show-toplevel"); err != nil {
		return domain.Repo{}, errors.New("path is not a git repository")
	}

	name := strings.TrimSpace(request.Name)
	if name == "" {
		name = filepath.Base(abs)
	}
	repo := domain.Repo{
		ID:        domain.NewID("repo"),
		Name:      name,
		Path:      abs,
		CreatedAt: s.now(),
	}

	_, err = s.store.Update(func(next *domain.State) error {
		for _, existing := range next.Repos {
			if existing.Path == repo.Path {
				return errors.New("repo is already tracked")
			}
		}
		next.Repos = append(next.Repos, repo)
		return nil
	})
	if err != nil {
		return domain.Repo{}, err
	}
	_, _ = s.notifications.Push(domain.NotificationSuccess, "Repository added", repo.Name)
	return repo, nil
}

func (s *Service) Branches(ctx context.Context, repoID string) ([]string, error) {
	repo, err := s.Find(repoID)
	if err != nil {
		return nil, err
	}
	result, err := s.runner.Run(ctx, repo.Path, "git", "branch", "--all", "--format=%(refname:short)")
	if err != nil {
		return nil, err
	}
	seen := map[string]bool{}
	branches := make([]string, 0)
	for _, line := range strings.Split(result.Stdout, "\n") {
		line = strings.TrimSpace(line)
		line = strings.TrimPrefix(line, "remotes/")
		if strings.HasPrefix(line, "origin/HEAD") {
			continue
		}
		if strings.HasPrefix(line, "origin/") {
			line = strings.TrimPrefix(line, "origin/")
		}
		if line != "" && !seen[line] {
			seen[line] = true
			branches = append(branches, line)
		}
	}
	sort.Strings(branches)
	return branches, nil
}

func (s *Service) Refresh(ctx context.Context, repoID string) error {
	repo, err := s.Find(repoID)
	if err != nil {
		return err
	}
	if _, err := s.runner.Run(ctx, repo.Path, "git", "fetch", "--all", "--prune"); err != nil {
		return err
	}
	_, _ = s.notifications.Push(domain.NotificationInfo, "Repository refreshed", repo.Name)
	return nil
}

func (s *Service) Find(id string) (domain.Repo, error) {
	current, err := s.store.Load()
	if err != nil {
		return domain.Repo{}, err
	}
	for _, repo := range current.Repos {
		if repo.ID == id {
			return repo, nil
		}
	}
	return domain.Repo{}, errors.New("repo not found")
}
