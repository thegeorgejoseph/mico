# Desktop operations

## Install

Preferred path:

```sh
brew install --cask thegeorgejoseph/tap/mico-desktop
```

Fallback path:

- download the latest signed DMG from the `mico` GitHub Releases page

## Update

- in-app Settings currently opens the latest signed release
- Homebrew cask users can also update through Homebrew

## Release checklist

1. `make test`
2. `make prod-local-build`
3. `make prod-local-run`
4. `MICO_CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" make release-macos VERSION=x.y.z`
5. smoke test install, launch, backend startup, worktree creation, and session launch
