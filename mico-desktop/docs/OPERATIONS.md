# Desktop operations

## Install

Preferred path:

```sh
brew install --cask mico
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
4. `make release-stage VERSION=x.y.z`
5. sign and notarize the staged app/DMG outside the repo
6. publish the signed release
7. update the Homebrew cask metadata
