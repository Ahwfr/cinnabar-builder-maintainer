#!/usr/bin/env bash
# Builds Cinnamatica-<version>-<arch>.AppImage from `make package-binaries` output plus this
# revision's schematic importer and guest (builder/linux/build-builder.sh --importer-only).
# Usage: builder/linux/build-appimage.sh [out_dir]
# Env: BINARIES (runtime executables, default target/release), ASSETC, ARCH (default uname -m),
#      APPIMAGETOOL, APPIMAGE_RUNTIME (pinned type2 runtime), APPDIR_ONLY=1 to stage without packing.
set -euo pipefail
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
# shellcheck source=../../packaging/common/stage-payload.sh
source "$here/../../packaging/common/stage-payload.sh"

out="${1:-$repo_root/.local/dist/cinnamatica-linux}"
arch="${ARCH:-$(uname -m)}"
assetc="${ASSETC:-$repo_root/target/release/assetc}"
importer="$repo_root/target/builder-importer/release/schematica-builder"
guest="$repo_root/target/builder-importer/wasm32-unknown-unknown/release/schematica_guest.wasm"
require_file "$assetc" 'build with make package-binaries'
require_file "$importer" 'build with builder/linux/build-builder.sh --importer-only'
require_file "$guest" 'build with builder/linux/build-builder.sh --importer-only'

commit="$(git -C "$repo_root" rev-parse HEAD)"
if [[ -n "$(git -C "$repo_root" status --porcelain --untracked-files=no)" ]]; then
    echo "Commit or stash changes so the package matches one revision." >&2
    exit 1
fi
# The launcher refuses a guest built for a different revision; fail here instead of at first launch.
if ! grep -qaF "$commit" "$guest"; then
    echo "The guest was not built for $commit. Rebuild it with builder/linux/build-builder.sh --importer-only." >&2
    exit 1
fi
version="$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' "$repo_root/builder/schematica/Cargo.toml" | head -n 1)"

# Layout read by the launcher's portable package: the launcher beside App/, with the client in
# App/bin and its resources in App/share/cinnabar.
appdir="$out/Cinnamatica.AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/App"
stage_runtime_binaries "$appdir/App/bin"
stage_resources "$appdir/App/share/cinnabar" "$assetc"
install -m 0755 "$importer" "$appdir/Cinnamatica"
install -m 0644 "$guest" "$appdir/App/schematica-guest-template.bin"
printf '%s\n' "$commit" > "$appdir/App/builder-commit.txt"
printf '%s\n' "$version" > "$appdir/App/builder-version.txt"
install -m 0644 "$repo_root/LICENSE" "$repo_root/THIRD_PARTY_NOTICES.md" \
    "$repo_root/builder/schematica/SCHEMATICA-LICENSE.txt" "$appdir/App/"
install -m 0644 "$here/README.md" "$appdir/App/README.md"
install -m 0755 "$here/AppRun" "$appdir/AppRun"
install -m 0644 "$here/cinnamatica.desktop" "$appdir/cinnamatica.desktop"
install -m 0644 "$repo_root/packaging/icons/cinnabar.svg" "$appdir/cinnamatica.svg"
ln -s cinnamatica.svg "$appdir/.DirIcon"

if [[ "${APPDIR_ONLY:-0}" == 1 ]]; then
    echo "$appdir"
    exit 0
fi
image="$out/Cinnamatica-$version-$arch.AppImage"
runtime_args=()
[[ -z "${APPIMAGE_RUNTIME:-}" ]] || runtime_args=(--runtime-file "$APPIMAGE_RUNTIME")
ARCH="$arch" "${APPIMAGETOOL:-appimagetool}" "${runtime_args[@]}" "$appdir" "$image"
echo "$image"
