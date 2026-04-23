package navigation

import (
	"math"
	"sort"
	"strings"
)

type SearchResultKind string

const (
	SearchResultRepo     SearchResultKind = "repo"
	SearchResultWorktree SearchResultKind = "worktree"
)

type SearchResult struct {
	ID     string           `json:"id"`
	Kind   SearchResultKind `json:"kind"`
	Label  string           `json:"label"`
	Meta   string           `json:"meta"`
	RepoID string           `json:"repoId,omitempty"`
}

func (s *Service) Search(query string, limit int) ([]SearchResult, error) {
	trimmed := strings.TrimSpace(query)
	if trimmed == "" {
		return []SearchResult{}, nil
	}

	current, err := s.store.Load()
	if err != nil {
		return nil, err
	}

	repoNames := make(map[string]string, len(current.Repos))
	results := make([]scoredResult, 0, len(current.Repos)+len(current.Worktrees))
	for _, repo := range current.Repos {
		repoNames[repo.ID] = repo.Name
		score := maxScore(trimmed, repo.Name, repo.Path)
		if score > math.MinInt {
			results = append(results, scoredResult{
				SearchResult: SearchResult{
					ID:    repo.ID,
					Kind:  SearchResultRepo,
					Label: repo.Name,
					Meta:  repo.Path,
				},
				score: score,
			})
		}
	}

	for _, worktree := range current.Worktrees {
		repoName := repoNames[worktree.RepoID]
		score := maxScore(trimmed, worktree.Branch, worktree.Path, repoName+" "+worktree.Branch, worktree.Base)
		if score > math.MinInt {
			results = append(results, scoredResult{
				SearchResult: SearchResult{
					ID:     worktree.ID,
					Kind:   SearchResultWorktree,
					Label:  worktree.Branch,
					Meta:   firstNonEmpty(repoName, "Project") + " · " + worktree.Path,
					RepoID: worktree.RepoID,
				},
				score: score,
			})
		}
	}

	sort.Slice(results, func(left, right int) bool {
		if results[left].score != results[right].score {
			return results[left].score > results[right].score
		}
		return results[left].Label < results[right].Label
	})

	if limit <= 0 || limit > len(results) {
		limit = len(results)
	}
	next := make([]SearchResult, 0, limit)
	for _, result := range results[:limit] {
		next = append(next, result.SearchResult)
	}
	return next, nil
}

type scoredResult struct {
	SearchResult
	score int
}

func maxScore(query string, candidates ...string) int {
	best := math.MinInt
	for _, candidate := range candidates {
		if score := fuzzyScore(query, candidate); score > best {
			best = score
		}
	}
	return best
}

func fuzzyScore(query string, candidate string) int {
	q := strings.ToLower(strings.TrimSpace(query))
	c := strings.ToLower(strings.TrimSpace(candidate))
	if q == "" || c == "" {
		return math.MinInt
	}
	if c == q {
		return 1000
	}
	if strings.HasPrefix(c, q) {
		return 700 - maxInt(0, len(c)-len(q))
	}
	if index := strings.Index(c, q); index >= 0 {
		return 500 - index
	}

	score := 0
	position := -1
	for _, char := range q {
		next := strings.IndexRune(c[position+1:], char)
		if next == -1 {
			return math.MinInt
		}
		actual := position + 1 + next
		if position >= 0 && actual == position+1 {
			score += 16
		} else {
			score += 7
		}
		position = actual
	}
	return score - maxInt(0, len(c)-len(q))
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return value
		}
	}
	return ""
}

func maxInt(left int, right int) int {
	if left > right {
		return left
	}
	return right
}
