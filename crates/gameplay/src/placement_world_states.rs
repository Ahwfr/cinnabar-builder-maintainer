//! Which block-state properties the world sets after placement, so placing ignores them.
//!
//! Power, redstone signal, lit and pulse bits, connections, open and pressed state, growth and
//! age, leaf decay, farmland moisture, scaffolding stability, dripleaf tilt and container
//! contents all follow from the world or later use. A placement prediction never produces
//! them, so a guide that records them must not block the placement of the plain block. Every
//! orientation property (facing, direction, half, axis, rotation) stays placement-controlled.

use assets::facts::BlockProperty;

/// Whether the world, not the placement, sets `property`. The match is exhaustive over the
/// generated property list, so a new property fails to build until it is classified here.
pub const fn world_controlled(property: BlockProperty) -> bool {
    use BlockProperty as P;
    match property {
        P::Age
        | P::AgeBit
        | P::AttachedBit
        | P::BambooLeafSize
        | P::BambooStalkThickness
        | P::BigDripleafTilt
        | P::BiteCounter
        | P::BooksStored
        | P::BrewingStandSlotABit
        | P::BrewingStandSlotBBit
        | P::BrewingStandSlotCBit
        | P::BrushedProgress
        | P::ButtonPressedBit
        | P::CauldronLiquid
        | P::ComposterFillLevel
        | P::CrackedState
        | P::Crafting
        | P::DisarmedBit
        | P::DripstoneThickness
        | P::EndPortalEyeBit
        | P::FillLevel
        | P::GrowingPlantAge
        | P::Growth
        | P::HoneyLevel
        | P::InWallBit
        | P::KelpAge
        | P::Lit
        | P::MinecraftConnectionDown
        | P::MinecraftConnectionEast
        | P::MinecraftConnectionNorth
        | P::MinecraftConnectionSouth
        | P::MinecraftConnectionUp
        | P::MinecraftConnectionWest
        | P::MoisturizedAmount
        | P::OccupiedBit
        | P::OpenBit
        | P::OutputLitBit
        | P::OutputSubtractBit
        | P::PaleMossCarpetSideEast
        | P::PaleMossCarpetSideNorth
        | P::PaleMossCarpetSideSouth
        | P::PaleMossCarpetSideWest
        | P::PersistentBit
        | P::PoweredBit
        | P::PropaguleStage
        | P::RailDataBit
        | P::RailDirection
        | P::RedstoneEast
        | P::RedstoneNorth
        | P::RedstoneSignal
        | P::RedstoneSouth
        | P::RedstoneWest
        | P::RepeaterDelay
        | P::RespawnAnchorCharge
        | P::SculkSensorPhase
        | P::Stability
        | P::StabilityCheck
        | P::SuspendedBit
        | P::ToggleBit
        | P::TriggeredBit
        | P::TwistingVinesAge
        | P::UpdateBit
        | P::WallConnectionTypeEast
        | P::WallConnectionTypeNorth
        | P::WallConnectionTypeSouth
        | P::WallConnectionTypeWest
        | P::WallPostBit
        | P::WeepingVinesAge => true,
        P::Active
        | P::AllowUnderwaterBit
        | P::Attachment
        | P::BigDripleafHead
        | P::BlockLightLevel
        | P::Bloom
        | P::Bottom
        | P::CanSummon
        | P::Candles
        | P::ChemistryTableType
        | P::ChiselType
        | P::ClusterCount
        | P::Color
        | P::ColorBit
        | P::ConditionalBit
        | P::CoralColor
        | P::CoralDirection
        | P::CoralFanDirection
        | P::CoralHangTypeBit
        | P::CoveredBit
        | P::CreakingHeartState
        | P::Damage
        | P::DeadBit
        | P::Deprecated
        | P::Direction
        | P::DirtType
        | P::DoorHingeBit
        | P::DoublePlantType
        | P::DragDown
        | P::ExplodeBit
        | P::Extinguished
        | P::FacingDirection
        | P::FlowerType
        | P::GroundSignDirection
        | P::Hanging
        | P::HeadPieceBit
        | P::Height
        | P::HugeMushroomBits
        | P::InfiniburnBit
        | P::ItemFrameMapBit
        | P::ItemFramePhotoBit
        | P::LeverDirection
        | P::LiquidDepth
        | P::Locked
        | P::MinecraftBlockFace
        | P::MinecraftCardinalDirection
        | P::MinecraftCorner
        | P::MinecraftFacingDirection
        | P::MinecraftMultiBlockPart
        | P::MinecraftSixteenWayRotation
        | P::MinecraftVerticalHalf
        | P::MonsterEggStoneType
        | P::MultiFaceDirectionBits
        | P::Natural
        | P::NewLeafType
        | P::NewLogType
        | P::NoDropBit
        | P::OldLeafType
        | P::OldLogType
        | P::Ominous
        | P::Orientation
        | P::PillarAxis
        | P::PortalAxis
        | P::PotentSulfurState
        | P::PoweredShelfType
        | P::PrismarineBlockType
        | P::RehydrationLevel
        | P::Rotation
        | P::SandStoneType
        | P::SandType
        | P::SaplingType
        | P::SeaGrassType
        | P::SpongeType
        | P::StoneBrickType
        | P::StoneSlabType
        | P::StoneSlabType2
        | P::StoneSlabType3
        | P::StoneSlabType4
        | P::StoneType
        | P::StrippedBit
        | P::StructureBlockType
        | P::StructureVoidType
        | P::TallGrassType
        | P::Tip
        | P::TopSlotBit
        | P::TorchFacingDirection
        | P::TrialSpawnerState
        | P::TurtleEggCount
        | P::UpperBlockBit
        | P::UpsideDownBit
        | P::VaultState
        | P::VineDirectionBits
        | P::WallBlockType
        | P::WeirdoDirection
        | P::WoodType => false,
    }
}

/// Whether the named state property is set by the world; unknown names are not.
pub fn world_controlled_name(name: &str) -> bool {
    BlockProperty::from_name(name).is_some_and(world_controlled)
}

/// The serialized names of every world-controlled property, for consumers outside this
/// workspace; a test checks it against [`world_controlled`].
pub const WORLD_CONTROLLED_NAMES: &[&str] = &[
    "age",
    "age_bit",
    "attached_bit",
    "bamboo_leaf_size",
    "bamboo_stalk_thickness",
    "big_dripleaf_tilt",
    "bite_counter",
    "books_stored",
    "brewing_stand_slot_a_bit",
    "brewing_stand_slot_b_bit",
    "brewing_stand_slot_c_bit",
    "brushed_progress",
    "button_pressed_bit",
    "cauldron_liquid",
    "composter_fill_level",
    "cracked_state",
    "crafting",
    "disarmed_bit",
    "dripstone_thickness",
    "end_portal_eye_bit",
    "fill_level",
    "growing_plant_age",
    "growth",
    "honey_level",
    "in_wall_bit",
    "kelp_age",
    "lit",
    "minecraft:connection_down",
    "minecraft:connection_east",
    "minecraft:connection_north",
    "minecraft:connection_south",
    "minecraft:connection_up",
    "minecraft:connection_west",
    "moisturized_amount",
    "occupied_bit",
    "open_bit",
    "output_lit_bit",
    "output_subtract_bit",
    "pale_moss_carpet_side_east",
    "pale_moss_carpet_side_north",
    "pale_moss_carpet_side_south",
    "pale_moss_carpet_side_west",
    "persistent_bit",
    "powered_bit",
    "propagule_stage",
    "rail_data_bit",
    "rail_direction",
    "redstone_east",
    "redstone_north",
    "redstone_signal",
    "redstone_south",
    "redstone_west",
    "repeater_delay",
    "respawn_anchor_charge",
    "sculk_sensor_phase",
    "stability",
    "stability_check",
    "suspended_bit",
    "toggle_bit",
    "triggered_bit",
    "twisting_vines_age",
    "update_bit",
    "wall_connection_type_east",
    "wall_connection_type_north",
    "wall_connection_type_south",
    "wall_connection_type_west",
    "wall_post_bit",
    "weeping_vines_age",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_controlled_names_list_exactly_the_world_controlled_properties() {
        let classified: Vec<&str> = BlockProperty::ALL
            .iter()
            .filter(|property| world_controlled(**property))
            .map(|property| property.name())
            .collect();
        assert_eq!(classified, WORLD_CONTROLLED_NAMES);
    }

    #[test]
    fn orientation_stays_placement_controlled_and_world_state_is_ignored() {
        for orientation in [
            "direction",
            "facing_direction",
            "minecraft:cardinal_direction",
            "minecraft:facing_direction",
            "minecraft:block_face",
            "minecraft:vertical_half",
            "upside_down_bit",
            "top_slot_bit",
            "weirdo_direction",
            "pillar_axis",
            "orientation",
            "lever_direction",
            "torch_facing_direction",
            "ground_sign_direction",
            "hanging",
            "upper_block_bit",
            "candles",
        ] {
            assert!(!world_controlled_name(orientation), "{orientation}");
        }
        for world in [
            "growth",
            "age",
            "open_bit",
            "powered_bit",
            "rail_direction",
            "lit",
        ] {
            assert!(world_controlled_name(world), "{world}");
        }
        assert!(!world_controlled_name("not_a_property"));
    }
}
