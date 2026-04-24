# UI Package Suggestions

mico desktop should feel like a modern, native-adjacent developer tool without casually growing a huge dependency tree.

The current light package lock is a feature: it helps performance, keeps installs fast, and reduces the attack surface. Keep that posture by default.

For complex UI primitives, consider maintained packages that popular apps already rely on instead of rebuilding difficult behavior by hand.

## Principles

- Prefer zero or few new dependencies when custom code is simple and safe.
- Use proven behavior primitives when the interaction is complex, accessibility-heavy, or easy to get subtly wrong.
- Keep mico's visual styling local and consistent.
- Wrap third-party primitives in typed local components.
- Avoid dependency sprawl.
- Search vulnerability sources before installing any package.
- Run `npm audit --audit-level=moderate` after dependency changes.

## Suggested Candidates

Terminal:

- `@xterm/xterm`
- `@xterm/addon-fit`
- `@xterm/addon-web-links`

Layout:

- `react-resizable-panels`

Menus, dialogs, popovers, command surfaces:

- Radix UI primitives
- shadcn-compatible wrappers

Icons:

- `lucide-react`

Large lists:

- TanStack Virtual

Tables:

- TanStack Table

Async request state:

- TanStack Query, only once request invalidation/loading/error state becomes hard to manage manually

## Possible Targets

The most likely package-backed interface to add next is real terminal rendering with xterm.js. The current terminal panel can capture tmux output and send input, but it is not yet a real terminal emulator.

Resizable panes are another candidate if the app layout starts needing adjustable worktree, agent chat, terminal, and inspector regions.
