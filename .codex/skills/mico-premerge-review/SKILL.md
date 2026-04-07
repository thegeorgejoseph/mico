---
name: mico-premerge-review
description: Review the mico codebase before merging to main. Use this when the user wants a security review, Rust best-practices audit, type-safety review, merge-readiness check, or a high-signal pass over changed code in this repository.
---

# Mico Premerge Review

## Overview

Use this skill for pre-merge review work in `mico`. It focuses on concrete bugs, security and integrity risks, Rust type-safety issues, and regressions in the git/tmux/TUI workflow.

## Workflow

1. Start with automated checks.
Run `./.codex/skills/mico-premerge-review/scripts/premerge_checks.sh`.

2. Read the changed files and the risky modules.
Prioritize:
- `src/app/runtime.rs`
- `src/infra/git.rs`
- `src/infra/tmux.rs`
- `src/infra/iterm.rs`
- `src/infra/json_store.rs`
- `src/tui/mod.rs`
- `xtask/src/main.rs`

3. Review with findings first.
Do not lead with a summary. List concrete findings ordered by severity, with file references and short explanations of impact.
For each finding, include at least one potential solution. When there is more than one reasonable fix, list the main options and give short pros/cons so the user can choose an approach quickly.

4. Focus on mico-specific failure modes.
Look especially for:
- state/config corruption or unsafe persistence under `~/.mico`
- git worktree ownership mistakes, especially linked vs managed checkouts
- branch drift between recorded metadata and actual `HEAD`
- tmux/iTerm command construction and quoting
- PATH-dependent command execution
- TUI logic that can desync selection, views, or stored status
- lint/type-safety regressions, especially anything that breaks `cargo clippy`

5. Call out residual risk after findings.
If no findings are present, say that explicitly and mention remaining gaps such as missing tests, missing migration coverage, or manual-only flows.

## Mico Heuristics

- Treat `cargo clippy` failures as real review signals, not style noise. This repo already opts into strict Clippy expectations.
- Prefer state-integrity findings over cosmetic TUI feedback.
- Be suspicious of direct filesystem writes to persistent state, non-atomic updates, and best-effort cleanup paths.
- When a workstream points at an existing checkout, verify that remove/reconcile/open flows cannot damage the original repo checkout.
- When reviewing TUI changes, ask whether the label teaches the feature. If the user would need hidden knowledge to understand a panel, call that out.

## Output Style

- Findings first, ordered by severity.
- Use clickable file references.
- After each finding, include `Potential solutions:` with 1-3 practical options.
- For each option, include short `Pros:` and `Cons:` lines focused on implementation risk, UX impact, and long-term maintainability.
- Keep summaries short.
- Mention commands you ran when relevant.

## Resource

### scripts/

- `premerge_checks.sh`: runs the standard automated checks for this repo before manual review.
