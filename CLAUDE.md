# Cinnamatica

Cinnamatica is a fork of the Cinnabar Bedrock client that adds the Schematica builder mod.
Most fork-specific code lives in `builder/`:

- `builder/schematica/core`: the blueprint format (`SCB2` header, then `[sx, sy, sz]`) and the codecs.
- `builder/schematica/guest`: the WASM mod. It contains the window, Fast Place, the ghost overlay and paste.
- `builder/schematica/builder`: the `schematica-builder` importer and launcher binary.
- `builder/linux`, `builder/windows`: the build and launch scripts. Keep the two platforms in step.

Read `builder/AGENTS.md` for release rules (version bumps, `RELEASE-NOTES.md`, `UPDATE.md`).
The architecture rules in the root `AGENTS.md` still apply. Its upstream workflow does not: no PRs into `dev`, no Codex review, no parallel-agent build rules.

## Which change needs which build

| You changed | Rebuild | Time |
| --- | --- | --- |
| Only `builder/schematica/**` | Guest WASM (and the importer if you touched it) | seconds |
| `app/`, `crates/`, `core/`, `Cargo.lock` | Full client: `builder/linux/build-builder.sh` | ~25 min (release) |

Why client builds are slow:

- Every commit changes `CINNABAR_BUILD_COMMIT`, which `crates/diagnostics/src/session.rs` bakes into the binary. Most of the client depends on that crate, so each new commit recompiles nearly everything.
- `profile.release` uses `codegen-units = 1` and thin LTO, with no incremental builds. `--profile play` is incremental and much faster to rebuild.

How launching works (`builder/linux/start-builder.sh`):

- The script only starts a client whose `target/<profile>/builder-commit.txt` equals `git rev-parse HEAD`.
- `build-builder.sh` deletes that stamp before it builds. It refuses a dirty tree, and it writes the stamp only if HEAD and the tree were unchanged throughout the build. **Stopping a build part-way leaves no stamp.**
- On launch, `start-checkout` refreshes the installed mod from the guest template. The guest's embedded `SCHEMATICA_BUILD_REVISION` must equal the stamp.
- For a mod-only change, build the guest with `SCHEMATICA_BUILD_REVISION=$(git rev-parse HEAD)` and launch. Committing changes HEAD, so the stamp no longer matches and the launcher asks for a client rebuild.

## Mod commands

The mod is a separate Cargo workspace. Always pass `--manifest-path builder/schematica/Cargo.toml --target-dir target/builder-importer`. Running `cargo -p schematica-*` from the root fails.

```bash
# Guest tests
cargo test --locked --manifest-path builder/schematica/Cargo.toml --target-dir target/builder-importer --release -p schematica-guest

# Regenerate the precomputed window layout (guest/ui/browser.json) after layout changes
SCHEMATICA_BROWSER_REGENERATE=1 cargo test --locked --manifest-path builder/schematica/Cargo.toml --target-dir target/builder-importer --release -p schematica-guest precomputed

# Guest WASM
SCHEMATICA_BUILD_REVISION=$(git rev-parse HEAD) cargo build --locked --release --manifest-path builder/schematica/Cargo.toml --target-dir target/builder-importer -p schematica-guest --target wasm32-unknown-unknown

# Install a rebuilt guest into the profile without launching (start-builder.sh also does this)
SCHEMATICA_GUEST_TEMPLATE=target/builder-importer/wasm32-unknown-unknown/release/schematica_guest.wasm \
  target/builder-importer/release/schematica-builder refresh-component \
  .local/builder-user/data/mods/schematica-builder.wasm /tmp/refreshed.wasm \
  && cp /tmp/refreshed.wasm .local/builder-user/data/mods/schematica-builder.wasm
```

`start-builder.sh --check` prints whether the client and importer are built for HEAD.

## Window (P / M) constraints

- The JSON-UI layout is built by the test-only `mod layout` in `guest/src/browser.rs` and precomputed into `guest/ui/browser.json`. A test fails if the file is stale.
- The host refuses surface documents over **192 KiB**, and a test checks this. Draw button contents once, under a transparent hit button that holds only the hover and pressed overlays. Never copy contents into each state.
- Refreshes run inside the mod's per-frame WASM fuel budget. Precompute lookups and send only changed values. Check the client log for `fuel exhausted` after UI changes.
- The client also draws the window natively with OreUI (`crates/oreui/src/cinnamatica.rs`) from the same panel values. The JSON-UI window is the fallback. Keep the two consistent.

## Live testing

Use a client built with `--features cinnabar/developer-control`, plus `cinnabar-mcp` (`docs/agents/client-mcp.md`) and the local server (`bedrock-local-server`).
Launch with `headless: false` at size `[800, 450]`. Key presses can be dropped while the window is unfocused, so retry them before you assume a bug.
