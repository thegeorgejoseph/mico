# Local Production Testing

This doc covers the fastest way to test `mico` in a production-like shape on your local machine without publishing anything.

This is not the final signed/notarized distribution flow. It is a local smoke-test path that uses:

- built renderer assets
- a compiled Go backend binary
- a packaged unsigned `mico.app` built from the local Electron runtime

That makes it the right way to test the real packaged runtime contract before you touch signing or notarization.

## What This Verifies

The local production test path verifies:

- the renderer builds correctly from `app/dist`
- the backend builds correctly as a standalone binary
- `mico.app` launches without the Vite dev server
- the packaged app can find its bundled backend in `Contents/Resources/backend/mico-desktop`
- the app works with `app.isPackaged` behavior instead of the development path

It does not yet verify:

- a signed `.app`
- a `.dmg`
- notarization
- the final signed and notarized release artifact

## One-Time Setup

From `mico-desktop/`:

```sh
make install
```

## Build Production-Like Assets

```sh
make prod-local-build
```

This does:

1. build `backend/bin/mico-desktop`
2. build the renderer into `app/dist`
3. stage local production inputs under `dist/local-prod/`
4. package an unsigned `dist/local-prod/mico.app`

After that you should have:

```text
dist/local-prod/
  backend/mico-desktop
  mico.app
  manifest.txt
```

## Run the App in Production-Like Mode

```sh
make prod-local-run
```

This launches the packaged app executable directly from:

```text
dist/local-prod/mico.app/Contents/MacOS/mico
```

That means:

- no Vite dev server
- no `electron .`
- no `go run`
- the same packaged backend lookup used by the shipped app bundle

## Inspect the Local Bundle

If you want to sanity-check the local bundle before launching it:

```sh
plutil -p dist/local-prod/mico.app/Contents/Info.plist
find dist/local-prod/mico.app/Contents/Resources -maxdepth 2 -type f | sort
```

Important things to see:

- bundle name is `mico`
- icon is `mico.icns`
- bundled backend exists at `Contents/Resources/backend/mico-desktop`
- packaged app files exist at `Contents/Resources/app/`

## Manual Smoke-Test Checklist

Before you call a build good, check:

1. the app launches without the dev server
2. the macOS app/menu name is `mico`, not `Electron`
3. existing state imports cleanly
4. projects load
5. worktrees load
6. you can create a worktree
7. you can start a terminal session
8. terminal input/output works
9. command palette / agent mode works
10. settings and doctor work
11. light and dark mode both look sane
12. Settings -> App shows the expected version

## Typical Local QA Loop

From `mico-desktop/`:

```sh
make test
make prod-local-build
make prod-local-run
```

That is the path to use before staging a release.

## Clean the Local Build

```sh
make clean-local-prod
```

## Notes

- This is the recommended path for local QA before you make a release candidate.
- If `prod-local-run` fails, fix that before touching DMG/signing/notarization work.
