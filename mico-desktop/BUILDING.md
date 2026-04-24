# Building mico desktop

## Day-to-day

```sh
make install
make dev
```

## Verification

```sh
make test
make build
```

## Local production-like smoke test

```sh
make prod-local-build
make prod-local-run
```

## Release staging

```sh
make release-stage VERSION=1.0.0
```

That produces an unsigned staged `mico.app` bundle plus the backend binary under `dist/release/v1.0.0/`.

## Release and publish

```sh
MICO_CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
make release-macos VERSION=1.0.0
```

`release-macos` runs the local verification steps, stages the app bundle, signs it, builds a stable `mico-desktop-arm64.dmg`, notarizes it with the `mico-notary` keychain profile by default, and uploads it to the GitHub release for `v1.0.0`.

## Distribution shape

- user-facing artifact: `mico.app` inside a signed macOS DMG
- primary install path: `brew install --cask thegeorgejoseph/tap/mico-desktop`
- fallback install path: latest signed GitHub Release
- current update path: Settings opens the latest signed release

The release work that still happens outside this repo is installing your Apple Developer credentials and the first-time Homebrew tap setup.
