use super::*;
use std::collections::HashMap;

/// A region is one terrain query, so a larger one sweeps the guide in fewer queries. The
/// per-cell loop still yields to the frame's fuel budget and resumes where it stopped.
const SCAN_EDGE: i32 = 8;
const FAR_SCAN_PERIOD: u8 = 4;

pub(crate) fn on_distant_scan_frame(phase: &mut u8) -> bool {
    let distant = *phase >= FAR_SCAN_PERIOD - 2;
    *phase = (*phase + 1) % FAR_SCAN_PERIOD;
    distant
}

#[derive(Default)]
pub(super) struct DistantScan {
    region: usize,
    cell: usize,
}

impl WholeGhost {
    /// Reconciles the complete textured grid beyond the local verifier window without
    /// allocating status markers or changing placement and material observations.
    pub(crate) fn scan_distant(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        focus: [i32; 3],
    ) {
        if self.changes.len() >= 1024 {
            return;
        }
        let size = schematic.size();
        let (lo, hi) = preview.bounds(size);
        let near = preview.scan_window(size, Some(focus));
        let counts = tile_counts(lo, hi, SCAN_EDGE).map(|count| count as usize);
        let regions = counts.iter().product();
        let (start, end) = scan_region(lo, hi, counts, self.distant.region);
        if inside(start, near) && inside(end.map(|n| n - 1), near) {
            self.advance_distant(regions);
            return;
        }
        let query = terrain::RegionQuery {
            min: position(start),
            max: position(end),
            geometry: terrain::Geometry::empty(),
        };
        let region = match terrain::read_region(query, None) {
            Ok(terrain::ReadResult::Pending) => return,
            Ok(terrain::ReadResult::Changed(region)) => region,
            _ => {
                self.advance_distant(regions);
                return;
            }
        };
        let [sx, sy, sz] = std::array::from_fn(|axis| (end[axis] - start[axis]) as usize);
        let count = sx * sy * sz;
        let grid_y = (hi[1] - lo[1]) as usize;
        let grid_z = (hi[2] - lo[2]) as usize;
        let placement = preview.placement.unwrap();
        let mut compared = HashMap::new();
        while self.distant.cell < count {
            let cell = self.distant.cell;
            if budget::remaining_fuel() < 425_000 {
                return;
            }
            self.distant.cell += 1;
            let world = [
                start[0] + (cell / (sy * sz)) as i32,
                start[1] + (cell % sy) as i32,
                start[2] + (cell / sy % sz) as i32,
            ];
            if inside(world, near)
                || !region
                    .loaded
                    .get(cell / 64)
                    .is_some_and(|bits| bits & (1 << (cell % 64)) != 0)
            {
                continue;
            }
            let local = placement.world_to_local(size, world).unwrap();
            let id = schematic.palette_index(local).unwrap();
            if schematic.is_air_index(id)
                || schematic.palette()[id as usize].is_placement_artifact()
            {
                continue;
            }
            let Some(&actual) = region.states.get(cell) else {
                continue;
            };
            let status = *compared
                .entry((id, actual))
                .or_insert_with(|| compare(&region, cell, &preview.palette[id as usize], false));
            let pos = std::array::from_fn(|axis| (world[axis] - lo[axis]) as u32);
            let [x, y, z] = pos.map(|axis| axis as usize);
            self.apply_observation((x * grid_z + z) * grid_y + y, pos, id, status);
        }
        self.advance_distant(regions);
    }

    fn advance_distant(&mut self, regions: usize) {
        self.distant.region = (self.distant.region + 1) % regions;
        self.distant.cell = 0;
    }
}

fn inside(pos: [i32; 3], (lo, hi): ([i32; 3], [i32; 3])) -> bool {
    (0..3).all(|axis| (lo[axis]..hi[axis]).contains(&pos[axis]))
}

fn scan_region(
    lo: [i32; 3],
    hi: [i32; 3],
    [_, cy, cz]: [usize; 3],
    index: usize,
) -> ([i32; 3], [i32; 3]) {
    let offsets = [index / (cy * cz), index % cy, index / cy % cz];
    let start = std::array::from_fn(|i| lo[i] + offsets[i] as i32 * SCAN_EDGE);
    let end = std::array::from_fn(|i| hi[i].min(start[i] + SCAN_EDGE));
    (start, end)
}

#[test]
fn coverage_reaches_beyond_the_local_camera_window() {
    let size = [47, 45, 36];
    let mut preview = Preview::default();
    preview.reposition(Placement::new([0; 3]), LayerMode::All);
    let (lo, hi) = preview.bounds(size);
    let focus = preview.focus(size, [1.; 3]).unwrap();
    let near = preview.scan_window(size, Some(focus));
    let far = [46, 44, 35];
    assert!(!inside(far, near));
    let counts = tile_counts(lo, hi, SCAN_EDGE).map(|count| count as usize);
    let regions: Vec<_> = (0..counts.iter().product())
        .map(|i| scan_region(lo, hi, counts, i))
        .collect();
    assert_eq!(
        regions
            .iter()
            .filter(|&&bounds| inside(far, bounds))
            .count(),
        1
    );
    assert_eq!(
        regions
            .iter()
            .map(|(lo, hi)| (0..3).map(|i| hi[i] - lo[i]).product::<i32>())
            .sum::<i32>(),
        size.iter().product::<i32>()
    );
}

#[test]
fn distant_queries_get_two_adjacent_frames_to_deliver_a_sample() {
    let mut phase = 0;
    let samples = std::array::from_fn::<_, 12, _>(|_| on_distant_scan_frame(&mut phase));
    assert_eq!(
        samples,
        [
            false, false, true, true, false, false, true, true, false, false, true, true
        ]
    );
}
