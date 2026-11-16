#!/usr/bin/env bash
# Builds the Builder client, Go core, schematic importer and guest for this revision.
# Usage: builder/linux/build-builder.sh [--profile dev|release|play] [--importer-only]
set -euo pipefail

profile=release
importer_only=0
while (($#)); do
    case $1 in
        --profile) profile=${2:?--profile needs dev, release or play}; shift 2 ;;
        --importer-only) importer_only=1; shift ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done
case $profile in dev | release | play) ;; *) echo "Choose --profile dev, release or play." >&2; exit 2 ;; esac

checkout=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tools=(cargo rustup)
((importer_only)) || tools+=(go make)
missing=()
for tool in "${tools[@]}"; do command -v "$tool" >/dev/null || missing+=("$tool"); done
if ((${#missing[@]})); then
    echo "Install the required build tools: ${missing[*]}." >&2
    exit 1
fi

cd "$checkout"
manifest=builder/schematica/Cargo.toml
target=target/builder-importer
profile_dir=$([[ $profile == dev ]] && echo debug || echo "$profile")
stamp=target/$profile_dir/builder-commit.txt
commit=$(git rev-parse HEAD)
if ((!importer_only)) && [[ -n $(git status --porcelain) ]]; then
    echo "Commit or stash changes before building the launcher version." >&2
    exit 1
fi

export SCHEMATICA_BUILD_REVISION=$commit
cargo build --locked --release --manifest-path "$manifest" --target-dir "$target" -p schematica-builder
cargo build --locked --release --manifest-path "$manifest" --target-dir "$target" \
    -p schematica-guest --target wasm32-unknown-unknown
((importer_only)) && exit 0

rm -f "$stamp"
make play-build "PROFILE=$profile"
if [[ $(git rev-parse HEAD) != "$commit" || -n $(git status --porcelain) ]]; then
    echo "The source changed during the build. Rebuild before launching." >&2
    exit 1
fi
for file in cinnabar bedrock-core; do
    [[ -f target/$profile_dir/$file ]] || { echo "The upstream build did not produce $file." >&2; exit 1; }
done
echo "$commit" >"$stamp"
echo "Built Cinnabar Builder $commit. Start it with builder/linux/start-builder.sh."
