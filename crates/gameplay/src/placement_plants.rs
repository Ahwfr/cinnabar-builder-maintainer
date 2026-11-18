//! Provisional normal-soil plant placement; specialized crops and aquatic plants stay unresolved.
use crate::placement_state::{only_keys, set_bit};
use serde_json::{Map, Value};

// This supported subset is checked against every pinned state below. New species
// are not silently admitted with another plant's soil or paired-cell behavior.
const SINGLE: &[&str] = &[
    "dandelion",
    "poppy",
    "blue_orchid",
    "allium",
    "azure_bluet",
    "red_tulip",
    "orange_tulip",
    "white_tulip",
    "pink_tulip",
    "oxeye_daisy",
    "cornflower",
    "lily_of_the_valley",
    "torchflower",
    "short_grass",
    "fern",
    "open_eyeblossom",
    "closed_eyeblossom",
];
const DOUBLE: &[&str] = &[
    "tall_grass",
    "large_fern",
    "sunflower",
    "lilac",
    "rose_bush",
    "peony",
];
const SAPLINGS: &[&str] = &[
    "oak_sapling",
    "spruce_sapling",
    "birch_sapling",
    "jungle_sapling",
    "acacia_sapling",
    "dark_oak_sapling",
    "cherry_sapling",
    "pale_oak_sapling",
];
const SOILS: &[&str] = &[
    "grass_block",
    "dirt",
    "coarse_dirt",
    "podzol",
    "dirt_with_roots",
    "moss_block",
    "pale_moss_block",
    "mud",
];

/// Whether this item belongs to the explicitly supported, registry-checked soil subset.
pub fn supported(name: &str) -> bool {
    name.strip_prefix("minecraft:").is_some_and(|name| {
        SINGLE.contains(&name) || DOUBLE.contains(&name) || SAPLINGS.contains(&name)
    })
}

/// Plants may replace air or a loaded replaceable block, but not an occupied solid.
pub fn replaceable(fact: assets::facts::BlockFact) -> bool {
    use assets::registry::BlockTraits as T;
    fact.readable && (fact.traits.contains(T::AIR) || fact.traits.contains(T::REPLACEABLE))
}

/// Returns the lower placement state and whether one item creates a second cell.
pub(crate) fn states(name: &str, canonical: &str) -> Option<(Map<String, Value>, bool)> {
    let name = name.strip_prefix("minecraft:")?;
    let mut states: Map<String, Value> = serde_json::from_str(canonical).ok()?;
    if SINGLE.contains(&name) && states.is_empty() {
        Some((states, false))
    } else if DOUBLE.contains(&name) && only_keys(&states, &["upper_block_bit"]) {
        set_bit(&mut states, "upper_block_bit", false)?;
        Some((states, true))
    } else if SAPLINGS.contains(&name) && only_keys(&states, &["age_bit"]) {
        set_bit(&mut states, "age_bit", false)?;
        Some((states, false))
    } else {
        None
    }
}

/// Soil-dependent plants cannot use an invented air support or arbitrary full cube.
pub(crate) fn accepts_soil(name: &str) -> bool {
    name.strip_prefix("minecraft:")
        .is_some_and(|name| SOILS.contains(&name))
}

/// The second cell of a supported two-block plant.
pub(crate) fn upper(lower: &Map<String, Value>) -> Option<Map<String, Value>> {
    let mut states = lower.clone();
    set_bit(&mut states, "upper_block_bit", true)?;
    Some(states)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn soil_plant_catalog_and_states_match_the_pinned_registry() {
        let records = assets::read_registry_for_protocol(
            assets::pinned_block_registry_bytes(),
            assets::active_content_registry_protocol(),
        )
        .unwrap();
        for name in SINGLE.iter().chain(DOUBLE).chain(SAPLINGS) {
            let name = format!("minecraft:{name}");
            let rows: Vec<_> = records
                .iter()
                .filter(|row| row.name.as_ref() == name)
                .collect();
            assert!(!rows.is_empty(), "missing plant {name}");
            for row in rows {
                let (state, double) = states(&name, &row.canonical_state).unwrap_or_else(|| {
                    panic!(
                        "unhandled pinned plant state {name}: {}",
                        row.canonical_state
                    )
                });
                assert_eq!(
                    double,
                    DOUBLE.contains(&name.trim_start_matches("minecraft:"))
                );
                if double {
                    assert_eq!(upper(&state).unwrap()["upper_block_bit"]["value"], 1);
                }
            }
        }
        for soil in SOILS {
            assert!(
                records
                    .iter()
                    .any(|row| row.name.as_ref() == format!("minecraft:{soil}")),
                "missing soil {soil}"
            );
        }
    }
    #[test]
    fn plant_support_rejects_air_stone_and_specialized_species() {
        for soil in [
            "minecraft:grass_block",
            "minecraft:dirt",
            "minecraft:moss_block",
        ] {
            assert!(accepts_soil(soil));
        }
        for soil in [
            "minecraft:air",
            "minecraft:stone",
            "minecraft:water",
            "minecraft:oak_slab",
        ] {
            assert!(!accepts_soil(soil));
        }
        for plant in [
            "minecraft:bamboo",
            "minecraft:cactus",
            "minecraft:seagrass",
            "minecraft:deadbush",
        ] {
            assert!(!supported(plant));
            assert!(states(plant, "{}").is_none());
        }
    }
}
