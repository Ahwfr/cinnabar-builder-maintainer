//! Experimental "upper-half air place": a stair, slab or trapdoor planned in its upper half is
//! offered with a mirrored air click (the bottom face of the empty ghost cell), so it needs no
//! support block. Whether a server honours that click is not known in advance, so the first
//! result decides: the block that appears is compared with the plan, and a wrong state turns the
//! mode off for the session instead of repeating the mistake.

use schematica_core::{BlockState, CellStatus, StateValue};

/// Frames a placed block is waited for before the attempt is forgotten.
const WATCH_FRAMES: u16 = 240;

/// What the watched cell turned out to hold.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The server placed the planned state, upper half included.
    Confirmed,
    /// Something appeared there in a different state; the mode is off from now on.
    Wrong,
}

#[derive(Default)]
pub(crate) struct UpperAir {
    /// A wrong result was seen; the offers stop until the setting is switched off and on.
    pub(crate) failed: bool,
    /// Blocks the server placed in the planned upper half this session.
    pub(crate) confirmed: u32,
    watch: Option<([i32; 3], u16)>,
}

/// Whether `block` is a stair, slab or trapdoor planned in its upper half.
pub(crate) fn eligible(block: &BlockState) -> bool {
    let name = block.name.as_str();
    let kind = name.ends_with("_stairs")
        || name.ends_with("_trapdoor")
        || (name.ends_with("_slab") && !name.contains("double"));
    kind && upper(block) == Some(true)
}

/// The planned half, from either Bedrock spelling of it.
fn upper(block: &BlockState) -> Option<bool> {
    if let Some(value) = block.states.get("upside_down_bit") {
        return match value {
            StateValue::Bool(value) => Some(*value),
            StateValue::Byte(value) => Some(*value == 1),
            StateValue::Int(value) => Some(*value == 1),
            StateValue::Str(_) => None,
        };
    }
    match block.states.get("minecraft:vertical_half")? {
        StateValue::Str(half) => match half.as_str() {
            "top" => Some(true),
            "bottom" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

impl UpperAir {
    /// Switching the setting on gives the server another chance.
    pub(crate) fn rearm(&mut self) {
        self.failed = false;
        self.watch = None;
    }

    /// Whether an eligible block should be offered with the air click now.
    pub(crate) fn active(&self, enabled: bool, block: &BlockState) -> bool {
        enabled && !self.failed && eligible(block)
    }

    /// Remembers the cell an air click was offered for, renewing the wait.
    pub(crate) fn offered(&mut self, cell: [i32; 3]) {
        self.watch = Some((cell, WATCH_FRAMES));
    }

    /// The cell to read this frame.
    pub(crate) fn watched(&self) -> Option<[i32; 3]> {
        self.watch.map(|(cell, _)| cell)
    }

    /// Judges the watched cell from its current status. A missing block keeps waiting until
    /// the wait runs out; an unloaded cell is skipped.
    pub(crate) fn observe(&mut self, status: Option<CellStatus>) -> Option<Verdict> {
        let (cell, frames) = self.watch?;
        let verdict = match status {
            Some(CellStatus::Correct) => Some(Verdict::Confirmed),
            Some(CellStatus::WrongState | CellStatus::Wrong) => Some(Verdict::Wrong),
            _ => None,
        };
        match verdict {
            Some(Verdict::Confirmed) => {
                self.confirmed += 1;
                self.watch = None;
            }
            Some(Verdict::Wrong) => {
                self.failed = true;
                self.watch = None;
            }
            None => {
                self.watch = frames.checked_sub(1).filter(|left| *left > 0).map(|left| (cell, left));
            }
        }
        verdict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(name: &str, states: &[(&str, StateValue)]) -> BlockState {
        let mut block = BlockState::new(name);
        for (key, value) in states {
            block.states.insert((*key).into(), value.clone());
        }
        block
    }

    #[test]
    fn only_upper_half_stairs_slabs_and_trapdoors_are_eligible() {
        let upper = [("upside_down_bit", StateValue::Bool(true))];
        let lower = [("upside_down_bit", StateValue::Byte(0))];
        assert!(eligible(&block("minecraft:oak_stairs", &upper)));
        assert!(eligible(&block("minecraft:stone_block_slab", &upper)));
        assert!(eligible(&block("minecraft:oak_trapdoor", &[("upside_down_bit", StateValue::Int(1))])));
        assert!(eligible(&block(
            "minecraft:cut_copper_slab",
            &[("minecraft:vertical_half", StateValue::Str("top".into()))]
        )));
        assert!(!eligible(&block("minecraft:oak_stairs", &lower)));
        assert!(!eligible(&block("minecraft:double_stone_block_slab", &upper)));
        assert!(!eligible(&block("minecraft:dropper", &upper)));
        assert!(!eligible(&block("minecraft:oak_stairs", &[])));
    }

    #[test]
    fn the_first_wrong_block_turns_the_mode_off_until_it_is_switched_on_again() {
        let mut mode = UpperAir::default();
        let cell = [1, 2, 3];
        assert_eq!(mode.observe(Some(CellStatus::Wrong)), None, "nothing is watched yet");
        mode.offered(cell);
        assert_eq!(mode.watched(), Some(cell));
        assert_eq!(mode.observe(Some(CellStatus::Missing)), None);
        assert_eq!(mode.observe(Some(CellStatus::Unloaded)), None);
        assert_eq!(mode.observe(Some(CellStatus::WrongState)), Some(Verdict::Wrong));
        assert!(mode.failed);
        assert_eq!(mode.watched(), None);
        let upper = block("minecraft:oak_stairs", &[("upside_down_bit", StateValue::Bool(true))]);
        assert!(!mode.active(true, &upper));
        mode.rearm();
        assert!(mode.active(true, &upper));
        assert!(!mode.active(false, &upper));
    }

    #[test]
    fn a_correct_block_is_counted_once_and_the_mode_stays_on() {
        let mut mode = UpperAir::default();
        mode.offered([0, 0, 0]);
        assert_eq!(mode.observe(Some(CellStatus::Correct)), Some(Verdict::Confirmed));
        assert_eq!(mode.observe(Some(CellStatus::Correct)), None);
        assert_eq!((mode.confirmed, mode.failed), (1, false));
    }

    #[test]
    fn a_block_that_never_appears_is_forgotten_without_a_verdict() {
        let mut mode = UpperAir::default();
        mode.offered([0, 0, 0]);
        for _ in 0..WATCH_FRAMES {
            assert_eq!(mode.observe(Some(CellStatus::Missing)), None);
        }
        assert_eq!(mode.watched(), None);
        assert!(!mode.failed);
    }
}
