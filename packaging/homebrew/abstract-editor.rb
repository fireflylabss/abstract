cask "abstract-editor" do
  version "0.2.0"

  arch arm: "aarch64", intel: "x86_64"

  # sha256: filled from the release's SHA256SUMS
  # (abstract-<version>-macos-<arch>.dmg). See packaging/homebrew/README.md.
  sha256 arm:   "0000000000000000000000000000000000000000000000000000000000000000",
         intel: "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/fireflylabss/abstract/releases/download/v#{version}/abstract-#{version}-macos-#{arch}.dmg"
  name "abstract"
  desc "Minimal local-first markdown notes editor"
  homepage "https://github.com/fireflylabss/abstract"

  # LSMinimumSystemVersion in the bundle is 11.0.
  depends_on macos: ">= :big_sur"

  app "abstract.app"

  # Notes live in folders the user picks; nothing to remove. The seeded
  # default space stays under ~/.local/share/abstract and is user data.
  zap trash: [
    "~/.config/abstract",
    "~/.local/state/abstract",
  ]
end
