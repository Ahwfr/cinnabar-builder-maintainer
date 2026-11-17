use super::{create_private_dir, write_private};
use serde_json::{Value, json};
use std::{fs, io, path::Path};

const ID: &str = "215637af-958a-4eab-88b4-10671dc74a38";
const MANIFEST: &str = include_str!("../../../../builder/structure-host/manifest.json");
const SCRIPT: &str = include_str!("../../../../builder/structure-host/scripts/main.js");

/// BDS reads the world's active pack list at startup. Preserve all other packs.
pub(super) fn install(world: &Path, server: &Path) -> io::Result<()> {
    let path = world.join("world_behavior_packs.json");
    let mut enabled: Vec<Value> = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let config = server
        .join("config")
        .join("cd8f1a22-ebd0-4d22-8bbc-a41cad532380");
    create_private_dir(&config)?;
    write_private(
        &config.join("permissions.json"),
        br#"{"allowed_modules":["@minecraft/server"]}"#,
    )?;
    let pack = world.join("behavior_packs").join(ID);
    create_private_dir(&pack.join("scripts"))?;
    write_private(&pack.join("manifest.json"), MANIFEST.as_bytes())?;
    write_private(&pack.join("scripts/main.js"), SCRIPT.as_bytes())?;
    let entry = json!({"pack_id": ID, "version": [1, 0, 0]});
    if enabled.iter().any(|value| *value == entry) {
        return Ok(());
    }
    enabled.retain(|value| value["pack_id"].as_str() != Some(ID));
    enabled.push(entry);
    let temporary = world.join("world_behavior_packs.cinnamatica.tmp");
    write_private(
        &temporary,
        &serde_json::to_vec_pretty(&enabled).map_err(io::Error::other)?,
    )?;
    fs::rename(temporary, path)
}
