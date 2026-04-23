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

## Distribution shape

- user-facing artifact: `mico.app` inside a signed macOS DMG
- primary install path: `brew install --cask mico`
- fallback install path: latest signed GitHub Release
- current update path: Settings opens the latest signed release

The release work that still happens outside this repo is signing, notarization, release upload, and tap publication.
