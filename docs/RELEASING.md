# Releasing abstract

## Cutting a release

1. Bump `version` in `Cargo.toml`, update `Cargo.lock`
   (`cargo update -p abstract-editor`), and fill in the `CHANGELOG.md` entry.
2. Commit, tag `vX.Y.Z`, push the tag. The `release` workflow builds all
   targets, packages them, and publishes a GitHub Release with checksums.

Artifacts per release: Linux x86_64 and aarch64 (`tar.gz`, `.deb`, `.rpm`,
`.AppImage`), macOS arm64 + Intel (`dmg`, `.app.zip`, `tar.gz`), Windows
(`exe`, `zip`), plus `SHA256SUMS`.

## macOS signing and notarization

Without secrets the workflow produces ad-hoc signed builds. To ship signed,
notarized builds, set these repository secrets:

- `MACOS_CERTIFICATE_P12` — Developer ID Application certificate exported
  from Keychain Access as `.p12`, then `base64 -i cert.p12 | pbcopy`.
- `MACOS_CERTIFICATE_PASSWORD` — the export password of that `.p12`.
- `MACOS_SIGNING_IDENTITY` — e.g. `Developer ID Application: Name (TEAMID)`.
- `APPLE_ID` — Apple ID used for notarization.
- `APPLE_TEAM_ID` — 10-character team identifier.
- `APPLE_APP_PASSWORD` — app-specific password from appleid.apple.com
  (Sign-In and Security → App-Specific Passwords).

The workflow imports the cert into a temporary keychain, stores a notarytool
profile named `abstract-notary`, signs with `--options runtime --timestamp`,
submits the app and dmg to the notary service, and staples both tickets.

## AUR bump after a release

Each AUR package is its own repo (`packaging/aur/README.md` has the full flow):

1. `pkgver=<new>` and `pkgrel=1` in `packaging/aur/abstract-editor/PKGBUILD`
   and `packaging/aur/abstract-editor-bin/PKGBUILD`.
2. `updpkgsums` inside a checkout of the AUR repo (needs the tag and release
   assets published first).
3. `makepkg --printsrcinfo > .SRCINFO`, `makepkg -si` to verify.
4. Commit `PKGBUILD` + `.SRCINFO` and push to
   `ssh://aur@aur.archlinux.org/<pkgname>.git`.

## Distribution channels

Manifests for non-GitHub distribution channels live under `packaging/`,
each with a README covering the publish/refresh flow. All of them consume
the GitHub Release assets and `SHA256SUMS`, so cut the release first, then
update the channel manifests.

### AUR

`packaging/aur/` — covered in "AUR bump after a release" above.

### Homebrew

`packaging/homebrew/abstract-editor.rb` is a cask installing
`abstract.app` from the release dmg on both arm and Intel. Copy it into
the `fireflylabss/homebrew-tap` repo under `Casks/` and refresh `version`
plus the two `sha256` values from `SHA256SUMS`
(`abstract-*-macos-*.dmg` lines).

### Flatpak

`packaging/flatpak/` holds the `io.github.fireflylabss.abstract` manifest
plus the AppStream metainfo. It installs the released Linux tarballs and
fetches `.desktop`/icon from the tag. Refresh `sha256` for both arches,
the git source `commit`, and the metainfo `<release>` entry, then open a
PR against `flathub/flathub` per `packaging/flatpak/README.md`.

### winget

`packaging/winget/` is the three-manifest set (`version`, `installer`,
`defaultLocale`) for the portable Windows `.exe`. Fill `InstallerSha256`
from `SHA256SUMS` and submit/refresh via `wingetcreate` against
`microsoft/winget-pkgs`; see `packaging/winget/README.md`.

The Homebrew, Flatpak and winget submissions need no secrets of their own,
but the macOS dmg they point at is only Gatekeeper-clean when the Apple
secrets in "macOS signing and notarization" above are configured — without
them users get the unsigned-app warning.
