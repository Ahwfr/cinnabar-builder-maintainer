//! Portable Builder entry point and lossless library upgrades.

use anyhow::{Context, Result, ensure};
use schematica_core::blueprint;
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Resolves the matching template supplied by this release, never another checkout.
pub fn template() -> Result<Vec<u8>> {
    let root = env::current_exe()?
        .parent()
        .context("launcher folder")?
        .to_path_buf();
    let packaged = root.join("App/schematica-guest-template.bin");
    let path = env::var_os("SCHEMATICA_GUEST_TEMPLATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if packaged.is_file() {
                packaged
            } else {
                root.join("schematica-guest-template.bin")
            }
        });
    fs::read(&path).with_context(|| format!("Read the guest template at {}", path.display()))
}

/// Copies the old library payload into this release's guest, retaining every imported blueprint.
pub fn refresh(component: &[u8]) -> Result<Vec<u8>> {
    let template = template()?;
    refresh_with_template(component, &template)
}

fn refresh_with_template(component: &[u8], template: &[u8]) -> Result<Vec<u8>> {
    let payload = library_payload(component)?;
    super::component(&super::populate_template(template, &payload)?)
}

fn library_payload(component: &[u8]) -> Result<Vec<u8>> {
    let mut found = None;
    for section in wasmparser::Parser::new(0).parse_all(component) {
        if let wasmparser::Payload::DataSection(data) = section? {
            for segment in data {
                let segment = segment?;
                for (index, _) in segment
                    .data
                    .windows(blueprint::SLOT_MARKER.len())
                    .enumerate()
                    .filter(|(_, bytes)| *bytes == blueprint::SLOT_MARKER)
                {
                    let slot = &segment.data[index..];
                    if let Some(payload) = blueprint::embedded_payload(slot) {
                        blueprint::library(payload).map_err(anyhow::Error::msg)?;
                        ensure!(
                            found.is_none(),
                            "The old component contains ambiguous blueprint slots"
                        );
                        found = Some(payload.to_vec());
                    }
                }
            }
        }
    }
    found.context(
        "The old Schematica library has no supported embedded blueprint slot. Import its original files in game."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_encoder::{Component, ConstExpr, DataSection, Module, ModuleSection};

    /// The installed Linux client resolves its resources from `<prefix>/bin/../share/cinnabar`.
    #[cfg(target_os = "linux")]
    #[test]
    fn portable_linux_client_runs_from_a_bin_folder_inside_app() {
        let root = env::temp_dir().join(format!("cinnamatica-portable-{}", std::process::id()));
        fs::create_dir_all(root.join("App")).unwrap();
        let package = Package::portable(&root);
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(package.client, root.join("App/bin/cinnabar"));
        assert_eq!(package.working_dir, root.join("App/bin"));
    }

    #[test]
    fn nested_component_keeps_the_entire_imported_library_payload() -> Result<()> {
        let first = super::super::encode_blueprint(&super::super::demo())?;
        let payload = blueprint::encode_library(&[first.clone(), first]).unwrap();
        let slot = blueprint::embedded_slot(&payload).unwrap();
        let mut data = DataSection::new();
        data.active(0, &ConstExpr::i32_const(0), slot);
        let mut core = Module::new();
        core.section(&data);
        let mut component = Component::new();
        component.section(&ModuleSection(&core));
        assert_eq!(library_payload(&component.finish())?, payload);
        Ok(())
    }

    #[test]
    fn new_and_legacy_registrations_carry_the_request_id_the_client_requires() -> Result<()> {
        let installed = Path::new("/profile/data/mods/schematica-builder.wasm");
        let created = registered(None, installed)?;
        assert_eq!(created["request_id"], "builder");
        assert_eq!(created["mods"][0]["enabled"], true);

        let legacy = serde_json::json!({"version":2,"mods":[{"component":"/old/schematica-builder.wasm","enabled":false,"grants":{"commands":["say","fill","setblock"],"hud":true},"reload":3}]});
        let repaired = registered(Some(legacy), installed)?;
        assert_eq!(repaired["request_id"], "builder");
        assert_eq!(repaired["mods"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            repaired["mods"][0]["component"],
            installed.to_str().unwrap()
        );
        assert_eq!(repaired["mods"][0]["enabled"], false);
        assert_eq!(
            repaired["mods"][0]["grants"]["commands"],
            serde_json::json!(["fill", "setblock"])
        );

        let kept = serde_json::json!({"version":2,"request_id":"settings-7","mods":[]});
        assert_eq!(
            registered(Some(kept), installed)?["request_id"],
            "settings-7"
        );
        Ok(())
    }

    #[test]
    fn deleted_entries_free_their_slots_only_for_the_matching_revision() {
        let full = || (0..8u8).map(|n| vec![n]).collect::<Vec<_>>();
        let settings =
            serde_json::json!({"library_revision": 7, "deleted_entries": [2, 1, 0, 7, 4, 5]});

        let mut entries = full();
        assert!(prune_deleted(&mut entries, 7, Some(&settings)));
        assert_eq!(entries, vec![vec![3], vec![6]]);

        let mut stale = full();
        assert!(!prune_deleted(&mut stale, 8, Some(&settings)));
        assert_eq!(stale.len(), 8);

        let mut missing = full();
        assert!(!prune_deleted(&mut missing, 7, None));
        assert_eq!(missing.len(), 8);
    }
}

/// Publishes only complete components and first preserves the previous revision.
fn publish(mods: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(mods)?;
    let target = mods.join("schematica-builder.wasm");
    if target.is_file() && fs::read(&target)? == bytes {
        return Ok(());
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staged = mods.join(format!(
        ".schematica-{}-{nonce}.pending",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    use std::io::Write;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if target.is_file() {
        let backup = mods
            .parent()
            .context("mods data folder")?
            .join("schematic-backups");
        fs::create_dir_all(&backup)?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        fs::copy(&target, backup.join(format!("{stamp}.wasm")))?;
    }
    fs::rename(&staged, &target).context("Publish the complete schematic library")?;
    Ok(())
}

pub fn import_dialog(mods: &Path) -> Result<()> {
    let Some(inputs) = super::picker::choose()? else {
        return Ok(());
    };
    append_imports(mods, &inputs)?;
    println!("SCHEMATICA_IMPORTED");
    Ok(())
}

pub fn append_imports(mods: &Path, inputs: &[PathBuf]) -> Result<()> {
    let installed = mods.join("schematica-builder.wasm");
    let payload = if installed.is_file() {
        let file = fs::File::open(&installed)?;
        use std::io::Read as _;
        let mut existing = Vec::new();
        file.take(4 * 1024 * 1024 + 1).read_to_end(&mut existing)?;
        ensure!(
            existing.len() <= 4 * 1024 * 1024,
            "installed schematic component is too large"
        );
        library_payload(&existing)?
    } else {
        Vec::new()
    };
    let mut entries: Vec<_> = blueprint::library(&payload)
        .map_err(anyhow::Error::msg)?
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect();
    // Preserve existing indices and their saved placements and deletion preferences.
    let mut revision = blueprint::library_revision(&payload);
    let settings = fs::read_to_string(mods.join("schematica-builder.settings.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());
    if prune_deleted(&mut entries, revision, settings.as_ref()) {
        // Indices shifted, so a new revision makes the guest drop stale removals and placements.
        revision = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64;
    }
    super::add_inputs(&mut entries, inputs)?;
    let bytes = super::encode_component(&template()?, &entries, revision)?;
    publish(mods, &bytes)
}

/// Drops the entries the guest's settings mark as deleted for this library revision, so removed
/// schematics free their library slots. Returns whether any entry was dropped.
fn prune_deleted(
    entries: &mut Vec<Vec<u8>>,
    revision: u64,
    settings: Option<&serde_json::Value>,
) -> bool {
    let Some(settings) =
        settings.filter(|value| value["library_revision"].as_u64() == Some(revision))
    else {
        return false;
    };
    let deleted: Vec<u64> = settings["deleted_entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_u64)
        .collect();
    let before = entries.len();
    let mut index = 0;
    entries.retain(|_| {
        let keep = !deleted.contains(&index);
        index += 1;
        keep
    });
    entries.len() != before
}

/// The `App/` folder holding the client and its sibling runtime executables. An installed Linux
/// client needs `<prefix>/bin` beside its `<prefix>/share/cinnabar` resources.
const PORTABLE_RUNTIME: &str = if cfg!(target_os = "linux") {
    "bin"
} else {
    "runtime"
};

/// Where one build keeps the client, its working folder, guest template and build stamp.
pub struct Package {
    pub client: PathBuf,
    working_dir: PathBuf,
    template: PathBuf,
    revision: PathBuf,
    missing: &'static str,
}

impl Package {
    /// The extracted release layout, with the payload in `App/` beside the launcher.
    pub fn portable(root: &Path) -> Self {
        let package = if root.join("App").is_dir() {
            root.join("App")
        } else {
            root.to_path_buf()
        };
        let runtime = package.join(PORTABLE_RUNTIME);
        Self {
            client: runtime.join(format!("cinnabar{}", env::consts::EXE_SUFFIX)),
            working_dir: runtime,
            template: package.join("schematica-guest-template.bin"),
            revision: package.join("builder-commit.txt"),
            missing: "Keep the Cinnamatica launcher with the App folder from the same download.",
        }
    }

    /// A source checkout built by the Builder build script for one Cargo profile directory.
    pub fn checkout(checkout: &Path, profile_dir: &str) -> Self {
        let target = checkout.join("target");
        Self {
            client: target.join(format!("{profile_dir}/cinnabar{}", env::consts::EXE_SUFFIX)),
            working_dir: checkout.to_path_buf(),
            template: target
                .join("builder-importer/wasm32-unknown-unknown/release/schematica_guest.wasm"),
            revision: target.join(format!("{profile_dir}/builder-commit.txt")),
            missing: "Build this checkout with the Builder build script before launching.",
        }
    }
}

/// Installs or upgrades the library in the profile, registers it and starts the client.
///
/// `CINNABAR_USER_ROOT` overrides `default_profile`. Returns the running client.
pub fn start(package: &Package, default_profile: &Path) -> Result<std::process::Child> {
    ensure!(package.client.is_file(), package.missing);
    let template = fs::read(&package.template).with_context(|| {
        format!(
            "Read this build's Schematica guest template at {}",
            package.template.display()
        )
    })?;
    let revision = fs::read_to_string(&package.revision).context(package.missing)?;
    ensure!(
        super::guest_revision(&template)? == Some(revision.trim()),
        "The client and Schematica guest belong to different builds. Rebuild, or extract the complete release."
    );
    let profile = env::var_os("CINNABAR_USER_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_profile.to_path_buf());
    fs::create_dir_all(&profile)?;
    let profile = profile.canonicalize()?;
    let mods = profile.join("data/mods");
    let installed = mods.join("schematica-builder.wasm");
    let bytes = if installed.is_file() {
        refresh_with_template(&fs::read(&installed)?, &template)?
    } else {
        super::component(&template)?
    };
    publish(&mods, &bytes)?;
    registration(&profile, &installed)?;
    std::process::Command::new(&package.client)
        .current_dir(&package.working_dir)
        .env("CINNABAR_USER_ROOT", &profile)
        .env("CINNABAR_SCHEMATICA_IMPORTER", env::current_exe()?)
        .env("SCHEMATICA_GUEST_TEMPLATE", &package.template)
        .env("CINNABAR_UPDATE_CHECK", "0")
        .spawn()
        .context("Start Cinnamatica")
}

/// Preserve the enabled state and normal gameplay grants while retiring unused capabilities.
fn registration(profile: &Path, installed: &Path) -> Result<()> {
    let directory = profile.join("config");
    fs::create_dir_all(&directory)?;
    let path = directory.join("local-mod.json");
    let document = if path.is_file() {
        Some(serde_json::from_slice(&fs::read(&path)?)?)
    } else {
        None
    };
    let document = registered(document, installed)?;
    let staged = path.with_extension("next");
    fs::write(&staged, serde_json::to_vec(&document)?)?;
    fs::rename(staged, path)?;
    Ok(())
}

/// Adds or refreshes the library entry in a client mod list, creating a valid list when absent.
///
/// The client rejects a list without a request ID, which would leave every mod unloaded.
fn registered(document: Option<serde_json::Value>, installed: &Path) -> Result<serde_json::Value> {
    let mut document = document
        .unwrap_or_else(|| serde_json::json!({"version":2,"request_id":"builder","mods":[]}));
    if document["request_id"].as_str().is_none_or(str::is_empty) {
        document["request_id"] = serde_json::json!("builder");
    }
    let mods = document["mods"]
        .as_array_mut()
        .context("Invalid mod registration list")?;
    if let Some(entry) = mods.iter_mut().find(|entry| {
        entry["component"]
            .as_str()
            .and_then(|name| Path::new(name).file_name())
            == Some(std::ffi::OsStr::new("schematica-builder.wasm"))
    }) {
        entry["component"] = serde_json::json!(installed);
        if let Some(grants) = entry["grants"].as_object_mut() {
            for capability in ["wasi", "packets", "packet_delay"] {
                grants.remove(capability);
            }
            if let Some(commands) = grants
                .get_mut("commands")
                .and_then(serde_json::Value::as_array_mut)
            {
                commands.retain(|name| matches!(name.as_str(), Some("setblock" | "fill")));
            }
        }
    } else {
        let mut grants: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../guest/grants.json"))?;
        grants["commands"] = serde_json::json!([]);
        mods.push(serde_json::json!({
            "component":installed,"enabled":true,"grants":grants,"reload":0
        }));
    }
    Ok(document)
}
