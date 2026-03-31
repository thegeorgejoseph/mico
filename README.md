# mico

```text
                              
 _ __ ___  _  ___ ___         
| '_ ` _ \| |/ __/ _ \        
| | | | | | | (_| (_) |       
|_| |_| |_|_|\___\___/        
                              
```

get shit done. opinionated. reliable. fast.

Built with Rust, with safety and speed in mind.

`mico` is a local-first Rust TUI for orchestrating parallel AI coding workstreams across multiple repositories. The core idea is simple:

- `git` handles repo and worktree operations
- `tmux` owns the real PTY sessions
- iTerm is the first terminal handoff target
- `mico` provides the control plane

## Install

Homebrew is the fastest path:

```sh
brew tap thegeorgejoseph/tap
brew install thegeorgejoseph/tap/mico
mico
```

## What It Does

Today `mico` can:

- track multiple repositories locally
- fetch branches and create git-worktree-backed workstreams
- launch `claude`, `codex`, `opencode`, or a plain terminal inside tmux-backed sessions
- keep multiple agent sessions attached to the same workstream and switch between them from Mission Control
- run one-off non-interactive agent commands inside a workstream without leaving the dashboard
- open those sessions in iTerm or attach in the current terminal
- survive closing `mico` itself because tmux owns the long-lived session
- recover from a missing tmux session by recreating it in the saved worktree
- expose recent git/tmux activity through `mico status --follow`

The repo is organized around the boundaries that matter for the product:

- `src/domain`: app state and core models
- `src/app`: CLI parsing and application ports
- `src/infra`: filesystem, dependency inspection, and platform adapters
- `src/tui`: the mission-control dashboard
- `xtask`: project automation, including one-command releases

## Requirements

`mico` is currently:

- macOS only
- Apple Silicon only
- iTerm-first
- tmux-backed

Expected dependencies:

- `git`
- `tmux`
- `osascript`
- `iTerm`

## Runtime Model

- worktrees live under `~/.mico/worktrees/<repo-slug>/<branch-slug>`
- app state lives in `~/.mico/state.json`
- app config lives in `~/.mico/config.json`
- tmux sessions are the durable execution layer
- `mico` is the control plane, not the thing that keeps your agents alive

Practical implications:

- quitting `mico` does not stop a running workstream
- closing an iTerm tab does not stop a running workstream
- detaching from tmux returns you to `mico` when you attached in-place
- restarting the Mac usually kills live tmux sessions in v1
- if tmux is gone but the worktree still exists, `mico` can recreate the session

## TUI Flow

Launch the app:

```sh
make run
```

Inside the dashboard:

- `:` opens the command palette
- `tab` switches between the repo and workstream panes
- `j` / `k` move the current selection
- `enter` or `o` opens the selected workstream in the current terminal, and `Ctrl-b d` returns to `mico`
- `a` opens the selected workstream in a new iTerm tab
- `n` launches another session inside the selected workstream
- `!` opens the one-off agent drawer for the selected workstream
- `t` opens triage actions for the selected workstream
- `[` / `]` cycle workstream views, and `s` toggles attention-first vs recent sort
- `x` stops the selected workstream without removing it
- `Esc Esc` or `q q` quits the dashboard

Use `:` for less-common actions like add repo, refresh repo, remove repo, resume workstream, and remove workstream.

The command palette is where repo and workstream creation happens:

- choose `Add repo` to track a repository from a path
- choose `Create workstream` to use the selected repo
- choose whether to create a new branch or use an existing branch
- if you create a new branch, pick a base branch from a filterable list
- if you use an existing branch, pick any matching local or remote branch directly
- pick what launches in the tmux session: `terminal`, `claude`, `codex`, `opencode`, or another configured preset

That means you can stay in the TUI for the whole “select repo -> select branch -> make worktree -> launch agent” flow, then flip between filtered workstream views without losing access to the same open, attach, resume, stop, triage, launch-another-session, and one-off command actions.

## CLI Back Door

Track a repository:

```sh
mico repo add /path/to/repo
mico repo list
mico repo branches <repo>
```

Create and manage a workstream:

```sh
mico workstream create --repo <repo> --base main --branch my-task --agent claude
mico workstream create --repo <repo> --branch feature/existing-pr --existing --agent claude
mico workstream create --repo <repo> --base main --branch debug-shell --agent terminal
mico workstream create --repo <repo> --base main --branch opencode-lab --agent opencode
mico workstream list
mico workstream open my-task
mico workstream attach my-task
mico workstream resume my-task
mico workstream stop my-task
mico workstream remove my-task
mico status --follow
```

Clean up a repo after its workstreams are gone:

```sh
mico repo remove <repo>
```

Notes:

- `repo` can be a repo id prefix, display name, slug, or exact path
- `workstream` can be a workstream id prefix, branch name, or tmux session name
- `workstream create --branch <name> --existing` uses an existing local or remote branch as the workstream
- `--agent terminal` creates a plain shell workstream instead of launching an AI agent
- `--agent opencode` launches Opencode inside the tmux-backed session
- `workstream create --open` opens the new session in iTerm
- `workstream create --attach` attaches the new session in your current terminal
- one-off runs currently map to `claude -p`, `codex exec`, and `opencode run`
- worktrees are created under `~/.mico/worktrees/<repo-slug>/<branch-slug>`

## Development

Common local commands:

```sh
make run          # launch the TUI dashboard
make doctor       # inspect local dependencies and mico paths
make paths        # print the local filesystem paths mico uses
make fmt          # format all Rust code
make fmt-check    # verify formatting without changing files
make check        # type-check the whole workspace
make lint         # run clippy with warnings treated as errors
make test         # run all workspace tests
make ci           # run the local CI bundle: fmt-check, check, test
make release      # bump patch version, verify, tag, and push from main
make release BUMP=minor  # same flow, but bump the minor version
make ship         # release mico and update the sibling homebrew tap checkout
```

If you already use [`just`](https://github.com/casey/just), the repo also ships a matching `justfile`.

## Project Ops

Release, install, GitHub repo setup, and Homebrew tap notes live in [docs/OPERATIONS.md](/Users/george/Developer/github.com/mico/docs/OPERATIONS.md).
