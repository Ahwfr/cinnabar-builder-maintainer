use anyhow::{Context, Result, anyhow, bail, ensure};
use flate2::read::GzDecoder;
use nucleation::{
    UniversalSchematic, blockpedia,
    formats::{classic_schematic, limits::DecodeLimits, litematic, schematic},
    nbt::{
        Endian, NbtMap, NbtValue,
        io::{NbtReadLimits, read_nbt_with_limits},
    },
};
use schematica_core::{BlockState, Schematic, StateValue, blueprint};
use std::{borrow::Cow, collections::HashMap, fs, io::Read, path::Path};

const FILE_LIMIT: usize = 8 * 1024 * 1024;

/// Extensions with a dedicated decoder. Any other name is still accepted when its contents
/// match a supported format.
const KNOWN_EXTENSIONS: [&str; 6] = ["mcstructure", "nbt", "litematic", "schem", "schematic", "gz"];

pub fn import(path: &Path) -> Result<Schematic> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(FILE_LIMIT as u64 + 1)
        .read_to_end(&mut data)?;
    ensure!(data.len() <= FILE_LIMIT, "input exceeds 8 MiB");
    let name = path.file_stem().unwrap_or_default().to_string_lossy();
    let name: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != '§')
        .take(32)
        .collect();
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    import_bytes(&data, name, &ext)
}

fn import_bytes(data: &[u8], name: String, ext: &str) -> Result<Schematic> {
    let primary = match ext {
        "mcstructure" => import_bedrock(data, name.clone()),
        "nbt" | "gz" => gunzip(data).and_then(|plain| import_java_structure(&plain, name.clone())),
        "litematic" | "schem" | "schematic" => {
            parse_java(ext, data).and_then(|java| from_java(java, name.clone()))
        }
        _ => Err(anyhow!("unknown extension .{ext}")),
    };
    match primary {
        Ok(schematic) => Ok(schematic),
        Err(error) => match sniff(data, name) {
            Ok(schematic) => Ok(schematic),
            // A known extension with broken contents reports its own decoder's error.
            Err(sniffed) if KNOWN_EXTENSIONS.contains(&ext) => {
                let _ = sniffed;
                Err(error)
            }
            Err(sniffed) => Err(sniffed),
        },
    }
}

/// Tries every supported format in turn, so a file with any name still imports.
fn sniff(data: &[u8], name: String) -> Result<Schematic> {
    let attempts: [(&str, fn(&[u8], String) -> Result<Schematic>); 5] = [
        ("Bedrock structure", |data, name| import_bedrock(data, name)),
        ("Java structure", |data, name| {
            import_java_structure(&gunzip(data)?, name)
        }),
        ("Litematica", |data, name| {
            from_java(parse_java("litematic", data)?, name)
        }),
        ("Sponge schematic", |data, name| {
            from_java(parse_java("schem", data)?, name)
        }),
        ("MCEdit schematic", |data, name| {
            from_java(parse_java("schematic", data)?, name)
        }),
    ];
    let mut errors = Vec::new();
    for (label, attempt) in attempts {
        match attempt(data, name.clone()) {
            Ok(schematic) => return Ok(schematic),
            Err(error) => errors.push(format!("{label}: {error:#}")),
        }
    }
    bail!(
        "not a recognized Minecraft structure or schematic ({})",
        errors.join("; ")
    )
}

/// Returns the bytes unchanged unless they are gzip-compressed, as Java structures are.
fn gunzip(data: &[u8]) -> Result<Cow<'_, [u8]>> {
    if !data.starts_with(&[0x1f, 0x8b]) {
        return Ok(Cow::Borrowed(data));
    }
    let mut plain = Vec::new();
    GzDecoder::new(data)
        .take(FILE_LIMIT as u64 + 1)
        .read_to_end(&mut plain)
        .context("decompress gzip input")?;
    ensure!(plain.len() <= FILE_LIMIT, "decompressed input exceeds 8 MiB");
    Ok(Cow::Owned(plain))
}

/// Java decoders expect gzip. Some tools save the same NBT uncompressed, so compress it here.
fn gzipped(data: &[u8]) -> Result<Cow<'_, [u8]>> {
    use std::io::Write;
    if data.starts_with(&[0x1f, 0x8b]) {
        return Ok(Cow::Borrowed(data));
    }
    let mut out = Vec::new();
    let mut encoder = flate2::write::GzEncoder::new(&mut out, flate2::Compression::fast());
    encoder.write_all(data)?;
    encoder.finish()?;
    Ok(Cow::Owned(out))
}

fn parse_java(ext: &str, data: &[u8]) -> Result<UniversalSchematic> {
    let data = &*gzipped(data)?;
    let limits = DecodeLimits {
        max_input_bytes: FILE_LIMIT,
        max_decompressed_bytes: FILE_LIMIT,
        max_dimension: blueprint::MAX_EDGE as usize,
        max_volume: blueprint::MAX_CELLS,
        max_regions: 64,
        max_palette_entries: 8192,
        max_entities: 1024,
        max_block_entities: blueprint::MAX_CELLS,
        max_nbt_depth: 32,
        max_nbt_string_bytes: 32 * 1024,
        max_nbt_collection_items: blueprint::MAX_CELLS * 2,
        max_nbt_nodes: 262_144,
    };
    Ok(match ext {
        "litematic" => litematic::from_litematic_bounded(data, &limits)?,
        "schem" => schematic::from_schematic_bounded(data, &limits)?,
        "schematic" => classic_schematic::from_classic_schematic_bounded(data, &limits)?,
        _ => bail!("expected .mcstructure, .litematic, .schem, .schematic or .nbt"),
    })
}

fn from_java(java: UniversalSchematic, name: String) -> Result<Schematic> {
    let bounds = java
        .get_schematic_bounding_box()
        .context("empty Java schematic")?;
    let min = [bounds.min.0, bounds.min.1, bounds.min.2];
    let max = [bounds.max.0, bounds.max.1, bounds.max.2];
    let [sx, sy, sz] =
        [0, 1, 2].map(|i| i32::try_from(i64::from(max[i]) - i64::from(min[i]) + 1).unwrap_or(0));
    blueprint::validate_size([sx, sy, sz]).map_err(anyhow::Error::msg)?;
    let mut out = Schematic::new(name, [sx, sy, sz]);
    let mut translations = HashMap::new();
    for y in 0..sy {
        for z in 0..sz {
            for x in 0..sx {
                let Some(state) = java.get_block(min[0] + x, min[1] + y, min[2] + z) else {
                    continue;
                };
                if state.get_name() == "minecraft:structure_void" {
                    out.set([x, y, z], BlockState::new(state.get_name()));
                    continue;
                }
                if matches!(
                    state.get_name(),
                    "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                ) {
                    continue;
                }
                let key = state.to_string();
                if !translations.contains_key(&key) {
                    translations.insert(key.clone(), translate_java_block(&key)?);
                }
                out.set([x, y, z], translations[&key].clone());
            }
        }
    }
    Ok(out)
}

/// Maps one Java block state string such as `minecraft:oak_stairs[facing=east]` to Bedrock.
fn translate_java_block(key: &str) -> Result<BlockState> {
    let block = key.split('[').next().unwrap_or(key);
    // Flattened Java exports before 1.20.3 called short grass `grass`. The classic decoder
    // already maps numeric turf to `grass_block`.
    let renamed_chain;
    let java_state = if key == "minecraft:grass" {
        "minecraft:short_grass"
    } else if block == "minecraft:chain" {
        renamed_chain = key.replacen("minecraft:chain", "minecraft:iron_chain", 1);
        &renamed_chain
    } else {
        key
    };
    let source = blockpedia::BlockState::parse(java_state)
        .map_err(|e| anyhow!("cannot parse {key}: {e}"))?;
    let translated = source
        .to_bedrock()
        .map_err(|e| anyhow!("cannot translate {key} to Bedrock: {e}"))?;
    let mut mapped = BlockState::new(translated.id());
    for (property, value) in translated.properties() {
        mapped
            .states
            .insert(property.to_string(), StateValue::parse(value));
    }
    if mapped.name.ends_with("glass_pane") || mapped.name == "minecraft:iron_bars" {
        for (direction, value) in source.properties() {
            if matches!(direction.as_str(), "north" | "east" | "south" | "west")
                && matches!(value.as_str(), "true" | "false")
            {
                mapped.states.insert(
                    format!("minecraft:connection_{direction}"),
                    StateValue::Bool(value == "true"),
                );
            }
        }
    }
    Ok(mapped)
}

/// Reads a Java Edition structure (`.nbt`): a gzipped, big-endian NBT file with a block palette.
fn import_java_structure(data: &[u8], name: String) -> Result<Schematic> {
    let mut bytes = data;
    let root = read_nbt_with_limits(
        &mut bytes,
        Endian::Big,
        NbtReadLimits {
            max_depth: 32,
            max_string_bytes: 32 * 1024,
            max_collection_items: blueprint::MAX_CELLS,
            max_nodes: 262_144,
        },
    )
    .context("read big-endian Java structure NBT")?;
    let root = compound(&root)?;
    let size = ints3(root.get("size").context("missing size")?)?;
    let [sx, sy, sz] = size;
    blueprint::validate_size(size).map_err(anyhow::Error::msg)?;
    let palette: Vec<Option<BlockState>> = list(root.get("palette").context("missing palette")?)?
        .iter()
        .map(|entry| {
            let entry = compound(entry)?;
            let Some(NbtValue::String(name)) = entry.get("Name") else {
                bail!("palette entry has no name")
            };
            if matches!(
                name.as_str(),
                "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
            ) {
                return Ok(None);
            }
            if name == "minecraft:structure_void" {
                return Ok(Some(BlockState::new(name)));
            }
            translate_java_block(&java_state_key(name, entry)?).map(Some)
        })
        .collect::<Result<_>>()?;
    let mut out = Schematic::new(name, size);
    for block in list(root.get("blocks").context("missing blocks")?)? {
        let block = compound(block)?;
        let pos = ints3(block.get("pos").context("block has no position")?)?;
        ensure!(
            pos.iter().zip(size).all(|(p, s)| (0..s).contains(p)),
            "block at {pos:?} lies outside the {sx}x{sy}x{sz} structure"
        );
        let Some(NbtValue::Int(index)) = block.get("state") else {
            bail!("block at {pos:?} has no palette state")
        };
        let index = usize::try_from(*index).context("negative palette index")?;
        let state = palette
            .get(index)
            .with_context(|| format!("invalid palette index {index}"))?;
        if let Some(state) = state {
            out.set(pos, state.clone());
        }
    }
    Ok(out)
}

/// Joins a palette entry's name and sorted properties into `name[key=value,...]`.
fn java_state_key(name: &str, entry: &NbtMap) -> Result<String> {
    let Some(properties) = entry.get("Properties") else {
        return Ok(name.to_owned());
    };
    let mut pairs = compound(properties)?
        .iter()
        .map(|(key, value)| match value {
            NbtValue::String(value) => Ok(format!("{key}={value}")),
            _ => bail!("property {key} of {name} is not text"),
        })
        .collect::<Result<Vec<_>>>()?;
    if pairs.is_empty() {
        return Ok(name.to_owned());
    }
    pairs.sort();
    Ok(format!("{name}[{}]", pairs.join(",")))
}

fn ints3(tag: &NbtValue) -> Result<[i32; 3]> {
    match tag {
        NbtValue::IntArray(values) if values.len() == 3 => Ok([values[0], values[1], values[2]]),
        _ => match list(tag)? {
            [NbtValue::Int(x), NbtValue::Int(y), NbtValue::Int(z)] => Ok([*x, *y, *z]),
            _ => bail!("expected three integer coordinates"),
        },
    }
}

fn import_bedrock(mut bytes: &[u8], name: String) -> Result<Schematic> {
    let root = read_nbt_with_limits(
        &mut bytes,
        Endian::Little,
        NbtReadLimits {
            max_depth: 32,
            max_string_bytes: 32 * 1024,
            max_collection_items: blueprint::MAX_CELLS,
            max_nodes: 262_144,
        },
    )
    .context("read little-endian Bedrock structure NBT")?;
    let root = compound(&root)?;
    // Tools write the version as a byte, short, int or long, or leave it out; only a file
    // without the structure layout is refused. Another version is tried as version 1 anyway.
    let version = match root.get("format_version") {
        Some(NbtValue::Byte(v)) => Some(i64::from(*v)),
        Some(NbtValue::Short(v)) => Some(i64::from(*v)),
        Some(NbtValue::Int(v)) => Some(i64::from(*v)),
        Some(NbtValue::Long(v)) => Some(*v),
        _ => None,
    };
    if version.is_some_and(|version| version != 1) {
        eprintln!("Warning: Bedrock structure format version {version:?} is read as version 1.");
    }
    ensure!(
        root.get("structure").is_some() && root.get("size").is_some(),
        "not a Bedrock structure: it has no size or structure section"
    );
    let size = list(root.get("size").context("missing size")?)?;
    let [NbtValue::Int(sx), NbtValue::Int(sy), NbtValue::Int(sz)] = size else {
        bail!("expected three integer structure dimensions");
    };
    let [sx, sy, sz] = [*sx, *sy, *sz];
    let volume = blueprint::validate_size([sx, sy, sz]).map_err(anyhow::Error::msg)?;
    // Newer tools store the structure section as a compressed byte array instead of a compound.
    let inflated = bedrock_structure_section(root.get("structure").context("missing structure")?)?;
    let structure = compound(&inflated)?;
    let palette = compound(structure.get("palette").context("missing palette")?)?;
    let palette = compound(palette.get("default").context("missing default palette")?)?;
    let palette = list(
        palette
            .get("block_palette")
            .context("missing block palette")?,
    )?;
    let palette: Vec<_> = palette.iter().map(bedrock_state).collect::<Result<_>>()?;
    let layers = list(
        structure
            .get("block_indices")
            .context("missing block indices")?,
    )?;
    let primary = block_layer(layers.first().context("missing primary block layer")?)?;
    ensure!(
        primary.len() == volume,
        "primary block layer length differs from size"
    );
    if let Some(layer) = layers.get(1) {
        let layer = block_layer(layer)?;
        ensure!(
            layer.len() == volume,
            "secondary block layer length differs from size"
        );
        if layer.iter().any(|index| *index != -1) {
            eprintln!("Warning: the secondary block layer (such as waterlogging) is omitted.");
        }
    }
    let mut out = Schematic::new(name, [sx, sy, sz]);
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let index = ((x * sy + y) * sz + z) as usize;
                match primary[index] {
                    -1 => {
                        out.set([x, y, z], BlockState::new("minecraft:structure_void"));
                    }
                    i => {
                        let state = usize::try_from(i)
                            .ok()
                            .and_then(|i| palette.get(i))
                            .with_context(|| format!("invalid palette index {i}"))?;
                        if state.name != "minecraft:air" {
                            out.set([x, y, z], state.clone());
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The decoded size of a compressed structure section is bounded: the largest blueprint is
/// 4 bytes a cell, with room for the palette and the second layer.
const SECTION_LIMIT: u64 = 64 * 1024 * 1024;

/// The `structure` section of a Bedrock structure: a compound, or the same compound as
/// compressed little-endian NBT bytes (zlib, gzip or raw deflate) in a byte array.
fn bedrock_structure_section(tag: &NbtValue) -> Result<NbtValue> {
    let NbtValue::ByteArray(bytes) = tag else {
        return Ok(tag.clone());
    };
    let bytes: Vec<u8> = bytes.iter().map(|byte| *byte as u8).collect();
    let mut plain = Vec::new();
    if bytes.starts_with(&[0x1f, 0x8b]) {
        GzDecoder::new(&bytes[..])
            .take(SECTION_LIMIT + 1)
            .read_to_end(&mut plain)
            .context("decompress gzip structure section")?;
    } else if bytes.first() == Some(&0x0a) {
        // Not compressed: it already starts with a compound tag.
        plain = bytes;
    } else {
        let zlib = flate2::read::ZlibDecoder::new(&bytes[..])
            .take(SECTION_LIMIT + 1)
            .read_to_end(&mut plain);
        if zlib.is_err() {
            plain.clear();
            flate2::read::DeflateDecoder::new(&bytes[..])
                .take(SECTION_LIMIT + 1)
                .read_to_end(&mut plain)
                .context("decompress the structure section")?;
        }
    }
    ensure!(
        plain.len() as u64 <= SECTION_LIMIT,
        "decompressed structure section exceeds 64 MiB"
    );
    let mut cursor = &plain[..];
    read_nbt_with_limits(
        &mut cursor,
        Endian::Little,
        NbtReadLimits {
            max_depth: 32,
            max_string_bytes: 32 * 1024,
            max_collection_items: blueprint::MAX_CELLS,
            max_nodes: 262_144,
        },
    )
    .context("read the compressed Bedrock structure section")
}

/// One block layer: a list of ints, or an int array in newer files.
fn block_layer(tag: &NbtValue) -> Result<Vec<i32>> {
    match tag {
        NbtValue::IntArray(values) => Ok(values.clone()),
        NbtValue::List(values) => values
            .iter()
            .enumerate()
            .map(|(cell, value)| match value {
                NbtValue::Int(index) => Ok(*index),
                _ => bail!("expected integer block index at cell {cell}"),
            })
            .collect(),
        _ => bail!("expected a block layer list"),
    }
}

fn bedrock_state(tag: &NbtValue) -> Result<BlockState> {
    let entry = compound(tag)?;
    let Some(NbtValue::String(name)) = entry.get("name") else {
        bail!("block has no name")
    };
    let mut block = BlockState::new(name);
    for (key, tag) in compound(entry.get("states").context("block has no state object")?)?.iter() {
        let value = match tag {
            NbtValue::Byte(0) => StateValue::Bool(false),
            NbtValue::Byte(1) => StateValue::Bool(true),
            NbtValue::Byte(v) => StateValue::Byte(*v),
            NbtValue::Int(v) => StateValue::Int(*v),
            NbtValue::String(v) => StateValue::Str(v.clone()),
            _ => bail!("unsupported NBT state type for {name}.{key}"),
        };
        block.states.insert(key.clone(), value);
    }
    Ok(block)
}

fn compound(tag: &NbtValue) -> Result<&nucleation::nbt::NbtMap> {
    match tag {
        NbtValue::Compound(v) => Ok(v),
        _ => bail!("expected an NBT compound"),
    }
}

fn list(tag: &NbtValue) -> Result<&[NbtValue]> {
    match tag {
        NbtValue::List(v) => Ok(v),
        _ => bail!("expected an NBT list"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleation::{UniversalSchematic, formats::mcstructure};

    #[test]
    fn old_java_grass_imports_as_short_grass() -> Result<()> {
        let mut source = UniversalSchematic::new("old grass".into());
        source.metadata.mc_version = Some(3578);
        assert!(source.set_block_str(0, 0, 0, "minecraft:grass"));
        assert!(source.set_block_str(1, 0, 0, "minecraft:grass_block"));
        let bytes = litematic::to_litematic(&source)?;
        let imported = import_bytes(&bytes, "old grass".into(), "litematic")?;
        assert_eq!(
            imported.get([0, 0, 0]).unwrap().name,
            "minecraft:short_grass"
        );
        assert_eq!(
            imported.get([1, 0, 0]).unwrap().name,
            "minecraft:grass_block"
        );
        Ok(())
    }

    #[test]
    fn import_preserves_asymmetric_coordinates_and_states() -> Result<()> {
        let mut source = UniversalSchematic::new("fixture".into());
        assert!(source.set_block_str(0, 0, 0, "minecraft:stone"));
        assert!(source.set_block_str(
            1,
            2,
            3,
            "minecraft:oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]"
        ));
        let cases = [
            ("litematic", litematic::to_litematic(&source)?),
            ("schem", schematic::to_schematic(&source)?),
            ("mcstructure", mcstructure::to_mcstructure(&source)?),
        ];
        for (extension, data) in cases {
            let output = import_bytes(&data, extension.to_owned(), extension)?;
            assert_eq!(output.size(), [2, 3, 4], "{extension}");
            assert_eq!(output.block_count(), 2, "{extension}");
            assert_eq!(
                output.get([0, 0, 0]).unwrap().name,
                "minecraft:stone",
                "{extension}"
            );
            let stair = output.get([1, 2, 3]).unwrap();
            assert_eq!(stair.name, "minecraft:oak_stairs", "{extension}");
            assert_eq!(
                stair.states["upside_down_bit"],
                StateValue::Bool(true),
                "{extension}"
            );
            assert_eq!(
                stair.states["weirdo_direction"],
                StateValue::Int(3),
                "{extension}"
            );
        }
        Ok(())
    }

    #[test]
    fn java_structure_imports_by_extension_and_by_content() -> Result<()> {
        use std::io::Write;
        let text = |value: &str| NbtValue::String(value.into());
        let mut stairs = NbtMap::new();
        stairs.insert("Name".into(), text("minecraft:oak_stairs"));
        let mut properties = NbtMap::new();
        for (key, value) in [
            ("facing", "north"),
            ("half", "top"),
            ("shape", "straight"),
            ("waterlogged", "false"),
        ] {
            properties.insert(key.into(), text(value));
        }
        stairs.insert("Properties".into(), NbtValue::Compound(properties));
        let mut air = NbtMap::new();
        air.insert("Name".into(), text("minecraft:air"));
        let mut stone = NbtMap::new();
        stone.insert("Name".into(), text("minecraft:stone"));
        let block = |pos: [i32; 3], state: i32| {
            let mut block = NbtMap::new();
            block.insert(
                "pos".into(),
                NbtValue::List(pos.into_iter().map(NbtValue::Int).collect()),
            );
            block.insert("state".into(), NbtValue::Int(state));
            NbtValue::Compound(block)
        };
        let mut root = NbtMap::new();
        root.insert(
            "size".into(),
            NbtValue::List(vec![NbtValue::Int(2), NbtValue::Int(3), NbtValue::Int(4)]),
        );
        root.insert(
            "palette".into(),
            NbtValue::List(vec![
                NbtValue::Compound(air),
                NbtValue::Compound(stone),
                NbtValue::Compound(stairs),
            ]),
        );
        root.insert(
            "blocks".into(),
            NbtValue::List(vec![block([0, 0, 0], 1), block([1, 2, 3], 2), block([1, 0, 0], 0)]),
        );
        let mut plain = Vec::new();
        nucleation::nbt::io::write_nbt(&mut plain, &root, "", Endian::Big)?;
        let mut gzipped = Vec::new();
        let mut encoder = flate2::write::GzEncoder::new(&mut gzipped, flate2::Compression::default());
        encoder.write_all(&plain)?;
        encoder.finish()?;

        for (data, ext) in [(&gzipped, "nbt"), (&gzipped, "bin"), (&plain, "nbt")] {
            let output = import_bytes(data, "java".into(), ext)?;
            assert_eq!(output.size(), [2, 3, 4], "{ext}");
            assert_eq!(output.block_count(), 2, "{ext}");
            assert_eq!(output.get([0, 0, 0]).unwrap().name, "minecraft:stone", "{ext}");
            let stairs = output.get([1, 2, 3]).unwrap();
            assert_eq!(stairs.name, "minecraft:oak_stairs", "{ext}");
        }
        Ok(())
    }

    #[test]
    fn a_bedrock_structure_imports_whatever_type_its_version_is_stored_in() -> Result<()> {
        let text = |value: &str| NbtValue::String(value.into());
        let state = |name: &str| {
            let mut entry = NbtMap::new();
            entry.insert("name".into(), text(name));
            entry.insert("states".into(), NbtValue::Compound(NbtMap::new()));
            NbtValue::Compound(entry)
        };
        let build = |version: Option<NbtValue>| -> Result<Vec<u8>> {
            let mut default = NbtMap::new();
            default.insert(
                "block_palette".into(),
                NbtValue::List(vec![state("minecraft:oak_planks"), state("minecraft:dirt")]),
            );
            let mut palette = NbtMap::new();
            palette.insert("default".into(), NbtValue::Compound(default));
            let mut structure = NbtMap::new();
            structure.insert(
                "block_indices".into(),
                NbtValue::List(vec![
                    NbtValue::List(vec![NbtValue::Int(0), NbtValue::Int(1)]),
                    NbtValue::List(vec![NbtValue::Int(-1), NbtValue::Int(-1)]),
                ]),
            );
            structure.insert("palette".into(), NbtValue::Compound(palette));
            let mut root = NbtMap::new();
            if let Some(version) = version {
                root.insert("format_version".into(), version);
            }
            root.insert(
                "size".into(),
                NbtValue::List(vec![NbtValue::Int(1), NbtValue::Int(2), NbtValue::Int(1)]),
            );
            root.insert("structure".into(), NbtValue::Compound(structure));
            let mut data = Vec::new();
            nucleation::nbt::io::write_nbt(&mut data, &root, "", Endian::Little)?;
            Ok(data)
        };
        for version in [
            Some(NbtValue::Int(1)),
            Some(NbtValue::Byte(1)),
            Some(NbtValue::Short(1)),
            Some(NbtValue::Long(1)),
            Some(NbtValue::Int(2)),
            None,
        ] {
            let data = build(version.clone())?;
            let output = import_bytes(&data, "bedrock".into(), "mcstructure")?;
            assert_eq!(output.size(), [1, 2, 1], "{version:?}");
            assert_eq!(output.get([0, 0, 0]).unwrap().name, "minecraft:oak_planks");
            assert_eq!(output.get([0, 1, 0]).unwrap().name, "minecraft:dirt");
        }
        Ok(())
    }

    #[test]
    fn a_compressed_version_2_structure_imports() -> Result<()> {
        // A real file whose structure section is zlib-compressed and whose block layer is an
        // int array: 10 x 6 x 11 cells, 519 of them air.
        let data = include_bytes!("testdata/compressed-v2.mcstructure");
        let output = import_bytes(data, "demo".into(), "mcstructure")?;
        assert_eq!(output.size(), [10, 6, 11]);
        assert_eq!(output.block_count(), 660 - 519);
        let mut names = std::collections::BTreeSet::new();
        for x in 0..10 {
            for y in 0..6 {
                for z in 0..11 {
                    if let Some(block) = output.get([x, y, z]) {
                        names.insert(block.name.clone());
                    }
                }
            }
        }
        assert!(names.contains("minecraft:oak_log"));
        assert!(names.contains("minecraft:oak_fence"));
        assert!(names.contains("minecraft:dirt"));
        Ok(())
    }

    #[test]
    fn a_compressed_section_may_be_gzip_raw_deflate_or_plain() -> Result<()> {
        use std::io::Write;
        let data = include_bytes!("testdata/compressed-v2.mcstructure");
        let mut bytes = &data[..];
        let root = read_nbt_with_limits(
            &mut bytes,
            Endian::Little,
            NbtReadLimits {
                max_depth: 32,
                max_string_bytes: 32 * 1024,
                max_collection_items: blueprint::MAX_CELLS,
                max_nodes: 262_144,
            },
        )?;
        let NbtValue::Compound(mut root) = root else {
            bail!("root is not a compound");
        };
        let Some(NbtValue::ByteArray(zlib)) = root.get("structure").cloned() else {
            bail!("structure is not a byte array");
        };
        let zlib: Vec<u8> = zlib.iter().map(|byte| *byte as u8).collect();
        let mut plain = Vec::new();
        flate2::read::ZlibDecoder::new(&zlib[..]).read_to_end(&mut plain)?;
        let mut encode = |payload: Vec<u8>| -> Result<Vec<u8>> {
            root.insert(
                "structure".into(),
                NbtValue::ByteArray(payload.iter().map(|byte| *byte as i8).collect()),
            );
            let mut out = Vec::new();
            nucleation::nbt::io::write_nbt(&mut out, &root, "", Endian::Little)?;
            Ok(out)
        };
        let mut gzip = Vec::new();
        let mut encoder = flate2::write::GzEncoder::new(&mut gzip, flate2::Compression::fast());
        encoder.write_all(&plain)?;
        encoder.finish()?;
        let mut raw = Vec::new();
        let mut encoder = flate2::write::DeflateEncoder::new(&mut raw, flate2::Compression::fast());
        encoder.write_all(&plain)?;
        encoder.finish()?;
        for (label, payload) in [("gzip", gzip), ("raw deflate", raw), ("plain", plain.clone())] {
            let output = import_bytes(&encode(payload)?, "demo".into(), "mcstructure")?;
            assert_eq!(output.size(), [10, 6, 11], "{label}");
            assert_eq!(output.block_count(), 141, "{label}");
        }
        Ok(())
    }

    #[test]
    fn an_uncompressed_litematic_imports() -> Result<()> {
        let mut source = UniversalSchematic::new("plain".into());
        assert!(source.set_block_str(0, 0, 0, "minecraft:stone"));
        let zipped = litematic::to_litematic(&source)?;
        let plain = gunzip(&zipped)?.into_owned();
        assert!(!plain.starts_with(&[0x1f, 0x8b]));
        let output = import_bytes(&plain, "plain".into(), "litematic")?;
        assert_eq!(output.get([0, 0, 0]).unwrap().name, "minecraft:stone");
        Ok(())
    }

    #[test]
    fn a_misnamed_litematic_still_imports() -> Result<()> {
        let mut source = UniversalSchematic::new("misnamed".into());
        assert!(source.set_block_str(0, 0, 0, "minecraft:stone"));
        let data = litematic::to_litematic(&source)?;
        let output = import_bytes(&data, "misnamed".into(), "dat")?;
        assert_eq!(output.get([0, 0, 0]).unwrap().name, "minecraft:stone");
        Ok(())
    }
}
