# mico desktop agent notes

## What this package is

`mico-desktop` is a desktop control plane for local AI coding workflows. The Go backend owns state and system behavior. The Electron/React renderer should stay thin and render that backend state.

## Architecture guardrails

- Prefer the standard library unless a dependency clearly earns its place.
- Keep transport, feature logic, and persistence separate.
- Add types instead of widening contracts with `unknown`, `any`, or ad hoc maps.
- Prefer one feature per folder or file cluster with a clear owner.
- Keep backend migrations explicit and one-time.

## Frontend

- Use typed React components and backend-owned contracts.
- Keep styling component-adjacent when practical and share only tokens/utilities globally.
- Avoid renderer-owned business logic for repos, worktrees, sessions, notifications, and agent actions.

## Backend

- Split routers from handlers and keep services small.
- Inject dependencies instead of reaching through global state.
- Keep notification, log, and navigation behavior deterministic and testable.

## Product direction

- Install story: Homebrew cask first, signed GitHub release second.
- Update story today: Settings opens the latest signed release.
- Agent story: local Codex/Claude CLIs reason over typed mico tool contracts; they do not scrape the UI.
