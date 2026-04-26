package runtimepath

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestConfigureMergesExistingPathCandidatesAndLoginShellPath(t *testing.T) {
	t.Setenv("PATH", "/usr/bin:/bin")
	t.Setenv("SHELL", "/bin/zsh")

	home := tempHome(t)
	t.Setenv("HOME", home)

	service := &Service{
		loadShellPath: func(string) (string, error) {
			return "/opt/homebrew/bin:/Users/example/.cargo/bin:/usr/bin", nil
		},
	}

	if err := service.Configure(); err != nil {
		t.Fatalf("Configure() error = %v", err)
	}

	got := strings.Split(os.Getenv("PATH"), string(os.PathListSeparator))
	for _, want := range []string{
		"/usr/bin",
		"/bin",
		"/opt/homebrew/bin",
		filepath.Join(home, ".cargo", "bin"),
		filepath.Join(home, ".bun", "bin"),
	} {
		if !contains(got, want) {
			t.Fatalf("PATH = %q, missing %q", os.Getenv("PATH"), want)
		}
	}
	if count(got, "/usr/bin") != 1 {
		t.Fatalf("PATH = %q, expected /usr/bin once", os.Getenv("PATH"))
	}
}

func TestMergePathListsPreservesOrderAndDeduplicates(t *testing.T) {
	got := mergePathLists(
		[]string{"/usr/bin", "", "/bin"},
		[]string{"/usr/bin", "/opt/homebrew/bin"},
		[]string{" /bin ", "/Users/george/.cargo/bin"},
	)
	want := []string{"/usr/bin", "/bin", "/opt/homebrew/bin", "/Users/george/.cargo/bin"}
	if strings.Join(got, "|") != strings.Join(want, "|") {
		t.Fatalf("mergePathLists() = %v, want %v", got, want)
	}
}

func tempHome(t *testing.T) string {
	t.Helper()
	home := t.TempDir()
	if runtime.GOOS == "windows" {
		t.Skip("path handling test expects unix-style separators")
	}
	return home
}

func contains(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func count(values []string, target string) int {
	total := 0
	for _, value := range values {
		if value == target {
			total++
		}
	}
	return total
}
