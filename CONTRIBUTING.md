# Contributing to Builder

Start with `git status --short`, then make a branch from the current Builder base.
Keep one issue per change and preserve upstream notices. Include the commit, platform,
Bedrock version, settings, expected behavior and reproduction steps in reports.
Keep account information, private server addresses, worlds and imported schematics
out of reports and fixtures.

Rust is pinned in `rust-toolchain.toml` (1.99.0 in this snapshot, with rustfmt,
Clippy and wasm32-unknown-unknown). The Go core pins 1.26.1 in `core/go.mod`.
Cargo.lock, the separate Builder Cargo.lock, and core/go.sum pin dependency resolution.
GNU Make orchestrates the upstream build. On Windows the Builder launcher and
packaging helpers use PowerShell and the standard Windows Rust linker toolchain.

The following commands passed on Windows with a populated Cargo cache. For a fresh
checkout, omit `--offline` if dependency download is needed. Native runtime support
also needs the Go binaries from the same core revision and locally prepared game
resources. Follow the upstream README and the Builder guide for their setup.

```sh
cargo test --locked --release --manifest-path builder/schematica/Cargo.toml -p schematica-guest
cargo build --locked --release --manifest-path builder/schematica/Cargo.toml -p schematica-guest --target wasm32-unknown-unknown
cargo build --locked --profile play -p cinnabar -p update-installer -p cinnabar-cxb -p shaderpack-worker -p asset-compiler --bins
```

Run targeted checks before opening a PR and document what ran. For placement changes,
test Creative and Survival separately and record server confirmation and inventory
changes. A successful compile or a client prediction is insufficient acceptance.
The upstream workflows are retained for reference. The original Builder Windows
workflow is scoped to the original repository. Configure review-only CI for a new
remote before enabling hosted releases.

The recorded common upstream commit is
`2eb39e9426fc4c994548aac990c7b466cf75228a`. Fetch `upstream/dev` before comparing
changes. Test upstream updates on a disposable branch.
Update documentation, protocol pins and fixtures together when changing the target.
