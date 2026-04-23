# Production distribution

## Goal

Ship a signed macOS `mico.app` in GitHub Releases and keep the Homebrew cask pointed at the same release channel.

## Repository-owned steps

```sh
make test
make prod-local-build
make prod-local-run
make release-stage VERSION=1.0.0
```

## Manual release steps

1. sign the staged app
2. package and notarize the DMG
3. upload the signed release assets
4. update the Homebrew cask metadata
5. smoke test install, launch, backend startup, worktree creation, and session launch

## Expected release shape

- GitHub Release: signed DMG for `mico`
- Homebrew: `brew install --cask mico`
- In-app updates: Settings opens the same signed release channel
