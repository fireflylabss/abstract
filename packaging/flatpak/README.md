# Flatpak packaging

`io.github.fireflylabss.abstract.yaml` builds a Flatpak that installs the
released Linux binaries (x86_64 and aarch64 tarballs) and ships the
`.desktop` file, icon and AppStream metainfo from the source tag.

## App ID

`io.github.fireflylabss.abstract` follows the `io.github.<org>` scheme
that Flathub accepts without domain verification — control is proven
through the GitHub org. The app's `CFBundleIdentifier` on macOS is
`com.fireflylabs.abstract`, which presumes ownership of `fireflylabs.com`;
if the owner controls that domain (or another one), switching the ID
before the Flathub listing goes public is the moment to do it — the ID is
fixed once published. Rename the manifest, metainfo filename, `<id>`,
`<launchable>` and the two install paths together.

## Building locally

```bash
flatpak-builder --user --install --force-clean build-dir \
  packaging/flatpak/io.github.fireflylabss.abstract.yaml
flatpak run io.github.fireflylabss.abstract
```

Before a real build the placeholders must be real values:

1. `sha256` of each release tarball, from the release's `SHA256SUMS`
   (`abstract-<version>-linux-{x86_64,aarch64}.tar.gz` lines).
2. `commit` of the git source — `git rev-list -n1 v<version>` once the tag
   exists.
3. `date` of the `<release>` entry in the metainfo file.

Validate metainfo and manifest before submitting:

```bash
appstreamcli validate packaging/flatpak/io.github.fireflylabss.abstract.metainfo.xml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest \
  packaging/flatpak/io.github.fireflylabss.abstract.yaml
```

## Submitting to Flathub

1. Fork [`flathub/flathub`](https://github.com/flathub/flathub), create a
   branch, add a directory `io.github.fireflylabss.abstract/` containing
   the manifest (the metainfo is fetched from the upstream tag, as wired
   here), and open a PR with the "New submission" template.
2. Flathub's linter and test build run on the PR; fix what they flag.
3. Once merged, the app builds on Flathub's builders and appears at
   `flathub.org/apps/io.github.fireflylabss.abstract`.

Full guide:
<https://docs.flathub.org/docs/for-app-authors/submission>

The manifest repackages released binaries. Flathub allows this for
upstream-maintained submissions; if maintainers ever want a source build
instead, the Rust deps need vendoring via
[`flatpak-cargo-generator`](https://github.com/flatpak/flatpak-builder-tools).

## Refreshing on a new release

Bump the three placeholder spots listed above (`url`/`sha256`, git
`tag`/`commit`, metainfo `release` version+date), commit here, and open
the matching PR in the Flathub repo's app directory.
