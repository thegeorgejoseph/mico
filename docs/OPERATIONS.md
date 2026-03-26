# Operations

This doc covers packaging and distribution details for `mico`.

## Install

Homebrew is the recommended install path because it can pull in `tmux` automatically:

```sh
brew install <tap>/mico
```

There is also a GitHub Releases bootstrap path through `install.sh`:

```sh
curl -fsSL https://raw.githubusercontent.com/thegeorgejoseph/mico/main/install.sh | sh
```

If `mico` was installed through Homebrew, `mico install` upgrades the Homebrew formula. If it was installed from GitHub Releases, `mico install` reinstalls from the latest GitHub release.

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
4. regenerate `Cargo.lock`
5. run format, check, and test
6. create a release commit and tag
7. push `main` and the tag

GitHub Actions then builds the Apple Silicon binary and uploads:

- `mico-<version>-aarch64-apple-darwin.tar.gz`
- `mico-<version>-aarch64-apple-darwin.tar.gz.sha256`
