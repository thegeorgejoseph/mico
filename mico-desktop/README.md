# mico desktop

`mico-desktop` is the desktop control plane for local AI coding work. Electron owns the shell, Go owns the backend state and system work, and React renders the UI.

## Local dev

```sh
make install
make dev
```

Useful supporting commands:

```sh
make test
make build
```

## Product shape

- Primary install path: `brew install --cask thegeorgejoseph/tap/mico-desktop`
- Fallback install path: latest signed GitHub Release DMG
- Update path today: in-app Settings opens the current signed release
- Backend responsibilities: repos, worktrees, sessions, logs, notifications, agent actions, and navigation state
- Renderer responsibilities: render backend state and forward user intent

## Docs

- [AGENTS.md](./AGENTS.md): concise coding guidance for future agents
- [BUILDING.md](./BUILDING.md): local build + release staging overview
- [docs/AGENT_UI.md](./docs/AGENT_UI.md): agent-mode intent and tool boundary
- [docs/OPERATIONS.md](./docs/OPERATIONS.md): install/update/release operating notes
- [docs/PRODUCTION_DISTRIBUTION.md](./docs/PRODUCTION_DISTRIBUTION.md): short release checklist
