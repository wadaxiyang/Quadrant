#!/bin/sh
set -eu

repository=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
version=$(awk -F'"' '/^version = "/ { print $2; exit }' "$repository/Cargo.toml")
target_root="$repository/target/package/linux"
staging="$target_root/Quadrant-$version-linux-x86_64"

case "$staging" in
    "$target_root"/*) ;;
    *) echo "Refusing to replace staging outside target/package/linux" >&2; exit 1 ;;
esac

if [ "${1:-}" != "--skip-build" ]; then
    QUADRANT_DISTRIBUTION_CHANNEL=linux-package cargo build \
        --manifest-path "$repository/Cargo.toml" --locked --release -p quadrant-app -p quadrant-agent
fi

rm -rf -- "$staging"
mkdir -p "$staging/bin" "$staging/share/applications" "$staging/share/licenses/quadrant"
cp "$repository/target/release/quadrant-app" "$staging/bin/quadrant"
cp "$repository/target/release/quadrant-agent" "$staging/bin/"
cp "$repository/packaging/linux/quadrant.desktop" "$staging/share/applications/"
for size in 16 20 24 32 40 48 64 128 256 512; do
    icon_dir="$staging/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$icon_dir"
    cp "$repository/assets/branding/quadrant-$size.png" "$icon_dir/quadrant.png"
done
cp "$repository/LICENSE" "$staging/share/licenses/quadrant/"
cp "$repository/packaging/THIRD-PARTY-NOTICES.txt" "$staging/share/licenses/quadrant/"
cp "$repository/packaging/DEPENDENCY-LICENSES.txt" "$staging/share/licenses/quadrant/"
cp "$repository/README.md" "$staging/"

archive="$staging.tar.gz"
tar -C "$target_root" -czf "$archive" "$(basename "$staging")"
sha256sum "$archive" > "$archive.sha256"
printf '%s\n%s\n' "$archive" "$archive.sha256"
