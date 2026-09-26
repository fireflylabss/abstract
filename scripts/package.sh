#!/usr/bin/env bash
# Package a release binary into dist/.
# Usage: package.sh <target-triple> <artifact-name> <version>
set -euo pipefail

target="$1"
artifact="$2"
version="$3"

bin="target/${target}/release/abstract-editor"
mkdir -p dist

case "${target}" in
*-apple-darwin | *-linux-gnu) ;;

*)
    echo "unsupported target: ${target}" >&2
    exit 1
    ;;
esac

# Common tarball: binary + README + LICENSE
stage="dist/pkg"
rm -rf "${stage}"
mkdir -p "${stage}"
cp "${bin}" "${stage}/abstract-editor"
cp README.md LICENSE "${stage}/"
tar -C "${stage}" -czf "dist/abstract-editor-${version}-${artifact}.tar.gz" .
rm -rf "${stage}"

# macOS: bare .app bundle (ad-hoc signed), zipped
if [[ "${target}" == *-apple-darwin ]]; then
    app="dist/abstract.app"
    rm -rf "${app}"
    mkdir -p "${app}/Contents/MacOS"
    cp "${bin}" "${app}/Contents/MacOS/abstract-editor"
    cat >"${app}/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>abstract</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.horizzon3507.abstract-editor</string>
    <key>CFBundleExecutable</key>
    <string>abstract-editor</string>
    <key>CFBundleVersion</key>
    <string>${version}</string>
    <key>CFBundleShortVersionString</key>
    <string>${version}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
    codesign --force --deep -s - "${app}"
    ditto -c -k --keepParent "${app}" "dist/abstract-${version}-${artifact}.app.zip"
    rm -rf "${app}"
fi

ls -lh dist/
