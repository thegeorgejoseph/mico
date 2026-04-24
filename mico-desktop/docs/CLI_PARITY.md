# Rust CLI Parity

mico desktop should be a different interface over the same workstream model, not a separate product universe.

## Now

Desktop now imports existing Rust CLI state from:

```text
~/.mico/state.json
```

Imported data:

- repos
- workstreams as worktrees
- workstream sessions
- tmux session names
- running/stopped status where represented in state

Desktop stores its own UI state in:

```text
~/.mico-desktop/state.json
```

That UI state includes current Effort/Agent mode and selected repo/worktree/session.

## Existing Parity

- list tracked repos
- add repo
- list branches
- fetch repo / refresh refs
- create new branch-backed worktree
- create worktree from an existing branch
- list worktrees
- start terminal/Codex/Claude session
- stop workstream/session
- resume/recreate missing tmux sessions
- persist session metadata
- reuse durable tmux session names
- capture tmux output
- send terminal input
- notifications
- dependency doctor
- provider auth/setup checks

## Missing Parity

- configure upstream push target for new branches
- remove workstream and optionally remove managed worktree
- open in external terminal/iTerm
- attach in current terminal
- open repo/worktree in editor
- one-off agent runs
- workstream branch reconciliation
- attention events from task completion, idle output, failures, branch changes
- operation log/status feed
- dependency doctor
- provider auth/setup checks
- worktree ownership distinction between managed and external
- multi-session ordering and preferred-session behavior matching Rust CLI

## Plan

1. Make Desktop read and reconcile Rust CLI state on startup.
2. Add repo refresh and branch fetch.
3. Add existing-branch worktree creation.
4. Add stop/remove/resume session actions.
5. Add provider auth checks for Codex and Claude.
6. Add operation log and attention events.
7. Add one-off agent runs.
8. Decide whether Desktop writes back to `~/.mico/state.json` directly or uses a shared state adapter/migration.

The near-term behavior should be compatible import plus Desktop-owned UI state. Direct shared writes need more care because both the Rust CLI and Go backend could mutate state.
