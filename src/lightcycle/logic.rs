//! Bevy-free simulation for the TRON-style lightcycle mode.
//!
//! This module deliberately contains no Bevy types so movement, collisions,
//! spawn search, and parent-portal rules can be unit-tested on a plain thread.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Grid-aligned heading on the X/Z ground plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
    PosX,
    NegX,
    PosZ,
    NegZ,
}

impl Heading {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Heading::PosX => (1, 0),
            Heading::NegX => (-1, 0),
            Heading::PosZ => (0, 1),
            Heading::NegZ => (0, -1),
        }
    }

    /// Picks a heading whose next cell is safe (`is_safe` returns true), so a
    /// freshly spawned cycle does not crash immediately against a dense block.
    pub fn initial_heading(
        cell: (i32, i32),
        mut is_safe: impl FnMut((i32, i32)) -> bool,
    ) -> Option<Self> {
        [Heading::PosX, Heading::PosZ, Heading::NegX, Heading::NegZ]
            .iter()
            .copied()
            .find(|heading| {
                let (dx, dz) = heading.delta();
                is_safe((cell.0 + dx, cell.1 + dz))
            })
    }

    pub fn turn(self, turn: Turn) -> Self {
        match (self, turn) {
            // Left turns.
            (Heading::PosX, Turn::Left) => Heading::NegZ,
            (Heading::PosZ, Turn::Left) => Heading::PosX,
            (Heading::NegX, Turn::Left) => Heading::PosZ,
            (Heading::NegZ, Turn::Left) => Heading::NegX,
            // Right turns.
            (Heading::PosX, Turn::Right) => Heading::PosZ,
            (Heading::PosZ, Turn::Right) => Heading::NegX,
            (Heading::NegX, Turn::Right) => Heading::NegZ,
            (Heading::NegZ, Turn::Right) => Heading::PosX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    Left,
    Right,
}

/// High-level state of one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPhase {
    /// No empty spawn cell was found; the run is waiting for a restart/map change.
    Ready,
    Running,
    Crashed,
    /// A folder or parent portal has been requested and we are waiting for the load result.
    EnteringDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashReason {
    File,
    Trail,
    Wall,
}

/// Result of one or more cell-boundary crossings during an advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Moved,
    Crashed(CrashReason),
    EnteringDir(usize),
    GoToParent,
}

/// What a destination cell contains for collision purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellContent {
    Empty,
    File(usize),
    Dir(usize),
    Trail,
    Wall,
    ParentPortal,
}

/// Which external navigation request a run is waiting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRequest {
    Directory(usize),
    Parent,
}

/// One of the four arena walls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wall {
    NegX,
    PosX,
    NegZ,
    PosZ,
}

/// Where to cut the parent gate into an arena wall.
///
/// The caller picks this so arena construction stays a pure function of its
/// inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GatePlacement {
    pub wall: Wall,
    /// Position along the wall, from 0.0 at its start to 1.0 at its end.
    pub fraction: f32,
    /// How many wall cells the gate should cover.
    pub width_cells: i32,
}

impl GatePlacement {
    /// Derives a placement from a directory path.
    ///
    /// Hashing the path rather than drawing at random means every folder gets
    /// its own door in its own wall, but that door stays put between visits, so
    /// backtracking through a tree is learnable instead of a fresh search each
    /// time. The hash only has to be stable within a single run of the program,
    /// which `DefaultHasher` guarantees.
    pub fn for_path(path: &Path, width_cells: i32) -> Self {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        let hash = hasher.finish();

        let wall = match hash % 4 {
            0 => Wall::NegZ,
            1 => Wall::PosZ,
            2 => Wall::NegX,
            _ => Wall::PosX,
        };

        Self {
            wall,
            fraction: ((hash >> 8) as u16) as f32 / u16::MAX as f32,
            width_cells,
        }
    }
}

/// Gate in an arena wall that leads back to the parent directory.
///
/// `from`/`to` are the inclusive range of wall cells the gate covers. Driving
/// into any of them goes up a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentPortal {
    pub wall: Wall,
    pub from: (i32, i32),
    pub to: (i32, i32),
}

impl ParentPortal {
    /// Cuts a gate of `placement.width_cells` cells into the named wall of the
    /// arena bounded by `min`/`max`, sliding it along the wall by
    /// `placement.fraction`. Narrow arenas clamp the gate to the wall's length.
    fn place(min: (i32, i32), max: (i32, i32), placement: GatePlacement) -> Self {
        let (along_min, along_max) = match placement.wall {
            Wall::NegZ | Wall::PosZ => (min.0, max.0),
            Wall::NegX | Wall::PosX => (min.1, max.1),
        };

        let span = along_max - along_min + 1;
        let width = placement.width_cells.clamp(1, span);
        let slack = (span - width) as f32;
        let start = along_min + (placement.fraction.clamp(0.0, 1.0) * slack).round() as i32;
        let end = start + width - 1;

        let (from, to) = match placement.wall {
            Wall::NegZ => ((start, min.1 - 1), (end, min.1 - 1)),
            Wall::PosZ => ((start, max.1 + 1), (end, max.1 + 1)),
            Wall::NegX => ((min.0 - 1, start), (min.0 - 1, end)),
            Wall::PosX => ((max.0 + 1, start), (max.0 + 1, end)),
        };

        Self {
            wall: placement.wall,
            from,
            to,
        }
    }

    pub fn contains(&self, cell: (i32, i32)) -> bool {
        let within = |value: i32, a: i32, b: i32| value >= a.min(b) && value <= a.max(b);
        within(cell.0, self.from.0, self.to.0) && within(cell.1, self.from.1, self.to.1)
    }

    /// Number of wall cells the gate covers.
    pub fn width_cells(&self) -> i32 {
        (self.to.0 - self.from.0).abs() + (self.to.1 - self.from.1).abs() + 1
    }

    /// Inclusive cell range the gate covers along its own wall, ascending.
    pub fn along_span(&self) -> (i32, i32) {
        let (from, to) = match self.wall {
            Wall::NegZ | Wall::PosZ => (self.from.0, self.to.0),
            Wall::NegX | Wall::PosX => (self.from.1, self.to.1),
        };
        (from.min(to), from.max(to))
    }
}

/// Rectangular playable arena on the grid.
///
/// Grows an inclusive cell range to `span` cells, keeping its contents centered.
/// Ranges already that long are left alone.
fn expand_axis(min: i32, max: i32, span: i32) -> (i32, i32) {
    let extra = span - (max - min + 1);
    if extra <= 0 {
        return (min, max);
    }
    let before = extra / 2;
    (min - before, max + (extra - before))
}

/// `min`/`max` are inclusive cell coordinates inside the arena. Everything one
/// step beyond those bounds is wall territory, including the parent gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arena {
    pub min: (i32, i32),
    pub max: (i32, i32),
    pub parent_portal: Option<ParentPortal>,
}

impl Arena {
    /// Builds an arena around file/folder grid positions.
    ///
    /// Non-empty layouts get a `padding`-thick ring around the occupied cells.
    /// The result is then squared off and grown to at least `min_span` cells on
    /// each side: a handful of entries lands in a wide, shallow bounding box,
    /// and a corridor two or three cells deep has no room to take a corner.
    pub fn from_nodes(
        nodes: impl IntoIterator<Item = (i32, i32)>,
        parent_gate: Option<GatePlacement>,
        padding: i32,
        min_span: i32,
    ) -> Self {
        let mut min: Option<(i32, i32)> = None;
        let mut max: Option<(i32, i32)> = None;

        for (x, z) in nodes {
            min = Some(match min {
                Some((min_x, min_z)) => (min_x.min(x), min_z.min(z)),
                None => (x, z),
            });
            max = Some(match max {
                Some((max_x, max_z)) => (max_x.max(x), max_z.max(z)),
                None => (x, z),
            });
        }

        let (min, max) = match (min, max) {
            (Some(min), Some(max)) => (
                (min.0 - padding, min.1 - padding),
                (max.0 + padding, max.1 + padding),
            ),
            // An empty directory has nothing to build around, so the minimum
            // span is the whole arena.
            _ => ((0, 0), (0, 0)),
        };

        let span = (max.0 - min.0 + 1).max(max.1 - min.1 + 1).max(min_span);
        let (min_x, max_x) = expand_axis(min.0, max.0, span);
        let (min_z, max_z) = expand_axis(min.1, max.1, span);
        let (min, max) = ((min_x, min_z), (max_x, max_z));

        Self {
            min,
            max,
            parent_portal: parent_gate.map(|placement| ParentPortal::place(min, max, placement)),
        }
    }

    pub fn contains(&self, cell: (i32, i32)) -> bool {
        cell.0 >= self.min.0 && cell.0 <= self.max.0 && cell.1 >= self.min.1 && cell.1 <= self.max.1
    }

    pub fn center(&self) -> (i32, i32) {
        ((self.min.0 + self.max.0) / 2, (self.min.1 + self.max.1) / 2)
    }

    /// Returns the nearest empty cell to the arena center using an expanding
    /// square/ring search. Only interior arena cells are considered.
    pub fn nearest_empty_cell(
        &self,
        mut blocked: impl FnMut((i32, i32)) -> bool,
        max_radius: i32,
    ) -> Option<(i32, i32)> {
        let (cx, cz) = self.center();

        for radius in 0..=max_radius {
            let mut best: Option<(i32, (i32, i32))> = None;

            for x in (cx - radius)..=(cx + radius) {
                for z in (cz - radius)..=(cz + radius) {
                    let on_ring = (x - cx).abs().max((z - cz).abs()) == radius;
                    if !on_ring || !self.contains((x, z)) || blocked((x, z)) {
                        continue;
                    }

                    let dx = x - cx;
                    let dz = z - cz;
                    let distance_sq = dx * dx + dz * dz;
                    if best.is_none_or(|(best_distance_sq, _)| distance_sq < best_distance_sq) {
                        best = Some((distance_sq, (x, z)));
                    }
                }
            }

            if let Some((_, cell)) = best {
                return Some(cell);
            }
        }

        None
    }
}

/// One run's mutable simulation state.
#[derive(Debug, Clone, PartialEq)]
pub struct LightcycleSim {
    pub phase: RunPhase,
    pub cell: (i32, i32),
    pub heading: Heading,
    pub queued_turn: Option<Turn>,
    /// Progress from the current cell center toward the next cell, in cells.
    pub cell_t: f32,
    /// Cells that have been left behind and are now lethal trail.
    pub trail: Vec<(i32, i32)>,
    pub crash_reason: Option<CrashReason>,
    pub pending_request: Option<EntryRequest>,
}

impl LightcycleSim {
    /// Creates a run already moving from `cell`.
    pub fn start(cell: (i32, i32), heading: Heading) -> Self {
        Self {
            phase: RunPhase::Running,
            cell,
            heading,
            queued_turn: None,
            cell_t: 0.0,
            trail: Vec::new(),
            crash_reason: None,
            pending_request: None,
        }
    }

    /// Creates a run that cannot spawn yet (shown as `Ready`).
    pub fn ready(cell: (i32, i32), heading: Heading) -> Self {
        Self {
            phase: RunPhase::Ready,
            ..Self::start(cell, heading)
        }
    }

    /// Replaces the previous queued turn. A second turn before a boundary wins.
    pub fn queue_turn(&mut self, turn: Turn) {
        self.queued_turn = Some(turn);
    }

    /// Applies one frame's left/right input. Pressing both sides in the same
    /// frame cancels the queued turn instead of turning twice.
    pub fn queue_turn_input(&mut self, left: bool, right: bool) {
        match (left, right) {
            (true, true) => self.queued_turn = None,
            (true, false) => self.queue_turn(Turn::Left),
            (false, true) => self.queue_turn(Turn::Right),
            (false, false) => {}
        }
    }

    /// Stops movement while a breadcrumb/`u`/reload directory jump is loading.
    pub fn pause_for_directory_change(&mut self) {
        self.phase = RunPhase::EnteringDir;
        self.crash_reason = None;
        self.pending_request = None;
        self.queued_turn = None;
    }

    pub fn next_cell(&self) -> (i32, i32) {
        let (dx, dz) = self.heading.delta();
        (self.cell.0 + dx, self.cell.1 + dz)
    }

    /// Advances by `distance_cells` (typically `dt * cells_per_second`),
    /// crossing every boundary it reaches.
    ///
    /// `classify` is called for each destination cell with the immutable sim
    /// state so trail lookups can exclude the current head. Classification is
    /// done before entering the cell, so no crossed cell can be skipped.
    pub fn advance<F>(&mut self, distance_cells: f32, mut classify: F) -> StepOutcome
    where
        F: FnMut((i32, i32), &LightcycleSim) -> CellContent,
    {
        let mut remaining = distance_cells;
        let mut outcome = StepOutcome::Moved;

        while remaining > 0.0 {
            let needed = 1.0 - self.cell_t;
            if remaining < needed {
                self.cell_t += remaining;
                return outcome;
            }

            remaining -= needed;
            self.cell_t = 1.0;

            let next = self.next_cell();
            let content = classify(next, self);
            match content {
                CellContent::Empty => {
                    let previous = self.cell;
                    self.trail.push(previous);
                    self.cell = next;
                    self.cell_t = 0.0;

                    if let Some(turn) = self.queued_turn.take() {
                        self.heading = self.heading.turn(turn);
                    }

                    self.phase = RunPhase::Running;
                    self.crash_reason = None;
                    self.pending_request = None;
                    outcome = StepOutcome::Moved;
                }
                CellContent::Dir(index) => {
                    self.phase = RunPhase::EnteringDir;
                    self.pending_request = Some(EntryRequest::Directory(index));
                    return StepOutcome::EnteringDir(index);
                }
                CellContent::File(_) => {
                    return self.crash(CrashReason::File);
                }
                CellContent::Trail => {
                    return self.crash(CrashReason::Trail);
                }
                CellContent::Wall => {
                    return self.crash(CrashReason::Wall);
                }
                CellContent::ParentPortal => {
                    self.phase = RunPhase::EnteringDir;
                    self.pending_request = Some(EntryRequest::Parent);
                    return StepOutcome::GoToParent;
                }
            }
        }

        outcome
    }

    fn crash(&mut self, reason: CrashReason) -> StepOutcome {
        self.phase = RunPhase::Crashed;
        self.crash_reason = Some(reason);
        self.pending_request = None;
        StepOutcome::Crashed(reason)
    }
}

/// Convenience classifier shared by the Bevy plugin and unit tests.
pub fn classify_next_content(
    cell: (i32, i32),
    arena: &Arena,
    sim: &LightcycleSim,
    cells: &HashMap<(i32, i32), usize>,
    is_dir: impl Fn(usize) -> bool,
) -> CellContent {
    if arena
        .parent_portal
        .is_some_and(|portal| portal.contains(cell))
    {
        return CellContent::ParentPortal;
    }
    if !arena.contains(cell) {
        return CellContent::Wall;
    }
    if cell != sim.cell && sim.trail.contains(&cell) {
        return CellContent::Trail;
    }
    if let Some(&index) = cells.get(&cell) {
        return if is_dir(index) {
            CellContent::Dir(index)
        } else {
            CellContent::File(index)
        };
    }
    CellContent::Empty
}

#[cfg(test)]
mod tests {
    use super::{
        Arena, CellContent, CrashReason, EntryRequest, GatePlacement, Heading, LightcycleSim,
        RunPhase, StepOutcome, Turn, Wall, classify_next_content,
    };
    use std::collections::HashMap;
    use std::path::PathBuf;

    const PADDING: i32 = 1;
    /// Layout tests drive across known cells, so they opt out of the minimum
    /// span and keep arenas hugging their content.
    const MIN_SPAN: i32 = 0;
    const MAX_RADIUS: i32 = 1_000;
    const GATE_WIDTH: i32 = 3;

    /// A gate centered on the `-Z` wall, matching the pre-randomization layout
    /// so placement-independent tests stay readable.
    fn centered_gate() -> GatePlacement {
        GatePlacement {
            wall: Wall::NegZ,
            fraction: 0.5,
            width_cells: GATE_WIDTH,
        }
    }

    /// A layout with `(x, z)` cells. Files and dirs share one index space.
    struct TestLayout {
        arena: Arena,
        cells: HashMap<(i32, i32), usize>,
        is_dir: Vec<bool>,
    }

    impl TestLayout {
        fn new(
            positions: &[(i32, i32)],
            dirs: &[usize],
            parent_gate: Option<GatePlacement>,
        ) -> Self {
            let cells = positions
                .iter()
                .enumerate()
                .map(|(index, position)| (*position, index))
                .collect();
            let is_dir = (0..positions.len())
                .map(|index| dirs.contains(&index))
                .collect();
            Self {
                arena: Arena::from_nodes(positions.iter().copied(), parent_gate, PADDING, MIN_SPAN),
                cells,
                is_dir,
            }
        }

        fn empty(half: i32) -> Self {
            Self {
                arena: Arena {
                    min: (-half, -half),
                    max: (half, half),
                    parent_portal: None,
                },
                cells: HashMap::new(),
                is_dir: Vec::new(),
            }
        }

        fn classify(&self) -> impl FnMut((i32, i32), &LightcycleSim) -> CellContent + '_ {
            move |cell, sim| {
                classify_next_content(cell, &self.arena, sim, &self.cells, |index| {
                    self.is_dir[index]
                })
            }
        }
    }

    #[test]
    fn straight_run_advances_four_cells_along_pos_x() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start(layout.arena.center(), Heading::PosX);
        let outcome = sim.advance(4.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Moved);
        assert_eq!(
            sim.cell,
            (layout.arena.center().0 + 4, layout.arena.center().1)
        );
        assert_eq!(sim.trail.len(), 4);
        assert_eq!(sim.phase, RunPhase::Running);
    }

    #[test]
    fn left_turn_applies_at_boundary_pos_x_to_neg_z() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        sim.queue_turn(Turn::Left);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 0));
        assert_eq!(sim.heading, Heading::NegZ);
        assert!(sim.queued_turn.is_none());
    }

    #[test]
    fn right_turn_applies_at_boundary_pos_x_to_pos_z() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 0));
        assert_eq!(sim.heading, Heading::PosZ);
        assert!(sim.queued_turn.is_none());
    }

    #[test]
    fn second_turn_before_boundary_replaces_first() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        sim.queue_turn(Turn::Right);
        sim.queue_turn(Turn::Left);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.heading, Heading::NegZ);
    }

    #[test]
    fn left_and_right_in_same_frame_cancel() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        sim.queue_turn_input(true, true);
        assert!(sim.queued_turn.is_none());
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.heading, Heading::PosX);
    }

    #[test]
    fn initial_heading_avoids_blocked_neighbor() {
        let blocked = |cell: (i32, i32)| cell == (1, 0);
        let heading = Heading::initial_heading((0, 0), |cell| !blocked(cell)).unwrap();
        assert_eq!(heading, Heading::PosZ);
    }

    #[test]
    fn square_loop_into_own_trail_crashes_on_fourth_side() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);

        // Right turns draw a square: +X, +Z, -X, then back into the first cell.
        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 0));
        assert_eq!(sim.heading, Heading::PosZ);

        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 1));
        assert_eq!(sim.heading, Heading::NegX);

        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (0, 1));
        assert_eq!(sim.heading, Heading::NegZ);

        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::Trail));
        assert_eq!(sim.phase, RunPhase::Crashed);
        assert_eq!(sim.cell, (0, 1));
        assert!(sim.trail.contains(&(0, 0)));
    }

    #[test]
    fn next_cell_is_file_crashes() {
        let layout = TestLayout::new(&[(1, 0)], &[], None);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::File));
        assert_eq!(sim.crash_reason, Some(CrashReason::File));
        assert_eq!(sim.phase, RunPhase::Crashed);
        assert_eq!(sim.cell, (0, 0));
        assert!(sim.trail.is_empty());
    }

    #[test]
    fn next_cell_is_dir_requests_directory_not_crash() {
        let layout = TestLayout::new(&[(1, 0)], &[0], None);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::EnteringDir(0));
        assert_eq!(sim.phase, RunPhase::EnteringDir);
        assert_eq!(sim.pending_request, Some(EntryRequest::Directory(0)));
    }

    #[test]
    fn next_cell_is_trail_crashes() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        assert!(sim.trail.is_empty());

        // Right-hand square: the first turn is queued before leaving (0, 0).
        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 0));
        assert_eq!(sim.heading, Heading::PosZ);
        assert!(sim.trail.contains(&(0, 0)));

        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 1));

        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (0, 1));
        assert_eq!(sim.heading, Heading::NegZ);

        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::Trail));
    }

    #[test]
    fn current_head_on_trail_to_be_cell_is_allowed() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        sim.trail.push((1, 1)); // would-be trail cell that is not the head
        sim.cell = (1, 1); // current head is that cell
        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Moved);
        assert_eq!(sim.cell, (2, 1));
    }

    #[test]
    fn cell_just_left_is_trail_and_reentry_crashes() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);

        // Loop around the 2x2 square so the next step would re-enter (0, 0).
        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 0));
        assert!(sim.trail.contains(&(0, 0)));

        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (1, 1));

        sim.queue_turn(Turn::Right);
        sim.advance(1.0, layout.classify());
        assert_eq!(sim.cell, (0, 1));
        assert_eq!(sim.heading, Heading::NegZ);

        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::Trail));
    }

    #[test]
    fn arena_wall_crashes() {
        // One-cell arena at (0, 0); +X is immediately outside.
        let arena = Arena::from_nodes([(0, 0)].iter().copied(), None, 0, 0);
        let layout = TestLayout {
            arena,
            cells: HashMap::new(),
            is_dir: Vec::new(),
        };
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::Wall));
        assert_eq!(sim.crash_reason, Some(CrashReason::Wall));
    }

    #[test]
    fn parent_portal_cell_goes_to_parent() {
        // Arena covers z 0..=2; the portal sits just beyond the -Z wall.
        let arena = Arena::from_nodes([(1, 1)].iter().copied(), Some(centered_gate()), 1, MIN_SPAN);
        let portal = arena.parent_portal.unwrap();
        assert_eq!(portal.from.1, arena.min.1 - 1);

        let mut sim = LightcycleSim::start((portal.from.0, arena.min.1), Heading::NegZ);
        let layout = TestLayout {
            arena,
            cells: HashMap::new(),
            is_dir: Vec::new(),
        };
        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::GoToParent);
        assert_eq!(sim.phase, RunPhase::EnteringDir);
        assert_eq!(sim.pending_request, Some(EntryRequest::Parent));
    }

    #[test]
    fn parent_gate_covers_the_configured_width() {
        let arena = Arena::from_nodes([(1, 1)].iter().copied(), Some(centered_gate()), 1, MIN_SPAN);
        let portal = arena.parent_portal.unwrap();
        assert_eq!(portal.width_cells(), GATE_WIDTH);

        let wall_z = arena.min.1 - 1;
        let covered = (arena.min.0..=arena.max.0)
            .filter(|x| portal.contains((*x, wall_z)))
            .count();
        assert_eq!(covered, GATE_WIDTH as usize);

        // Only the gate's own wall row accepts the cycle.
        assert!(!portal.contains((portal.from.0, wall_z - 1)));
    }

    #[test]
    fn parent_gate_slides_along_the_wall_without_leaving_it() {
        // A wall nine cells long, so a three-cell gate has room to slide.
        let nodes = [(-3, 0), (3, 0)];
        for fraction in [0.0, 0.5, 1.0] {
            let arena = Arena::from_nodes(
                nodes.iter().copied(),
                Some(GatePlacement {
                    fraction,
                    ..centered_gate()
                }),
                PADDING,
                MIN_SPAN,
            );
            let portal = arena.parent_portal.unwrap();

            assert_eq!(portal.width_cells(), GATE_WIDTH);
            assert!(portal.from.0 >= arena.min.0, "gate ran off the -X end");
            assert!(portal.to.0 <= arena.max.0, "gate ran off the +X end");
        }
    }

    #[test]
    fn parent_gate_clamps_to_a_wall_shorter_than_the_gate() {
        let arena = Arena::from_nodes([(0, 0)].iter().copied(), Some(centered_gate()), 0, 0);
        let portal = arena.parent_portal.unwrap();

        assert_eq!(portal.width_cells(), 1);
        assert!(portal.contains((0, -1)));
    }

    #[test]
    fn a_gate_on_any_wall_is_reachable_from_inside_the_arena() {
        // Padding of one around a single node leaves a 3x3 arena, so a
        // three-cell gate spans whichever wall it lands on.
        for (wall, start, heading) in [
            (Wall::NegZ, (0, -1), Heading::NegZ),
            (Wall::PosZ, (0, 1), Heading::PosZ),
            (Wall::NegX, (-1, 0), Heading::NegX),
            (Wall::PosX, (1, 0), Heading::PosX),
        ] {
            let layout = TestLayout::new(
                &[(0, 0)],
                &[],
                Some(GatePlacement {
                    wall,
                    ..centered_gate()
                }),
            );
            let mut sim = LightcycleSim::start(start, heading);

            let outcome = sim.advance(1.0, layout.classify());
            assert_eq!(
                outcome,
                StepOutcome::GoToParent,
                "gate on {wall:?} was not reachable"
            );
        }
    }

    #[test]
    fn gate_placement_is_stable_per_path_but_differs_between_paths() {
        let path = PathBuf::from("/home/user/projects");
        assert_eq!(
            GatePlacement::for_path(&path, GATE_WIDTH),
            GatePlacement::for_path(&path, GATE_WIDTH)
        );

        let walls: Vec<_> = (0..16)
            .map(|index| {
                GatePlacement::for_path(&PathBuf::from(format!("/dir{index}")), GATE_WIDTH).wall
            })
            .collect();
        assert!(
            walls.iter().any(|wall| *wall != walls[0]),
            "every sample path landed on the same wall"
        );
    }

    #[test]
    fn gate_placement_survives_a_degenerate_fraction() {
        for fraction in [-1.0, 2.0, f32::NAN] {
            let arena = Arena::from_nodes(
                [(1, 1)].iter().copied(),
                Some(GatePlacement {
                    fraction,
                    ..centered_gate()
                }),
                PADDING,
                MIN_SPAN,
            );
            let portal = arena.parent_portal.unwrap();
            assert!(portal.from.0 >= arena.min.0 && portal.to.0 <= arena.max.0);
        }
    }

    #[test]
    fn root_has_no_active_portal_and_wall_crashes() {
        let arena = Arena::from_nodes([(0, 0)].iter().copied(), None, 1, MIN_SPAN);
        assert!(arena.parent_portal.is_none());

        // Drive from the -Z edge into where the portal would be at a non-root dir.
        let mut sim = LightcycleSim::start((0, arena.min.1), Heading::NegZ);
        let layout = TestLayout {
            arena,
            cells: HashMap::new(),
            is_dir: Vec::new(),
        };
        let outcome = sim.advance(1.0, layout.classify());
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::Wall));
    }

    #[test]
    fn spawn_search_prefers_nearest_empty_when_center_is_occupied() {
        // A fully occupied 3x3 block, with the default padding ring around it.
        let positions: Vec<_> = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| (x, z)))
            .collect();
        assert!(positions.contains(&(0, 0)));

        let cells: HashMap<_, _> = positions
            .iter()
            .enumerate()
            .map(|(index, p)| (*p, index))
            .collect();
        let arena = Arena::from_nodes(positions.iter().copied(), None, PADDING, MIN_SPAN);

        let spawn = arena
            .nearest_empty_cell(|cell| cells.contains_key(&cell), MAX_RADIUS)
            .expect("padding ring has empty spawn cells");
        assert_ne!(spawn, (0, 0));
        assert!(arena.contains(spawn));
        assert!(!cells.contains_key(&spawn));

        // It should prefer an edge cell over a corner on the same ring.
        let (cx, cz) = arena.center();
        let dx = spawn.0 - cx;
        let dz = spawn.1 - cz;
        assert_eq!(dx * dx + dz * dz, 4);
    }

    #[test]
    fn empty_directory_spawns_centered_in_a_minimum_sized_arena() {
        let arena = Arena::from_nodes(std::iter::empty::<(i32, i32)>(), None, PADDING, 9);
        assert_eq!(arena_span(&arena), (9, 9));

        let spawn = arena
            .nearest_empty_cell(|_| false, MAX_RADIUS)
            .expect("empty arena has a spawn");
        assert_eq!(spawn, (0, 0));
        assert!(arena.contains(spawn));
    }

    fn arena_span(arena: &Arena) -> (i32, i32) {
        (arena.max.0 - arena.min.0 + 1, arena.max.1 - arena.min.1 + 1)
    }

    /// Two entries sit side by side, so their bounding box is wide and shallow.
    /// Squaring it is what gives the run room to take a corner.
    #[test]
    fn a_shallow_layout_is_squared_off() {
        let arena = Arena::from_nodes([(-3, -3), (0, -3)].iter().copied(), None, PADDING, 0);

        let (width, depth) = arena_span(&arena);
        assert_eq!(width, depth, "arena was not square: {arena:?}");
        assert!(depth >= 3, "a corridor this shallow cannot be turned in");
    }

    #[test]
    fn a_tiny_layout_grows_to_the_minimum_span() {
        for nodes in [vec![(0, 0)], vec![(-3, -3), (0, -3)], vec![]] {
            let arena = Arena::from_nodes(nodes.iter().copied(), None, PADDING, 9);
            assert_eq!(arena_span(&arena), (9, 9), "nodes {nodes:?}");
        }
    }

    /// The minimum is a floor, not a resize: a layout already wider than the
    /// minimum keeps its own size.
    #[test]
    fn a_large_layout_keeps_its_own_span() {
        let nodes: Vec<_> = (-9..=9).map(|x| (x, 0)).collect();
        let arena = Arena::from_nodes(nodes.iter().copied(), None, PADDING, 9);

        let (width, depth) = arena_span(&arena);
        assert_eq!(width, 21, "padded content span should be preserved");
        assert_eq!(width, depth);
    }

    #[test]
    fn squaring_keeps_the_layout_centered() {
        let arena = Arena::from_nodes([(-3, -3), (0, -3)].iter().copied(), None, PADDING, 9);

        // Content spans x -3..0 and z -3..-3, so its center is (-1.5, -3).
        let center_x = (arena.min.0 + arena.max.0) as f32 / 2.0;
        let center_z = (arena.min.1 + arena.max.1) as f32 / 2.0;
        assert!((center_x - -1.5).abs() <= 0.5, "drifted in x: {arena:?}");
        assert!((center_z - -3.0).abs() <= 0.5, "drifted in z: {arena:?}");
    }

    #[test]
    fn after_directory_transition_trail_is_empty() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        sim.advance(3.0, layout.classify());
        assert_eq!(sim.trail.len(), 3);

        // A new run in a new directory starts fresh.
        let new_layout = TestLayout::new(&[(0, 0), (1, 0), (2, 0)], &[0, 1, 2], None);
        let spawn = new_layout
            .arena
            .nearest_empty_cell(|cell| new_layout.cells.contains_key(&cell), MAX_RADIUS)
            .unwrap();
        let mut restarted = LightcycleSim::start(spawn, Heading::PosX);
        restarted.advance(0.0, new_layout.classify());
        assert!(restarted.trail.is_empty());
    }

    #[test]
    fn multiple_catch_up_steps_classify_every_crossed_cell() {
        let layout = TestLayout::empty(10);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);

        // A large advance still crosses cells one at a time.
        let outcome = sim.advance(3.4, layout.classify());
        assert_eq!(outcome, StepOutcome::Moved);
        assert_eq!(sim.cell, (3, 0));
        assert_eq!(sim.trail.len(), 3);
        assert!((sim.cell_t - 0.4).abs() < 1e-6);

        // And a collision on the fourth crossed cell is still detected.
        let blocked_layout = TestLayout {
            arena: Arena {
                min: (-10, -10),
                max: (10, 10),
                parent_portal: None,
            },
            cells: HashMap::from([((4, 0), 0)]),
            is_dir: vec![false],
        };
        let mut sim2 = LightcycleSim::start((0, 0), Heading::PosX);
        let outcome = sim2.advance(4.0, blocked_layout.classify());
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::File));
        assert_eq!(sim2.cell, (3, 0));
        assert_eq!(sim2.trail.len(), 3);
    }
}
