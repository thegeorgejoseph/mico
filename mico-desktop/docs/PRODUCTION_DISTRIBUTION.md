# Production distribution

## Goal

Ship a signed macOS `mico.app` in GitHub Releases and keep the Homebrew cask pointed at the same release channel.

## Repository-owned steps

```sh
make test
make prod-local-build
make prod-local-run
MICO_CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" make release-macos VERSION=1.0.0
```

## What the release helper does

1. stages `mico.app`
2. signs the app with a Developer ID Application certificate
3. builds `mico-desktop-arm64.dmg`
4. notarizes and staples the DMG using the `mico-notary` keychain profile
5. uploads the DMG and checksum to the GitHub release for the matching tag

## Expected release shape

- GitHub Release: signed DMG for `mico`
- Homebrew: `brew install --cask thegeorgejoseph/tap/mico-desktop`
- In-app updates: Settings opens the same signed release channel
