//! After a paste, looks for the blocks the server did not keep and sends only those again.
//!
//! Blocks that hang from or lean on another block (hanging signs and lanterns, wall signs, sand,
//! torches, rails, plants) break when the paste places them before their support exists. Once
//! the whole paste has been sent the supports are in, so the missing ones can be placed again.
//! The check reads the world around the pasted area a few cells a frame, lists what is missing,
//! sends those cells bottom-up, waits, and checks again, up to `MAX_ROUNDS` times.

use super::Paste;
use crate::{Viewer, budget, gameplay, overlay, terrain};
use schematica_core::{CellStatus, Placement};
use std::collections::HashMap;

/// Edge of the box of world that one terrain read covers.
const EDGE: i32 = 8;
/// Scans, counting the first one. The last one only reports.
const MAX_ROUNDS: u8 = 4;
/// Frames to leave the server after a repair round (commands), and after a Structure Paste, which it
/// places in a background job, before looking.
const SETTLE_FRAMES: u32 = 180;
const SETTLE_FRAMES_STRUCTURE: u32 = 480;
/// Fuel a frame must still have before it checks cells or sends a command.
const SCAN_FUEL: u64 = 500_000;
const SEND_FUEL: u64 = 700_000;
/// Commands one frame may send. The host ends the mod for good when a frame asks for more than
/// four (`MAX_COMMANDS_PER_FRAME`), so stay under that and leave room for other senders.
const SENDS_PER_FRAME: usize = 3;
/// Failed reads of one box, a frame apart, after which it is counted as not checked. A box can
/// be unreadable when it is too far from the player or the frame's terrain reads are used up.
const BOX_RETRIES: u32 = 30;

enum Phase {
    Settle(u32),
    Scan,
    Send,
}

/// The box being checked: the world it covers and the cell to check next.
struct Scanning {
    region: terrain::Region,
    min: [i32; 3],
    dims: [i32; 3],
    cursor: usize,
}

pub(crate) struct Repair {
    scope: (u64, i32),
    size: [i32; 3],
    placements: Vec<Placement>,
    destroy: bool,
    round: u8,
    phase: Phase,
    copy: usize,
    next: [i32; 3],
    scanning: Option<Scanning>,
    /// Cells found missing this round: the copy and the schematic cell.
    missing: Vec<(usize, [i32; 3])>,
    send: usize,
    resent: usize,
    unloaded: usize,
    failures: u32,
    cache: HashMap<(u32, u32), (CellStatus, bool)>,
    palette: HashMap<u32, Option<String>>,
}

impl Repair {
    pub(super) fn new(run: &Paste, size: [i32; 3]) -> Self {
        let mut placements = run.finished.clone();
        placements.push(run.placement);
        let first = placements[0].world_bounds(size).0;
        Self {
            scope: run.scope,
            size,
            placements,
            destroy: run.destroy,
            round: 1,
            phase: Phase::Settle(SETTLE_FRAMES_STRUCTURE),
            copy: 0,
            next: first,
            scanning: None,
            missing: Vec::new(),
            send: 0,
            resent: 0,
            unloaded: 0,
            failures: 0,
            cache: HashMap::new(),
            palette: HashMap::new(),
        }
    }

    fn begin_scan(&mut self) {
        self.phase = Phase::Scan;
        self.copy = 0;
        self.next = self.placements[0].world_bounds(self.size).0;
        self.scanning = None;
        self.missing.clear();
        self.unloaded = 0;
        self.failures = 0;
        if self.cache.len() > 4096 {
            self.cache.clear();
        }
    }
}

/// The corner of the box after the one at `next` inside the world bounds `lo..hi`, taking y
/// slabs from the bottom up, or `None` after the last box.
fn next_box(next: [i32; 3], lo: [i32; 3], hi: [i32; 3]) -> Option<[i32; 3]> {
    let mut next = next;
    next[0] += EDGE;
    if next[0] >= hi[0] {
        next[0] = lo[0];
        next[2] += EDGE;
        if next[2] >= hi[2] {
            next[2] = lo[2];
            next[1] += EDGE;
            if next[1] >= hi[1] {
                return None;
            }
        }
    }
    Some(next)
}

/// How many entries from `at` a `cells`-long run along x from `cell` covers. The list is sorted,
/// so the covered entries follow `at` directly.
fn covered(missing: &[(usize, [i32; 3])], at: usize, copy: usize, cell: [i32; 3], cells: usize) -> usize {
    missing[at..]
        .iter()
        .take_while(|&&(other, [x, y, z])| {
            other == copy
                && y == cell[1]
                && z == cell[2]
                && x >= cell[0]
                && x < cell[0] + cells as i32
        })
        .count()
        .max(1)
}

impl Viewer {
    /// Drives the check after a paste. True while it is running, so the frame treats it like the
    /// paste itself.
    pub(crate) fn repair_step(&mut self) -> bool {
        let Some(mut run) = self.repair.take() else {
            return false;
        };
        if self.world != Some(run.scope) || self.schematic.size() != run.size {
            self.show_label("Paste check canceled because the world changed");
            return true;
        }
        if !gameplay::can_paste_blocks() {
            self.paste_result("Paste check stopped | Pasting is no longer allowed".into());
            return true;
        }
        match run.phase {
            Phase::Settle(frames) => {
                self.show_label(&format!(
                    "Paste check {}/{MAX_ROUNDS} | Waiting for the server",
                    run.round
                ));
                if frames <= 1 {
                    run.begin_scan();
                } else {
                    run.phase = Phase::Settle(frames - 1);
                }
            }
            Phase::Scan => {
                if let Some(done) = self.repair_scan(&mut run) {
                    self.paste_result(done);
                    return true;
                }
            }
            Phase::Send => {
                if let Some(done) = self.repair_send(&mut run) {
                    self.paste_result(done);
                    return true;
                }
            }
        }
        self.repair = Some(run);
        true
    }

    /// Checks cells of the current box, then moves on. `Some` ends the check with a report.
    fn repair_scan(&mut self, run: &mut Repair) -> Option<String> {
        let placement = run.placements[run.copy];
        let (lo, hi) = placement.world_bounds(run.size);
        if run.scanning.is_none() {
            let max: [i32; 3] = std::array::from_fn(|axis| (run.next[axis] + EDGE).min(hi[axis]));
            let query = terrain::RegionQuery {
                min: position(run.next),
                max: position(max),
                geometry: terrain::Geometry::empty(),
            };
            match terrain::read_region(query, None) {
                Ok(terrain::ReadResult::Changed(region)) => {
                    run.failures = 0;
                    run.scanning = Some(Scanning {
                        region,
                        min: run.next,
                        dims: std::array::from_fn(|axis| max[axis] - run.next[axis]),
                        cursor: 0,
                    });
                }
                _ => {
                    run.failures += 1;
                    if run.failures <= BOX_RETRIES {
                        return None;
                    }
                    run.failures = 0;
                    let cells: i32 = (0..3).map(|axis| max[axis] - run.next[axis]).product();
                    run.unloaded += cells as usize;
                    return self.repair_next_box(run, lo, hi);
                }
            }
        }
        let scanning = run.scanning.as_mut()?;
        let [dx, dy, dz] = scanning.dims.map(|axis| axis as usize);
        let cells = dx * dy * dz;
        while scanning.cursor < cells && u64::from(budget::remaining_fuel()) >= SCAN_FUEL {
            let cell = scanning.cursor;
            scanning.cursor += 1;
            // The terrain order: x, then z, then y.
            let offset = [(cell / (dz * dy)) as i32, (cell % dy) as i32, (cell / dy % dz) as i32];
            let world = std::array::from_fn(|axis| scanning.min[axis] + offset[axis]);
            let Some(local) = placement.world_to_local(run.size, world) else {
                continue;
            };
            let Some(index) = self.schematic.palette_index(local) else {
                continue;
            };
            if self.schematic.is_air_index(index)
                || self.schematic.palette()[index as usize].is_placement_artifact()
            {
                continue;
            }
            let Some(expected) = self.preview.expected(index) else {
                continue;
            };
            let Some(runtime_id) = scanning
                .region
                .states
                .get(cell)
                .and_then(|&state| scanning.region.palette.get(usize::from(state)))
                .map(|block| block.runtime_id)
            else {
                run.unloaded += 1;
                continue;
            };
            let (status, ignored) = *run
                .cache
                .entry((index, runtime_id))
                .or_insert_with(|| overlay::compare_cell(&scanning.region, cell, expected, false));
            if ignored {
                continue;
            }
            match status {
                CellStatus::Unloaded => run.unloaded += 1,
                CellStatus::Missing => run.missing.push((run.copy, local)),
                CellStatus::Wrong | CellStatus::WrongState if run.destroy => {
                    run.missing.push((run.copy, local));
                }
                _ => {}
            }
        }
        if scanning.cursor < cells {
            return None;
        }
        run.scanning = None;
        self.repair_next_box(run, lo, hi)
    }

    /// Moves to the next box, or the next copy. `Some` is the final report after the last box.
    fn repair_next_box(&mut self, run: &mut Repair, lo: [i32; 3], hi: [i32; 3]) -> Option<String> {
        match next_box(run.next, lo, hi) {
            Some(next) => run.next = next,
            None if run.copy + 1 < run.placements.len() => {
                run.copy += 1;
                run.next = run.placements[run.copy].world_bounds(run.size).0;
            }
            None => return self.repair_scanned(run),
        }
        let span = (hi[1] - lo[1]).max(1);
        self.show_label(&format!(
            "Paste check {}/{MAX_ROUNDS} | Looking for missing blocks {}%",
            run.round,
            (run.next[1] - lo[1]) * 100 / span
        ));
        None
    }

    /// The whole area has been read: report, or start sending what is missing.
    fn repair_scanned(&mut self, run: &mut Repair) -> Option<String> {
        let note = if run.unloaded > 0 {
            format!(" {} cells were not loaded and could not be checked.", run.unloaded)
        } else {
            String::new()
        };
        if run.missing.is_empty() {
            return Some(if run.resent == 0 {
                format!("Paste check | Every block is in place.{note}")
            } else {
                format!(
                    "Paste check | Every block is in place after re-sending {}.{note}",
                    run.resent
                )
            });
        }
        if run.round >= MAX_ROUNDS {
            return Some(format!(
                "Paste check | {} blocks are still missing after {} tries. Check the chat and the \
                 world, then paste again.{note}",
                run.missing.len(),
                MAX_ROUNDS - 1
            ));
        }
        // Bottom-up, so a block's support goes in before the block that leans on it.
        run.missing
            .sort_by_key(|&(copy, [x, y, z])| (copy, y, z, x));
        run.send = 0;
        run.phase = Phase::Send;
        None
    }

    /// Sends a few of the missing cells. `Some` ends the check with a report.
    fn repair_send(&mut self, run: &mut Repair) -> Option<String> {
        let mut sent = 0;
        while run.send < run.missing.len()
            && sent < SENDS_PER_FRAME
            && u64::from(budget::remaining_fuel()) >= SEND_FUEL
        {
            let (copy, local) = run.missing[run.send];
            let placement = run.placements[copy];
            let Some(index) = self.schematic.palette_index(local) else {
                run.send += 1;
                continue;
            };
            let formatted = run.palette.entry(index).or_insert_with(|| {
                super::format_state(
                    &placement.transform_state(&self.schematic.palette()[index as usize]),
                )
            });
            let Some(formatted) = formatted.clone() else {
                run.send += 1;
                continue;
            };
            let (command, cells) = super::command(
                &self.schematic,
                placement,
                local,
                index,
                &formatted,
                run.destroy,
            );
            let Some(command) = command else {
                run.send += 1;
                continue;
            };
            match gameplay::request_command(&command) {
                Ok(()) => {
                    let head = placement.local_to_world(run.size, local);
                    let tail = placement.local_to_world(
                        run.size,
                        [local[0] + cells as i32 - 1, local[1], local[2]],
                    );
                    self.expect_pasted(
                        std::array::from_fn(|axis| head[axis].min(tail[axis])),
                        std::array::from_fn(|axis| head[axis].max(tail[axis]) + 1),
                    );
                    let used = covered(&run.missing, run.send, copy, local, cells);
                    run.send += used;
                    run.resent += used;
                    sent += 1;
                }
                Err(error) if error == "command rate limit reached" => break,
                Err(error) => return Some(format!("Paste check stopped | {error}")),
            }
        }
        if run.send < run.missing.len() {
            self.show_label(&format!(
                "Paste check {}/{MAX_ROUNDS} | Re-sending missing blocks {} of {}",
                run.round,
                run.send,
                run.missing.len()
            ));
            return None;
        }
        run.round += 1;
        run.phase = Phase::Settle(SETTLE_FRAMES);
        None
    }
}

fn position([x, y, z]: [i32; 3]) -> terrain::Position {
    terrain::Position { x, y, z }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_cover_the_area_once_from_the_bottom_up() {
        let (lo, hi) = ([10, 0, -4], [31, 17, 13]);
        let mut seen = vec![0u8; ((hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2])) as usize];
        let mut last_y = lo[1];
        let mut next = Some(lo);
        while let Some(corner) = next {
            assert!(corner[1] >= last_y, "slabs go upward");
            last_y = corner[1];
            for x in corner[0]..(corner[0] + EDGE).min(hi[0]) {
                for y in corner[1]..(corner[1] + EDGE).min(hi[1]) {
                    for z in corner[2]..(corner[2] + EDGE).min(hi[2]) {
                        let at = ((x - lo[0]) * (hi[1] - lo[1]) + (y - lo[1])) * (hi[2] - lo[2])
                            + (z - lo[2]);
                        seen[at as usize] += 1;
                    }
                }
            }
            next = next_box(corner, lo, hi);
        }
        assert!(seen.iter().all(|&count| count == 1));
    }

    #[test]
    fn a_frame_never_sends_as_many_commands_as_the_host_allows() {
        // The host quarantines the mod when one frame asks for its fourth-plus command.
        assert!(SENDS_PER_FRAME < 4);
    }

    #[test]
    fn a_small_area_is_one_box() {
        assert_eq!(next_box([0, 0, 0], [0, 0, 0], [3, 3, 3]), None);
    }

    #[test]
    fn a_run_covers_the_missing_cells_it_fills() {
        let missing = [
            (0, [2, 5, 1]),
            (0, [3, 5, 1]),
            (0, [4, 5, 1]),
            (0, [9, 5, 1]),
            (0, [2, 6, 1]),
            (1, [2, 6, 1]),
        ];
        assert_eq!(covered(&missing, 0, 0, [2, 5, 1], 3), 3);
        // A run that stops before the last cell leaves it for the next command.
        assert_eq!(covered(&missing, 0, 0, [2, 5, 1], 2), 2);
        // A single cell always counts itself, even when the list does not hold it.
        assert_eq!(covered(&missing, 3, 0, [9, 5, 1], 1), 1);
        assert_eq!(covered(&missing, 4, 0, [2, 6, 1], 1), 1);
    }

    #[test]
    fn supports_are_sent_before_what_hangs_from_them() {
        let mut missing = vec![(0, [1, 7, 1]), (0, [3, 2, 0]), (0, [1, 2, 0]), (0, [1, 2, 1])];
        missing.sort_by_key(|&(copy, [x, y, z])| (copy, y, z, x));
        let heights: Vec<i32> = missing.iter().map(|&(_, [_, y, _])| y).collect();
        assert_eq!(heights, [2, 2, 2, 7]);
        assert_eq!(missing[0].1, [1, 2, 0]);
        assert_eq!(missing[1].1, [3, 2, 0]);
    }
}
