#!/bin/sh
set -eu

repository=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
version=$(awk -F'"' '/^version = "/ { print $2; exit }' "$repository/Cargo.toml")
target_root="$repository/target/package/macos"
bundle="$target_root/Quadrant.app"

case "$bundle" in
    "$target_root"/*) ;;
    *) echo "Refusing to replace bundle outside target/package/macos" >&2; exit 1 ;;
esac

if [ "${1:-}" != "--skip-build" ]; then
    QUADRANT_DISTRIBUTION_CHANNEL=macos-bundle cargo build \
        --manifest-path "$repository/Cargo.toml" --locked --release -p quadrant-app -p quadrant-agent
fi

rm -rf -- "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$repository/target/release/quadrant-app" "$bundle/Contents/MacOS/quadrant"
cp "$repository/target/release/quadrant-agent" "$bundle/Contents/MacOS/"
cp "$repository/assets/branding/Quadrant.icns" "$bundle/Contents/Resources/"
cp "$repository/LICENSE" "$bundle/Contents/Resources/"
cp "$repository/packaging/THIRD-PARTY-NOTICES.txt" "$bundle/Contents/Resources/"
cp "$repository/packaging/DEPENDENCY-LICENSES.txt" "$bundle/Contents/Resources/"
sed "s/@VERSION@/$version/g" "$repository/packaging/macos/Info.plist.in" > "$bundle/Contents/Info.plist"

architecture=$(uname -m)
archive="$target_root/Quadrant-$version-macos-$architecture-unsigned.tar.gz"
tar -C "$target_root" -czf "$archive" Quadrant.app
shasum -a 256 "$archive" > "$archive.sha256"
printf '%s\n%s\n' "$archive" "$archive.sha256"
