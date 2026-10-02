# Homebrew packaging

`abstract-editor.rb` is a [cask](https://docs.brew.sh/Cask-Cookbook) that
installs `abstract.app` from the release dmg. It picks
`abstract-<version>-macos-aarch64.dmg` on Apple Silicon and
`abstract-<version>-macos-x86_64.dmg` on Intel.

Homebrew does not ship casks out of a project's own repo; they live in a
*tap*. The examples below assume a tap at
`fireflylabss/homebrew-tap` — create that repo if it does not exist yet,
or substitute whichever tap is chosen.

## Publishing for the first time

```bash
git clone https://github.com/fireflylabss/homebrew-tap
cd homebrew-tap
mkdir -p Casks
cp /path/to/abstract/packaging/homebrew/abstract-editor.rb Casks/
git add Casks/abstract-editor.rb
git commit -m "abstract-editor 0.2.0 (new cask)"
git push
```

Users then install with:

```bash
brew install --cask fireflylabss/tap/abstract-editor
```

## Refreshing on a new release

1. Update `version` to the tag (without the `v`).
2. Update both `sha256` values from the release's `SHA256SUMS` — the
   `abstract-<version>-macos-aarch64.dmg` line for `arm:` and the
   `abstract-<version>-macos-x86_64.dmg` line for `intel:`:

   ```bash
   curl -fsSL https://github.com/fireflylabss/abstract/releases/download/v<version>/SHA256SUMS | grep dmg
   ```

3. Commit and push to the tap repo.

Sanity checks before pushing:

```bash
ruby -c Casks/abstract-editor.rb
brew install --cask Casks/abstract-editor.rb   # installs from the local file
brew audit --cask abstract-editor              # needs `brew tap fireflylabss/tap` first
```

[`brew bump-cask-pr`](https://docs.brew.sh/How-To-Open-a-Homebrew-Pull-Request)
can automate the version+sha256 bump once the cask is in a tap.

## Gatekeeper caveat

The dmg on the release is only as signed/notarized as the secrets in
`docs/RELEASING.md` allow. Unsigned (ad-hoc) builds make macOS show the
"damaged or unidentified developer" warning; users can right-click → Open
or run `xattr -dr com.apple.quarantine /Applications/abstract.app`. A tap
cask cannot fix this — it is resolved by shipping notarized builds, not in
the cask file.
