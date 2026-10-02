# winget packaging

The three manifests here are a standard
[winget-pkgs](https://github.com/microsoft/winget-pkgs) set for the
release's standalone Windows `.exe`:

- `Fireflylabss.abstract-editor.yaml` — `version` manifest (pins
  `DefaultLocale`).
- `Fireflylabss.abstract-editor.installer.yaml` — `installer` manifest
  (`InstallerType: portable`; the `.exe` is the app itself, not an
  installer).
- `Fireflylabss.abstract-editor.locale.en-US.yaml` — `defaultLocale`
  metadata (publisher, license, description, tags).

`InstallerType: portable` means `winget install` places the binary on the
user's PATH with a `abstract` command alias. If a real installer (MSI,
Inno, MSIX) is ever produced, add it as a second installer entry or new
manifest.

## Before submitting

`InstallerSha256` is a placeholder — fill it from the release's
`SHA256SUMS` (`abstract-<version>-windows-x86_64.exe` line):

```bash
curl -fsSL https://github.com/fireflylabss/abstract/releases/download/v<version>/SHA256SUMS | grep windows
```

## Publishing for the first time

1. Fork [`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs).
2. The files go in
   `manifests/f/Fireflylabss/abstract-editor/<version>/` — path segments
   are the `PackageIdentifier` parts, first letter lowercased for the
   `manifests/<letter>/` root.
3. Easiest path is [`wingetcreate`](https://github.com/microsoft/winget-create)
   which does the fork+PR for you:

   ```powershell
   wingetcreate new https://github.com/fireflylabss/abstract/releases/download/v<version>/abstract-<version>-windows-x86_64.exe
   ```

   then merge the metadata from this directory into the generated
   manifests (or copy these files over them before submitting).

4. Validate locally first:

   ```powershell
   winget validate --manifest <dir-containing-the-three-yamls>
   ```

5. Open the PR against `microsoft/winget-pkgs`; the pipeline validates,
   test-installs and, once green and reviewed, merges.

## Refreshing on a new release

```powershell
wingetcreate update Fireflylabss.abstract-editor \
  -u https://github.com/fireflylabss/abstract/releases/download/v<version>/abstract-<version>-windows-x86_64.exe \
  -v <version>
```

then commit the updated manifests here too, so the repo copy stays the
source of truth.

## Identity fields to confirm

- `Publisher: fireflylabss` — GitHub org name; winget renders this as the
  publisher line in `winget show`. Confirm the owner wants that exact
  spelling before first submission (it is sticky once published).
- `PackageIdentifier: Fireflylabss.abstract-editor` — matches the
  `abstract-editor` name used by the AUR and Homebrew packages.
