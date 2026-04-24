# Operations

This doc covers packaging and distribution details for `mico`.

## Install

Homebrew is the recommended install path because it can pull in `tmux` automatically:

```sh
brew tap thegeorgejoseph/tap
brew install thegeorgejoseph/tap/mico
mico
```

There is also a GitHub Releases bootstrap path through `install.sh`:

```sh
curl -fsSL https://raw.githubusercontent.com/thegeorgejoseph/mico/main/install.sh | sh
```

If `mico` was installed through Homebrew or GitHub Releases, rerun the original install path to pick up the latest published build.

The updater can use either:

- `github_repo` in `~/.mico/config.json`
- the `package.repository` GitHub URL from `Cargo.toml`

## Release Flow

1. Set the real GitHub repository URL in `Cargo.toml`.
2. Push `main` to GitHub and enable Actions on the repo.
3. Run:

```sh
make release
```

For a non-patch release:

```sh
make release BUMP=minor
```

That will:

1. verify you are on `main`
2. verify the working tree is clean
3. bump the version in `Cargo.toml`
4. refresh workspace lockfile entries without broadly updating dependencies
5. run format, check, and test
6. create a release commit and tag
7. push `main` and the tag

GitHub Actions then builds the Apple Silicon binary and uploads:

- `mico-<version>-aarch64-apple-darwin.tar.gz`
- `mico-<version>-aarch64-apple-darwin.tar.gz.sha256`

## Homebrew Tap Update

If you want the tap formula updated from the same laptop workflow, use:

```sh
make ship
```

For a non-patch release:

```sh
make ship BUMP=minor
```

That local helper:

1. runs the normal release flow
2. waits for the GitHub release checksum artifact
3. updates the sibling `homebrew-tap` checkout at `../homebrew-tap` by default
4. commits the formula change
5. pushes the tap repo

If your tap repo lives somewhere else, set `MICO_HOMEBREW_TAP_DIR` before running the helper.
