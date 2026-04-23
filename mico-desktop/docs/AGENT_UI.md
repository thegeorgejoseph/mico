# Agent UI

mico desktop has two modes:

- Effort Mode: direct UI control for repos, worktrees, sessions, and terminals
- Agent Mode: local Codex or Claude reasons over typed mico tools

## Rules

- Providers reason; mico executes.
- Providers do not scrape the UI.
- Tool contracts stay typed and narrow.
- State changes stay visible in the UI and activity log.

## Current tool shape

The current agent surface focuses on navigation first:

- list repos
- list worktrees
- list sessions
- select repo
- select worktree
- select session

That keeps the first iteration deterministic and easy to audit.
