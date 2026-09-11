//! Bevy-free simulation for the TRON-style lightcycle mode.
//!
//! This module deliberately contains no Bevy types so movement, collisions,
//! spawn search, and parent-portal rules can be unit-tested on a plain thread.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
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

    /// Headings in the order the spawn search prefers them when several offer
    /// the cycle equally long runway.
    const PREFERENCE: [Self; 4] = [Self::PosX, Self::PosZ, Self::NegX, Self::NegZ];

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

    /// The heading directly opposite this one, used when a disc turns back.
    pub fn opposite(self) -> Self {
        match self {
            Heading::PosX => Heading::NegX,
            Heading::NegX => Heading::PosX,
            Heading::PosZ => Heading::NegZ,
            Heading::NegZ => Heading::PosZ,
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
    /// A folder, parent portal, or markdown file has been requested.
    EnteringDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashReason {
    File,
    Trail,
    Wall,
    /// Rode into the Recognizer opponent's body.
    Opponent,
    /// Rode into a live disc.
    Disc,
    /// Lingered on too many hazard tiles in a row.
    Hazard,
}

/// Result of one or more cell-boundary crossings during an advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Moved,
    Crashed(CrashReason),
    EnteringDir(usize),
    EnteringDocument(usize),
    EnteringSource(usize),
    GoToParent,
    CloseDocument,
}

/// What a destination cell contains for collision purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellContent {
    Empty,
    File(usize),
    Dir(usize),
    Markdown(usize),
    /// A rideable source file: entering starts a disc-wars ring.
    Source(usize),
    Trail,
    Wall,
    ParentPortal,
    ClosePortal,
    /// The opponent's body.
    Opponent,
    /// A live opponent disc.
    OpponentDisc,
}

/// Which external navigation request a run is waiting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRequest {
    Directory(usize),
    Document(usize),
    Source(usize),
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
    /// its own door in its own wall, but that door stays put between visits and
    /// across Rust releases, so backtracking through a tree is learnable.
    pub fn for_path(path: &Path, width_cells: i32) -> Self {
        let hash = stable_path_seed(path);

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

/// Visual family for one path-seeded TRON district.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CityTheme {
    Cyan,
    Magenta,
    Violet,
    Amber,
}

impl CityTheme {
    pub(crate) fn from_seed(seed: u64) -> Self {
        match seed % 4 {
            0 => Self::Cyan,
            1 => Self::Magenta,
            2 => Self::Violet,
            _ => Self::Amber,
        }
    }
}

/// Silhouette used to render one lethal architecture cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CityStructureKind {
    Barrier,
    GlassFin,
    Pylon,
}

/// Bevy-free rendering metadata for a procedural structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CityStructure {
    pub cell: (i32, i32),
    pub kind: CityStructureKind,
    pub along_x: bool,
    pub height_tier: u8,
    pub accent: u8,
    pub pulse_phase: u8,
}

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

/// Directory cities, markdown pages, and disc-wars rings share bounds and
/// portals, but the kind decides which generator and palette may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArenaKind {
    #[default]
    Directory,
    Document,
    /// A circular source-file ring. Its close gate behaves like a document's.
    Disc,
}

/// `min`/`max` are inclusive cell coordinates inside the arena. Everything one
/// step beyond those bounds is wall territory, including the parent gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arena {
    pub min: (i32, i32),
    pub max: (i32, i32),
    pub parent_portal: Option<ParentPortal>,
    pub kind: ArenaKind,
    /// Connected arteries, plazas, and the perimeter boulevard.
    pub roads: BTreeSet<(i32, i32)>,
    /// Lethal footprints occupied by `structures`.
    pub street_walls: BTreeSet<(i32, i32)>,
    pub structures: Vec<CityStructure>,
    pub city_theme: CityTheme,
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
            kind: ArenaKind::Directory,
            roads: BTreeSet::new(),
            street_walls: BTreeSet::new(),
            structures: Vec::new(),
            city_theme: CityTheme::Cyan,
        }
    }

    pub fn contains(&self, cell: (i32, i32)) -> bool {
        cell.0 >= self.min.0 && cell.0 <= self.max.0 && cell.1 >= self.min.1 && cell.1 <= self.max.1
    }

    pub fn center(&self) -> (i32, i32) {
        ((self.min.0 + self.max.0) / 2, (self.min.1 + self.max.1) / 2)
    }

    /// Builds a deterministic connected road lattice first, then places capped
    /// runs of architecture in the remaining buildable cells.
    ///
    /// Towers live on a `tower_stride` lattice. Arteries use a seeded offset
    /// between lattice lines, which keeps every 3×3 tower plaza connected while
    /// making directory paths produce different street plans.
    pub fn generate_city(
        &mut self,
        path: &Path,
        occupied: impl IntoIterator<Item = (i32, i32)>,
        seed_chance: u8,
        tower_stride: i32,
    ) {
        let occupied: HashSet<_> = occupied.into_iter().collect();
        let seed = stable_path_seed(path);
        self.city_theme = CityTheme::from_seed(seed);
        let mut protected = HashSet::new();

        // A one-cell asphalt ring means every tower can be approached from any
        // direction and closely packed towers still form a connected district.
        for &(x, z) in &occupied {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let cell = (x + dx, z + dz);
                    if self.contains(cell) {
                        protected.insert(cell);
                    }
                }
            }
        }
        for cell in self.parent_gate_approaches() {
            protected.insert(cell);
        }

        let hub = self
            .nearest_empty_cell(|cell| occupied.contains(&cell), self.max.0 - self.min.0 + 1)
            .unwrap_or_else(|| self.center());
        for dx in -1..=1 {
            for dz in -1..=1 {
                let cell = (hub.0 + dx, hub.1 + dz);
                if self.contains(cell) {
                    protected.insert(cell);
                }
            }
        }

        let stride = tower_stride.max(4);
        let x_offset = if seed & 1 == 0 { 2 } else { stride - 2 };
        let z_offset = if seed & 2 == 0 { 2 } else { stride - 2 };
        let mut roads = BTreeSet::new();

        // A perimeter boulevard creates a loop and ties every arterial together.
        for x in self.min.0..=self.max.0 {
            roads.insert((x, self.min.1));
            roads.insert((x, self.max.1));
        }
        for z in self.min.1..=self.max.1 {
            roads.insert((self.min.0, z));
            roads.insert((self.max.0, z));
        }

        // The regular lattice is cheap to generate even for huge directories,
        // unlike running a full-arena BFS for every filesystem landmark.
        for x in self.min.0..=self.max.0 {
            for z in self.min.1..=self.max.1 {
                if x.rem_euclid(stride) == x_offset || z.rem_euclid(stride) == z_offset {
                    roads.insert((x, z));
                }
            }
        }
        roads.extend(
            protected
                .iter()
                .copied()
                .filter(|cell| !occupied.contains(cell)),
        );

        // Join the spawn plaza and gate to the arterial lattice. Tower plazas
        // are already adjacent to a lattice line because stride is at least 4.
        for target in std::iter::once(hub).chain(self.parent_gate_approach()) {
            if let Some(route) = grid_path_to_any(self, target, &roads, &occupied, seed) {
                roads.extend(route);
            }
        }
        roads.retain(|cell| self.contains(*cell) && !occupied.contains(cell));

        let chance = seed_chance.min(100) as u64;
        let wall_budget = (occupied.len().saturating_mul(4)).clamp(48, 8_192).min(
            (self.max.0 - self.min.0 + 1) as usize * (self.max.1 - self.min.1 + 1) as usize / 3,
        );
        let mut candidates = Vec::new();
        for x in self.min.0..=self.max.0 {
            for z in self.min.1..=self.max.1 {
                let cell = (x, z);
                let hash = cell_hash(seed, cell);
                if hash % 100 < chance
                    && !occupied.contains(&cell)
                    && !protected.contains(&cell)
                    && !roads.contains(&cell)
                {
                    candidates.push((hash, cell));
                }
            }
        }
        candidates.sort_unstable();

        let mut walls = BTreeSet::new();
        for (hash, origin) in candidates {
            if walls.len() >= wall_budget {
                break;
            }
            let direction = if hash & 0x100 == 0 { (1, 0) } else { (0, 1) };
            let length = 2 + ((hash >> 9) % 3) as i32;
            for step in 0..length {
                let cell = (origin.0 + direction.0 * step, origin.1 + direction.1 * step);
                if self.contains(cell)
                    && !occupied.contains(&cell)
                    && !protected.contains(&cell)
                    && !roads.contains(&cell)
                    && walls.len() < wall_budget
                {
                    walls.insert(cell);
                }
            }
        }

        // Sparse rooms should still have a skyline even if no random run
        // survived the protected plazas and road lattice.
        let minimum = 6.min(wall_budget);
        if walls.len() < minimum {
            let mut fallback: Vec<_> = (self.min.0..=self.max.0)
                .flat_map(|x| (self.min.1..=self.max.1).map(move |z| (x, z)))
                .filter(|cell| {
                    !occupied.contains(cell) && !protected.contains(cell) && !roads.contains(cell)
                })
                .collect();
            fallback.sort_unstable_by_key(|cell| cell_hash(seed ^ 0xa11e_u64, *cell));
            for cell in fallback {
                walls.insert(cell);
                if walls.len() >= minimum {
                    break;
                }
            }
        }

        let mut structures: Vec<_> = walls
            .iter()
            .copied()
            .map(|cell| city_structure(seed, cell))
            .collect();
        let mut prominent: Vec<_> = (0..structures.len()).collect();
        prominent.sort_unstable_by_key(|&index| {
            let cell = structures[index].cell;
            ((cell.0 - hub.0).abs() + (cell.1 - hub.1).abs(), cell)
        });
        for (order, kind) in [
            CityStructureKind::Pylon,
            CityStructureKind::GlassFin,
            CityStructureKind::Barrier,
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(&index) = prominent.get(order) {
                structures[index].kind = kind;
                structures[index].height_tier = 2_u8.saturating_sub(order as u8);
                structures[index].accent = (order & 1) as u8;
            }
        }
        self.roads = roads;
        self.street_walls = walls;
        self.structures = structures;
    }

    fn parent_gate_approach(&self) -> Option<(i32, i32)> {
        let approaches = self.parent_gate_approaches();
        approaches.get(approaches.len() / 2).copied()
    }

    pub fn parent_gate_approaches(&self) -> Vec<(i32, i32)> {
        let Some(portal) = self.parent_portal else {
            return Vec::new();
        };
        let (from, to) = portal.along_span();
        (from..=to)
            .map(|along| match portal.wall {
                Wall::NegZ => (along, self.min.1),
                Wall::PosZ => (along, self.max.1),
                Wall::NegX => (self.min.0, along),
                Wall::PosX => (self.max.0, along),
            })
            .collect()
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

    /// Picks a spawn cell and heading with clear cells ahead of the cycle.
    ///
    /// [`Self::nearest_empty_cell`] only promises the spawn cell itself is free,
    /// which left a rider dropped into an unfamiliar folder reacting to whatever
    /// stood in the very next cell. This keeps the same bias toward the arena
    /// center but only accepts a candidate that can run `desired_runway` cells in
    /// a straight line, widening the search until one does.
    ///
    /// Returns `None` when no free cell has anywhere to go at all, which is not a
    /// spawn but a stall; the caller holds the run instead of starting it.
    pub fn spawn_with_runway(
        &self,
        mut blocked: impl FnMut((i32, i32)) -> bool,
        max_radius: i32,
        desired_runway: i32,
    ) -> Option<((i32, i32), Heading)> {
        let (cx, cz) = self.center();
        // Nothing exists beyond the arena's own extent, so this bounds the scan
        // when no cell anywhere can offer the full runway.
        let extent = (self.max.0 - self.min.0).max(self.max.1 - self.min.1);
        let mut fallback: Option<SpawnCandidate> = None;

        for radius in 0..=max_radius.min(extent) {
            let mut ring_best: Option<SpawnCandidate> = None;

            for x in (cx - radius)..=(cx + radius) {
                for z in (cz - radius)..=(cz + radius) {
                    let cell = (x, z);
                    let on_ring = (x - cx).abs().max((z - cz).abs()) == radius;
                    if !on_ring || !self.contains(cell) || blocked(cell) {
                        continue;
                    }
                    let Some((runway, heading)) =
                        self.longest_runway_heading(cell, &mut blocked, desired_runway)
                    else {
                        continue;
                    };

                    let candidate = SpawnCandidate {
                        cell,
                        heading,
                        runway,
                        distance_sq: (x - cx) * (x - cx) + (z - cz) * (z - cz),
                    };
                    if ring_best.is_none_or(|best| candidate.rank() > best.rank()) {
                        ring_best = Some(candidate);
                    }
                }
            }

            let Some(candidate) = ring_best else {
                continue;
            };
            if candidate.runway >= desired_runway {
                return Some((candidate.cell, candidate.heading));
            }
            if fallback.is_none_or(|best| candidate.rank() > best.rank()) {
                fallback = Some(candidate);
            }
        }

        fallback.map(|candidate| (candidate.cell, candidate.heading))
    }

    /// The heading out of `cell` with the most clear cells ahead of it.
    ///
    /// `None` means the cycle would be boxed in whichever way it faced.
    fn longest_runway_heading(
        &self,
        cell: (i32, i32),
        blocked: &mut impl FnMut((i32, i32)) -> bool,
        limit: i32,
    ) -> Option<(i32, Heading)> {
        Heading::PREFERENCE
            .into_iter()
            .enumerate()
            .map(|(rank, heading)| {
                (
                    self.clear_runway(cell, heading, blocked, limit),
                    rank,
                    heading,
                )
            })
            .filter(|&(runway, ..)| runway > 0)
            .max_by_key(|&(runway, rank, _)| (runway, std::cmp::Reverse(rank)))
            .map(|(runway, _, heading)| (runway, heading))
    }

    /// Counts the cells the cycle can cross from `cell` along `heading` before it
    /// would hit something, stopping at `limit`.
    ///
    /// Leaving the arena counts as blocked, which covers both the perimeter wall
    /// and the parent gate: aiming a fresh run at the gate would bounce the rider
    /// straight back out to the parent directory.
    fn clear_runway(
        &self,
        cell: (i32, i32),
        heading: Heading,
        blocked: &mut impl FnMut((i32, i32)) -> bool,
        limit: i32,
    ) -> i32 {
        let (dx, dz) = heading.delta();
        let mut cursor = cell;
        for step in 0..limit {
            cursor = (cursor.0 + dx, cursor.1 + dz);
            if !self.contains(cursor) || blocked(cursor) {
                return step;
            }
        }
        limit
    }
}

/// One cell the spawn search is weighing up, with the heading it would start on.
#[derive(Debug, Clone, Copy)]
struct SpawnCandidate {
    cell: (i32, i32),
    heading: Heading,
    runway: i32,
    distance_sq: i32,
}

impl SpawnCandidate {
    /// Sort key, highest wins: the longest runway, then the cell closest to the
    /// arena center, then a fixed cell order so one folder always spawns alike.
    fn rank(self) -> (i32, i32, i32, i32) {
        (self.runway, -self.distance_sq, -self.cell.0, -self.cell.1)
    }
}

/// Stable FNV-1a seed. Unlike `DefaultHasher`, this is deliberately fixed so a
/// directory keeps the same streets across application and Rust releases.
pub(crate) fn stable_path_seed(path: &Path) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn cell_hash(seed: u64, cell: (i32, i32)) -> u64 {
    let mut value = seed
        ^ (cell.0 as u32 as u64).wrapping_mul(0x9e3779b185ebca87)
        ^ (cell.1 as u32 as u64).wrapping_mul(0xc2b2ae3d27d4eb4f);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn city_structure(seed: u64, cell: (i32, i32)) -> CityStructure {
    let hash = cell_hash(seed ^ 0xc17c_1a7e_5eed_u64, cell);
    let kind = match hash % 100 {
        0..=49 => CityStructureKind::Barrier,
        50..=79 => CityStructureKind::GlassFin,
        _ => CityStructureKind::Pylon,
    };
    CityStructure {
        cell,
        kind,
        along_x: hash & 0x100 == 0,
        height_tier: ((hash >> 9) % 3) as u8,
        accent: ((hash >> 11) & 1) as u8,
        pulse_phase: ((hash >> 12) % 3) as u8,
    }
}

/// Finds a deterministic shortest route from `start` to the existing road
/// network while avoiding filesystem towers.
fn grid_path_to_any(
    arena: &Arena,
    start: (i32, i32),
    roads: &BTreeSet<(i32, i32)>,
    occupied: &HashSet<(i32, i32)>,
    seed: u64,
) -> Option<Vec<(i32, i32)>> {
    let directions = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let rotation = (cell_hash(seed, start) % directions.len() as u64) as usize;
    let mut queue = VecDeque::from([start]);
    let mut previous = HashMap::from([(start, start)]);
    let mut target = None;

    while let Some(cell) = queue.pop_front() {
        if roads.contains(&cell) {
            target = Some(cell);
            break;
        }
        for index in 0..directions.len() {
            let delta = directions[(index + rotation) % directions.len()];
            let next = (cell.0 + delta.0, cell.1 + delta.1);
            if arena.contains(next) && !occupied.contains(&next) && !previous.contains_key(&next) {
                previous.insert(next, cell);
                queue.push_back(next);
            }
        }
    }

    let mut cursor = target?;
    let mut path = vec![cursor];
    while cursor != start {
        cursor = previous[&cursor];
        path.push(cursor);
    }
    path.reverse();
    Some(path)
}

#[cfg(test)]
fn nearest_approach(
    arena: &Arena,
    tower: (i32, i32),
    hub: (i32, i32),
    occupied: &HashSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|delta| (tower.0 + delta.0, tower.1 + delta.1))
        .filter(|cell| arena.contains(*cell) && !occupied.contains(cell))
        .min_by_key(|cell| (cell.0 - hub.0).abs() + (cell.1 - hub.1).abs())
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
                CellContent::Markdown(index) => {
                    self.phase = RunPhase::EnteringDir;
                    self.pending_request = Some(EntryRequest::Document(index));
                    return StepOutcome::EnteringDocument(index);
                }
                CellContent::Source(index) => {
                    self.phase = RunPhase::EnteringDir;
                    self.pending_request = Some(EntryRequest::Source(index));
                    return StepOutcome::EnteringSource(index);
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
                CellContent::Opponent => {
                    return self.crash(CrashReason::Opponent);
                }
                CellContent::OpponentDisc => {
                    return self.crash(CrashReason::Disc);
                }
                CellContent::ParentPortal => {
                    self.phase = RunPhase::EnteringDir;
                    self.pending_request = Some(EntryRequest::Parent);
                    return StepOutcome::GoToParent;
                }
                CellContent::ClosePortal => {
                    self.phase = RunPhase::EnteringDir;
                    self.pending_request = Some(EntryRequest::Parent);
                    return StepOutcome::CloseDocument;
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
///
/// The three predicates separate the enterable file types: markdown opens a
/// page, source opens a disc-wars ring, and anything else is a hard crash.
pub fn classify_next_content(
    cell: (i32, i32),
    arena: &Arena,
    sim: &LightcycleSim,
    cells: &HashMap<(i32, i32), usize>,
    is_dir: impl Fn(usize) -> bool,
    is_markdown: impl Fn(usize) -> bool,
    is_source: impl Fn(usize) -> bool,
) -> CellContent {
    if arena
        .parent_portal
        .is_some_and(|portal| portal.contains(cell))
    {
        return if matches!(arena.kind, ArenaKind::Document | ArenaKind::Disc) {
            CellContent::ClosePortal
        } else {
            CellContent::ParentPortal
        };
    }
    if !arena.contains(cell) {
        return CellContent::Wall;
    }
    if arena.street_walls.contains(&cell) {
        return CellContent::Wall;
    }
    if cell != sim.cell && sim.trail.contains(&cell) {
        return CellContent::Trail;
    }
    if let Some(&index) = cells.get(&cell) {
        return if is_dir(index) {
            CellContent::Dir(index)
        } else if is_markdown(index) {
            CellContent::Markdown(index)
        } else if is_source(index) {
            CellContent::Source(index)
        } else {
            CellContent::File(index)
        };
    }
    CellContent::Empty
}

#[cfg(test)]
mod tests {
    use super::{
        Arena, ArenaKind, CellContent, CityTheme, CrashReason, EntryRequest, GatePlacement,
        Heading, LightcycleSim, RunPhase, StepOutcome, Turn, Wall, classify_next_content,
    };
    use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
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
                    kind: ArenaKind::Directory,
                    roads: BTreeSet::new(),
                    street_walls: BTreeSet::new(),
                    structures: Vec::new(),
                    city_theme: CityTheme::Cyan,
                },
                cells: HashMap::new(),
                is_dir: Vec::new(),
            }
        }

        fn classify(&self) -> impl FnMut((i32, i32), &LightcycleSim) -> CellContent + '_ {
            move |cell, sim| {
                classify_next_content(
                    cell,
                    &self.arena,
                    sim,
                    &self.cells,
                    |index| self.is_dir[index],
                    |_| false,
                    |_| false,
                )
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

    /// Counts the clear cells ahead of `cell`, walking the grid the way the sim
    /// will so the assertions do not lean on the search's own arithmetic.
    fn clear_ahead(
        arena: &Arena,
        blocked: impl Fn((i32, i32)) -> bool,
        cell: (i32, i32),
        heading: Heading,
        limit: i32,
    ) -> i32 {
        let (dx, dz) = heading.delta();
        let mut cursor = cell;
        for step in 0..limit {
            cursor = (cursor.0 + dx, cursor.1 + dz);
            if !arena.contains(cursor) || blocked(cursor) {
                return step;
            }
        }
        limit
    }

    const RUNWAY: i32 = 6;

    #[test]
    fn spawn_leaves_the_asked_for_runway_ahead_of_the_cycle() {
        let towers: Vec<_> = (0..5)
            .flat_map(|x| (0..5).map(move |z| (x * 5, z * 5)))
            .collect();
        let mut arena = Arena::from_nodes(towers.iter().copied(), Some(centered_gate()), 2, 13);
        arena.generate_city(&PathBuf::from("/dense/city"), towers.iter().copied(), 18, 5);

        let occupied: HashSet<_> = towers.iter().copied().collect();
        let blocked =
            |cell: (i32, i32)| occupied.contains(&cell) || arena.street_walls.contains(&cell);
        let (spawn, heading) = arena
            .spawn_with_runway(blocked, MAX_RADIUS, RUNWAY)
            .expect("a generated city has somewhere to ride");

        assert!(arena.contains(spawn) && !blocked(spawn));
        assert_eq!(clear_ahead(&arena, blocked, spawn, heading, RUNWAY), RUNWAY);
    }

    /// The old search only checked the single cell in front of the spawn, so a
    /// pocket like this one started the run a third of a second from a crash.
    #[test]
    fn spawn_moves_off_center_when_the_center_is_a_dead_end() {
        let pocket = [(2, 0), (-2, 0), (0, 2), (0, -2)];
        let arena = Arena::from_nodes(pocket.iter().copied(), None, 2, 13);
        let walls: HashSet<_> = pocket.iter().copied().collect();
        let blocked = |cell: (i32, i32)| walls.contains(&cell);

        let center = arena.center();
        assert_eq!(center, (0, 0));
        for heading in [Heading::PosX, Heading::PosZ, Heading::NegX, Heading::NegZ] {
            assert_eq!(
                clear_ahead(&arena, blocked, center, heading, RUNWAY),
                1,
                "the pocket should box the center in after one cell"
            );
        }

        let (spawn, heading) = arena
            .spawn_with_runway(blocked, MAX_RADIUS, RUNWAY)
            .expect("the open arena around the pocket is rideable");
        assert_ne!(spawn, center, "spawning in the pocket is the bug");
        assert_eq!(clear_ahead(&arena, blocked, spawn, heading, RUNWAY), RUNWAY);
    }

    /// Short of the target the search still hands back the roomiest cell it saw,
    /// rather than refusing to start a run in a cramped folder.
    #[test]
    fn a_cramped_arena_still_spawns_facing_its_longest_run() {
        let arena = Arena::from_nodes(std::iter::empty::<(i32, i32)>(), None, PADDING, 5);
        let blocked = |cell: (i32, i32)| cell.0 != 0 && cell.1 != 0;

        let (spawn, heading) = arena
            .spawn_with_runway(blocked, MAX_RADIUS, RUNWAY)
            .expect("the open cross through the middle is rideable");
        let runway = clear_ahead(&arena, blocked, spawn, heading, RUNWAY);
        assert!(runway > 0 && runway < RUNWAY);
        assert_eq!(
            runway,
            [Heading::PosX, Heading::PosZ, Heading::NegX, Heading::NegZ]
                .into_iter()
                .map(|heading| clear_ahead(&arena, blocked, spawn, heading, RUNWAY))
                .max()
                .unwrap(),
            "it should face the longest run available from that cell"
        );
    }

    /// Folder sizes that used to spawn hard against something. A folder with two
    /// entries gave the rider a single clear cell, under a third of a second at
    /// `LIGHTCYCLE_CELLS_PER_SEC`, and small folders were the worst because their
    /// arenas are tight and their plazas sit right where the spawn search looked.
    #[test]
    fn generated_cities_all_spawn_with_the_full_runway() {
        for count in [0i32, 1, 2, 3, 5, 9, 17, 40, 120] {
            for name in ["/a", "/usr/share/doc", "/home/user/projects/raptor"] {
                let width = ((count as f32).sqrt().ceil() as i32).max(1);
                let towers: Vec<(i32, i32)> = (0..count)
                    .map(|index| {
                        (
                            (index % width - width / 2) * 5,
                            (index / width - width / 2) * 5,
                        )
                    })
                    .collect();
                let path = PathBuf::from(format!("{name}/{count}"));
                let mut arena =
                    Arena::from_nodes(towers.iter().copied(), Some(centered_gate()), 2, 13);
                arena.generate_city(&path, towers.iter().copied(), 18, 5);

                let occupied: HashSet<_> = towers.iter().copied().collect();
                let blocked = |cell: (i32, i32)| {
                    occupied.contains(&cell) || arena.street_walls.contains(&cell)
                };
                let (spawn, heading) = arena
                    .spawn_with_runway(blocked, MAX_RADIUS, RUNWAY)
                    .unwrap_or_else(|| panic!("no spawn for {count} entries in {name}"));

                assert_eq!(
                    clear_ahead(&arena, blocked, spawn, heading, RUNWAY),
                    RUNWAY,
                    "{count} entries in {name} spawned short of the runway"
                );

                // The runway must not come at the cost of exiling the rider to
                // the edge of the city, away from the folder's towers.
                let center = arena.center();
                let offset = (spawn.0 - center.0).abs() + (spawn.1 - center.1).abs();
                assert!(offset <= 2, "spawned {offset} cells off center in {name}");
            }
        }
    }

    #[test]
    fn a_boxed_in_cell_is_not_a_spawn() {
        let arena = Arena::from_nodes(std::iter::empty::<(i32, i32)>(), None, PADDING, 9);
        assert!(
            arena
                .spawn_with_runway(|cell| cell != (0, 0), MAX_RADIUS, RUNWAY)
                .is_none(),
            "a single free cell with no exit has to stall the run, not start it"
        );
        assert!(
            arena
                .spawn_with_runway(|_| true, MAX_RADIUS, RUNWAY)
                .is_none()
        );
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
    fn next_cell_is_markdown_requests_document_not_crash() {
        let layout = TestLayout::new(&[(1, 0)], &[], None);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        let outcome = sim.advance(1.0, |cell, sim| {
            classify_next_content(
                cell,
                &layout.arena,
                sim,
                &layout.cells,
                |_| false,
                |_| true,
                |_| false,
            )
        });
        assert_eq!(outcome, StepOutcome::EnteringDocument(0));
        assert_eq!(sim.pending_request, Some(EntryRequest::Document(0)));
        assert_eq!(sim.phase, RunPhase::EnteringDir);
    }

    #[test]
    fn next_cell_is_source_requests_a_ring_not_a_crash() {
        let layout = TestLayout::new(&[(1, 0)], &[], None);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        let outcome = sim.advance(1.0, |cell, sim| {
            classify_next_content(
                cell,
                &layout.arena,
                sim,
                &layout.cells,
                |_| false,
                |_| false,
                |_| true,
            )
        });
        assert_eq!(outcome, StepOutcome::EnteringSource(0));
        assert_eq!(sim.pending_request, Some(EntryRequest::Source(0)));
        assert_eq!(sim.phase, RunPhase::EnteringDir);
    }

    #[test]
    fn a_source_file_still_crashes_when_the_source_predicate_is_absent() {
        let layout = TestLayout::new(&[(1, 0)], &[], None);
        let mut sim = LightcycleSim::start((0, 0), Heading::PosX);
        let outcome = sim.advance(1.0, |cell, sim| {
            classify_next_content(
                cell,
                &layout.arena,
                sim,
                &layout.cells,
                |_| false,
                |_| false,
                |_| false,
            )
        });
        assert_eq!(outcome, StepOutcome::Crashed(CrashReason::File));
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

    fn generated_arena(path: &str) -> (Arena, HashSet<(i32, i32)>) {
        let occupied: HashSet<_> = [(-10, -10), (0, 0), (10, 10)].into_iter().collect();
        let mut arena = Arena::from_nodes(occupied.iter().copied(), Some(centered_gate()), 2, 25);
        arena.generate_city(
            PathBuf::from(path).as_path(),
            occupied.iter().copied(),
            35,
            5,
        );
        (arena, occupied)
    }

    fn reachable_streets(
        arena: &Arena,
        start: (i32, i32),
        occupied: &HashSet<(i32, i32)>,
    ) -> HashSet<(i32, i32)> {
        let mut reached = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(cell) = queue.pop_front() {
            for delta in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = (cell.0 + delta.0, cell.1 + delta.1);
                if arena.contains(next)
                    && !occupied.contains(&next)
                    && !arena.street_walls.contains(&next)
                    && reached.insert(next)
                {
                    queue.push_back(next);
                }
            }
        }
        reached
    }

    fn reachable_roads(arena: &Arena, start: (i32, i32)) -> HashSet<(i32, i32)> {
        let mut reached = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(cell) = queue.pop_front() {
            for delta in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = (cell.0 + delta.0, cell.1 + delta.1);
                if arena.roads.contains(&next) && reached.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        reached
    }

    #[test]
    fn generated_streets_are_stable_per_path() {
        let (first, _) = generated_arena("/projects/raptor");
        let (second, _) = generated_arena("/projects/raptor");
        assert_eq!(first, second);
        assert!(!first.street_walls.is_empty());
    }

    #[test]
    fn a_two_file_room_still_gets_generated_scenery() {
        let occupied = [(-5, 0), (0, 0)];
        let mut arena = Arena::from_nodes(occupied, Some(centered_gate()), 2, 13);
        arena.generate_city(PathBuf::from("/two-files").as_path(), occupied, 18, 5);
        assert!(
            !arena.street_walls.is_empty(),
            "landmark protection and route carving erased every generated wall"
        );
        for kind in [
            super::CityStructureKind::Barrier,
            super::CityStructureKind::GlassFin,
            super::CityStructureKind::Pylon,
        ] {
            assert!(
                arena
                    .structures
                    .iter()
                    .any(|structure| structure.kind == kind)
            );
        }
    }

    #[test]
    fn different_paths_generate_different_streets() {
        let (first, _) = generated_arena("/projects/raptor");
        let (second, _) = generated_arena("/projects/another");
        assert!(
            first.street_walls != second.street_walls
                || first.roads != second.roads
                || first.city_theme != second.city_theme
        );
    }

    #[test]
    fn generation_keeps_connected_road_plazas_around_every_tower() {
        let (arena, occupied) = generated_arena("/projects/raptor");
        for &(x, z) in &occupied {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let cell = (x + dx, z + dz);
                    assert!(
                        !arena.street_walls.contains(&cell),
                        "wall generated beside tower {:?}",
                        (x, z)
                    );
                    if cell != (x, z) {
                        assert!(arena.roads.contains(&cell));
                    }
                }
            }
        }
    }

    #[test]
    fn arterial_network_is_connected_and_contains_loops() {
        let (arena, _) = generated_arena("/projects/raptor");
        let start = *arena.roads.iter().next().unwrap();
        assert_eq!(reachable_roads(&arena, start).len(), arena.roads.len());

        let edges = arena
            .roads
            .iter()
            .map(|&(x, z)| {
                usize::from(arena.roads.contains(&(x + 1, z)))
                    + usize::from(arena.roads.contains(&(x, z + 1)))
            })
            .sum::<usize>();
        assert!(
            edges >= arena.roads.len(),
            "connected road graph has no alternate-route cycle"
        );
    }

    #[test]
    fn roads_structures_and_towers_never_overlap() {
        let (arena, occupied) = generated_arena("/projects/raptor");
        assert!(arena.roads.is_disjoint(&arena.street_walls));
        assert!(occupied.iter().all(|cell| !arena.roads.contains(cell)));
        assert!(
            occupied
                .iter()
                .all(|cell| !arena.street_walls.contains(cell))
        );
        let structure_cells: BTreeSet<_> = arena
            .structures
            .iter()
            .map(|structure| structure.cell)
            .collect();
        assert_eq!(structure_cells, arena.street_walls);
    }

    #[test]
    fn every_tower_and_parent_gate_remain_reachable() {
        let (arena, occupied) = generated_arena("/projects/raptor");
        let hub = arena
            .nearest_empty_cell(
                |cell| occupied.contains(&cell) || arena.street_walls.contains(&cell),
                MAX_RADIUS,
            )
            .unwrap();
        let reachable = reachable_streets(&arena, hub, &occupied);

        for tower in occupied.iter().copied() {
            let approach = super::nearest_approach(&arena, tower, hub, &occupied).unwrap();
            assert!(
                reachable.contains(&approach),
                "tower {tower:?} was cut off at {approach:?}"
            );
        }
        let gate = arena.parent_gate_approach().unwrap();
        assert!(reachable.contains(&gate), "parent gate was cut off");
        for approach in arena.parent_gate_approaches() {
            assert!(
                !arena.street_walls.contains(&approach),
                "generated wall narrowed the parent gate at {approach:?}"
            );
            assert!(
                arena.roads.contains(&approach),
                "gate approach is not part of the road network at {approach:?}"
            );
        }
    }

    #[test]
    fn dense_four_by_four_room_has_visible_architecture() {
        let occupied: Vec<_> = (0..4)
            .flat_map(|z| (0..4).map(move |x| (x * 5, z * 5)))
            .collect();
        let mut arena = Arena::from_nodes(occupied.iter().copied(), None, 2, 13);
        arena.generate_city(
            PathBuf::from("/dense-four-by-four").as_path(),
            occupied.iter().copied(),
            18,
            5,
        );
        assert!(arena.structures.len() >= 6);
        assert!(
            arena
                .structures
                .iter()
                .any(|structure| structure.kind == super::CityStructureKind::Pylon)
        );
    }

    #[test]
    fn empty_room_gets_a_connected_plaza_and_skyline() {
        let mut arena = Arena::from_nodes([], None, 2, 13);
        arena.generate_city(PathBuf::from("/empty").as_path(), [], 18, 5);
        assert!(arena.structures.len() >= 6);
        let start = *arena.roads.iter().next().unwrap();
        assert_eq!(reachable_roads(&arena, start).len(), arena.roads.len());
    }

    #[test]
    fn large_city_structure_count_is_bounded() {
        let occupied: Vec<_> = (0..48)
            .flat_map(|z| (0..64).map(move |x| (x * 5, z * 5)))
            .collect();
        let mut arena = Arena::from_nodes(occupied.iter().copied(), None, 2, 13);
        arena.generate_city(
            PathBuf::from("/large-city").as_path(),
            occupied.iter().copied(),
            100,
            5,
        );
        assert!(arena.structures.len() <= 8_192);
    }

    #[test]
    fn generated_street_walls_are_lethal() {
        let (arena, _) = generated_arena("/projects/raptor");
        let wall = *arena.street_walls.iter().next().unwrap();
        let sim = LightcycleSim::start(arena.center(), Heading::PosX);
        assert_eq!(
            classify_next_content(
                wall,
                &arena,
                &sim,
                &HashMap::new(),
                |_| false,
                |_| false,
                |_| false,
            ),
            CellContent::Wall
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
                kind: ArenaKind::Directory,
                roads: BTreeSet::new(),
                street_walls: BTreeSet::new(),
                structures: Vec::new(),
                city_theme: CityTheme::Cyan,
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
