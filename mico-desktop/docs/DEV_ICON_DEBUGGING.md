# Dev Icon Debugging

This note explains the macOS app-switcher icon problem we hit while developing `mico desktop`, what actually caused it, and why the final fix worked.

## Symptom

In development, the app name in the macOS menu bar was correctly showing `mico`, but the Dock / `Cmd+Tab` app switcher still behaved like Electron in a few ways:

- it sometimes showed `Electron` instead of `mico`
- it showed an icon with an unwanted white background / white framed tile
- icon changes sometimes appeared to do nothing even after regenerating assets

The problem was confusing because different macOS surfaces were behaving differently:

- menu bar app name: mostly correct
- in-app icon: correct
- Dock / `Cmd+Tab`: stale or wrong

## What Was Going On

There turned out to be **three separate issues** layered on top of each other.

### 1. We were fixing the runtime name, but not the host app identity

Setting `app.setName("mico")` in Electron is enough to improve some runtime-facing surfaces, but it does **not** fully replace the identity of the host `Electron.app` bundle in dev.

That meant:

- the renderer could know it was `mico`
- the menu bar could say `mico`
- but the Dock / app switcher could still treat the process like Electron

## 2. Our first dev launch path executed the binary directly

At one point we launched the branded app by executing the binary inside the cloned bundle directly:

- `.../mico.app/Contents/MacOS/mico`

That improved some behavior, but it still did not make macOS consistently treat the process as the app bundle we intended.

The better path was to launch the bundle itself with:

- `open -a <path-to-mico.app> --args ...`

That gives Launch Services the actual app bundle to reason about.

### 3. The icon asset itself was wrong for macOS shell use

We were initially reusing the in-app mark as the app icon. That was a bad fit.

The in-app logo looked good inside the UI, but it was composed like a design asset:

- smaller dark tile
- transparency around it
- aesthetically fine in the app
- not ideal as a macOS app icon

When macOS rendered that in the app switcher, it effectively treated the transparent outer area like empty canvas and placed the icon on its own white app plate. That is why the switcher looked like it had a white border or white tile behind the icon.

## Why Regenerating the Icon Initially Seemed To Do Nothing

Even after correcting the icon asset, the switcher often still showed the old result.

That happened because the dev app identity stayed stable:

- same app path
- same bundle id
- same display name

From Launch Services' perspective, it was still "the same app", so macOS had every excuse to reuse cached switcher metadata and icon presentation.

In other words, we were changing the icon file, but not changing the identity of the app macOS associated with that icon.

## The Final Fix

The working solution had **two parts**.

### A. Split the shell icon from the in-app logo

We now keep two separate icon concepts:

- `mico-mark.*`: the in-app logo / visual mark
- `mico-shell-icon.*`: the macOS shell icon for Dock, switcher, and packaged app bundle use

That lets the app UI keep the mark that looks good inside the interface while the operating system gets an icon composed specifically for app-shell rendering.

Important detail:

- the shell icon renditions are **opaque**
- the earlier icon renditions had alpha

That change matters because it stops macOS from compositing the icon like a small floating tile on top of a white switcher plate.

### B. Make the dev app identity depend on the shell icon

In development we now generate a branded app bundle whose name and bundle id include a short hash of the shell icon file.

Example:

- app path: `dist/dev-electron/mico-13470d11a867.app`
- bundle id: `com.thegeorgejoseph.mico.dev.13470d11a867`

This means that when the shell icon changes, the dev app is no longer "the same app" to Launch Services.

That forces macOS to treat it as a fresh app identity instead of reusing stale switcher icon state.

## Why This Does Not Break Production

The hash-based identity trick is **dev-only**.

Production packaging still uses:

- stable bundle name: `mico.app`
- stable bundle id: `com.thegeorgejoseph.mico`
- stable icon file inside the app bundle: `mico.icns`

The development workaround exists only because local Electron dev flows interact badly with Launch Services caching.

Production is simpler:

- the app bundle is assembled once
- the backend binary is bundled in `Contents/Resources/backend/mico-desktop`
- the shell icon is copied into the packaged app bundle
- the packaged app uses a normal stable identity

So the dev fix does not leak into shipping behavior.

## Files That Matter

### Dev branding

- `scripts/sync-dev-electron-brand.sh`
- `scripts/launch-dev-electron-app.sh`

### Shell icon assets

- `app/assets/mico-shell-icon.svg`
- `app/assets/mico-shell-icon.png`
- `app/assets/mico-shell-icon.icns`

### In-app mark

- `app/assets/mico-mark.svg`
- `app/assets/mico-mark.png`
- `app/assets/mico-mark.icns`

### Packaging

- `scripts/package-macos-app.sh`
- `scripts/build-local-prod.sh`
- `scripts/stage-release.sh`

### Electron runtime behavior

- `app/main.js`

## Practical Rule Going Forward

If the macOS switcher icon looks wrong in dev:

1. treat the shell icon as a separate asset from the in-app logo
2. regenerate the `.icns`
3. re-sync the branded dev bundle
4. make sure the dev bundle identity changes when the shell icon changes

If the packaged app looks wrong in prod:

1. inspect the final app bundle plist
2. confirm `CFBundleIconFile`
3. confirm the packaged `.icns` matches the intended shell icon
4. verify the final `mico.app` bundle directly, not just the source assets

## Short Version

The bug was not just "an icon file problem."

It was the combination of:

- Electron dev host identity
- Launch Services caching
- using an in-app logo as a shell icon
- transparent icon renditions that macOS composited poorly

The fix worked because we addressed all of those at the right layer:

- separate shell icon
- opaque shell icon renditions
- app-bundle launch in dev
- hash-based dev app identity for cache busting
- stable normal identity in production
