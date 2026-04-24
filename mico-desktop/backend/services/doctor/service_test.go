package doctor

import "testing"

func TestReportIncludesCoreChecks(t *testing.T) {
	report := NewService().Report()
	if len(report.Checks) < 4 {
		t.Fatalf("checks = %+v", report.Checks)
	}

	var foundGit bool
	var foundTmux bool
	for _, check := range report.Checks {
		if check.Name == "git" {
			foundGit = true
			if !check.Required {
				t.Fatalf("git should be required: %+v", check)
			}
		}
		if check.Name == "tmux" {
			foundTmux = true
			if !check.Required {
				t.Fatalf("tmux should be required: %+v", check)
			}
		}
		if check.Status == "" {
			t.Fatalf("missing status for check: %+v", check)
		}
		if check.Help == "" {
			t.Fatalf("missing help for check: %+v", check)
		}
	}

	if !foundGit || !foundTmux {
		t.Fatalf("core checks missing: %+v", report.Checks)
	}
}
