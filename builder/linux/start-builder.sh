#!/usr/bin/env bash
# Starts the Builder client with its isolated profile in .local/builder-user.
# The importer installs or upgrades the Schematica library and enables it before launch.
#
# Usage: builder/linux/start-builder.sh [--profile debug|release|play] [--check]
#        builder/linux/start-builder.sh --import-only [--demo] [FILE...]
set -euo pipefail

profile=release
check=0
import_only=0
demo=0
inputs=()
while (($#)); do
    case $1 in
        --profile) profile=${2:?--profile needs debug, release or play}; shift 2 ;;
        --check) check=1; shift ;;
        --import-only) import_only=1; shift ;;
        --demo) demo=1; shift ;;
        --) shift; inputs+=("$@"); break ;;
        -*) echo "Unknown option: $1" >&2; exit 2 ;;
        *) inputs+=("$1"); shift ;;
    esac
done
case $profile in debug | release | play) ;; *) echo "Choose --profile debug, release or play." >&2; exit 2 ;; esac

checkout=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
profile_dir=$checkout/target/$profile
user_root=${CINNABAR_USER_ROOT:-$checkout/.local/builder-user}
stamp=$profile_dir/builder-commit.txt
codec=$checkout/target/builder-importer/release/schematica-builder
guest=$checkout/target/builder-importer/wasm32-unknown-unknown/release/schematica_guest.wasm
head=$(git -C "$checkout" rev-parse HEAD)

built=0
if [[ -f $profile_dir/cinnabar && -f $profile_dir/bedrock-core && -f $stamp ]] &&
    [[ $(<"$stamp") == "$head" ]]; then
    built=1
fi
importer_ready=0
[[ -f $codec && -f $guest ]] && importer_ready=1

if ((check)); then
    bedrock=$(sed -n 's/.*"game_version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
        "$checkout/assets/bedrock-target.json")
    missing=()
    for tool in go make; do command -v "$tool" >/dev/null || missing+=("$tool"); done
    picker=none
    for tool in zenity kdialog; do command -v "$tool" >/dev/null && { picker=$tool; break; }; done
    printf '%-14s %s\n' Checkout "$checkout" Commit "$head" Bedrock "$bedrock" \
        ClientBuilt "$built" ImporterBuilt "$importer_ready" UserRoot "$user_root" \
        FilePicker "$picker" MissingTools "${missing[*]:-}"
    exit 0
fi

((importer_ready)) || { echo "Build the importer with builder/linux/build-builder.sh --importer-only first." >&2; exit 1; }

if ((import_only)); then
    mods=$user_root/data/mods
    if ((!demo && ${#inputs[@]} == 0)); then
        exec env SCHEMATICA_GUEST_TEMPLATE="$guest" "$codec" import-dialog "$mods"
    fi
    backups=$user_root/library-backups
    mkdir -p "$mods" "$backups"
    installed=$mods/schematica-builder.wasm
    # The importer creates the staged file itself and refuses to overwrite one.
    staged=$backups/import-$$-$RANDOM.pending
    trap 'rm -f "$staged"' EXIT
    mode=$([[ $demo == 1 ]] && echo demo-library || echo library)
    SCHEMATICA_GUEST_TEMPLATE=$guest "$codec" "$mode" "$staged" "${inputs[@]}"
    if [[ -f $installed ]]; then
        cp -- "$installed" "$backups/$(date +%Y%m%d-%H%M%S)-$(basename "$staged" .pending).wasm"
    fi
    mv -- "$staged" "$installed"
    echo "Installed $installed. Start Builder, grant its permissions in Settings > Mods, then press Alt+J in a world."
    exit 0
fi

((built)) || { echo "Build this revision with builder/linux/build-builder.sh before launching." >&2; exit 1; }
CINNABAR_USER_ROOT=$user_root exec "$codec" start-checkout "$checkout" "$profile"
