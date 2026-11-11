# Builder releases

Before editing, inspect the working tree and preserve existing work. Run the
relevant focused tests and build checks, then update documentation and release
notes for changed behavior. Distinguish verified results from proposed support.
Review the final diff and package for private data and required notices.
Use only an authorized repository for pushes. Prepare a patch if access is
unavailable. Public releases and visibility changes require owner approval.

Each shipped update increments workspace.package.version in schematica/Cargo.toml.
Keep RELEASE-NOTES.md and UPDATE.md accurate for that version. The guest and importer
inherit the shared version. Preserve the stable schematica-builder.wasm filename.

Stage the current executable package, importer, component, exact source ZIP and editable
source in the user's outputs/Latest. Archive superseded release payloads under
outputs/Archive. Preserve profile data and original schematic inputs.

Publish completed packages with Publish-BuilderRelease.ps1. Never include profiles,
credentials, compiler caches or local game asset caches in release ZIPs. Do not overwrite
an existing release version. Record missing live server acceptance evidence honestly.

Use one persistent local compiler directory. UI-only updates use the matching native API
and do not require rebuilding an unchanged runtime. Keep authored compact/full panels
precomputed so JSON parsing and layout fitting do not consume the WASM callback budget.
