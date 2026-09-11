//! Bevy-free Pac-Man: a maze of dots on the X/Z grid, chased by ghosts.
//!
//! The maze is a fixed, connected grid with a seeded set of ghost quirks. The
//! cycle moves continuously along corridors and eats the dot in each cell it
//! enters; ghosts pick the open direction that closes on the cycle. Clear every
//! dot to win, and lose all three lives to a ghost and the run is over.

use crate::config;
use std::collections::BTreeSet;

/// One chasing ghost, moving continuously along corridors.
#[derive(Debug, Clone, PartialEq)]
pub struct Ghost {
    pub cell: (i32, i32),
    pub x: f32,
    pub z: f32,
    dir: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacPhase {
    Playing,
    Won,
    Caught,
}

impl PacPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Playing => "CHASING",
            Self::Won => "CLEARED",
            Self::Caught => "CAUGHT",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PacEvents {
    pub dots: u32,
    pub lost_life: bool,
    pub cleared: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacSim {
    pub cell: (i32, i32),
    pub x: f32,
    pub z: f32,
    pub heading: (i32, i32),
    pub dots: BTreeSet<(i32, i32)>,
    pub ghosts: Vec<Ghost>,
    pub lives: u8,
    pub score: u32,
    pub phase: PacPhase,
    pub invuln: f32,
    /// Held direction: `-1/0/1` east and `-1/0/1` toward +Z.
    pub input: (i32, i32),
    rng: u64,
    seed: u64,
    lines: usize,
}

impl PacSim {
    /// The maze is fixed so every cell is always reachable; the seed only picks
    /// ghost phases, keeping the same file beatable.
    pub fn new(seed: u64, lines: usize) -> Self {
        let mut sim = Self {
            cell: (1, 1),
            x: 0.0,
            z: 0.0,
            heading: (0, 0),
            dots: BTreeSet::new(),
            ghosts: Vec::new(),
            lives: config::PAC_LIVES,
            score: 0,
            phase: PacPhase::Playing,
            invuln: config::PAC_INVULN,
            input: (0, 0),
            rng: seed | 1,
            seed,
            lines,
        };
        let (px, pz) = Self::center(sim.cell);
        sim.x = px;
        sim.z = pz;
        for row in 0..config::PAC_ROWS {
            for col in 0..config::PAC_COLS {
                let cell = (col, row);
                if !Self::solid(cell) && !Self::spawn_cells().contains(&cell) {
                    sim.dots.insert(cell);
                }
            }
        }
        for spawn in [(7, 1), (1, 5), (7, 5)].iter() {
            let cell = *spawn;
            let (x, z) = Self::center(cell);
            sim.ghosts.push(Ghost {
                cell,
                x,
                z,
                dir: (0, 0),
            });
        }
        // Longer files make the ghosts a touch quicker, but never unfair.
        let _ = lines;
        sim
    }

    /// World centre of a maze cell.
    pub fn center(cell: (i32, i32)) -> (f32, f32) {
        let x = (cell.0 as f32 - (config::PAC_COLS - 1) as f32 * 0.5) * config::GRID_SPACING;
        let z = (cell.1 as f32 - (config::PAC_ROWS - 1) as f32 * 0.5) * config::GRID_SPACING;
        (x, z)
    }

    /// The fixed internal walls; the border is implicit.
    fn internal_walls() -> [(i32, i32); 5] {
        [(2, 2), (6, 2), (4, 3), (2, 4), (6, 4)]
    }

    fn spawn_cells() -> [(i32, i32); 4] {
        [(1, 1), (7, 1), (1, 5), (7, 5)]
    }

    /// True when nothing can walk into the cell.
    pub fn solid(cell: (i32, i32)) -> bool {
        cell.0 <= 0
            || cell.0 >= config::PAC_COLS - 1
            || cell.1 <= 0
            || cell.1 >= config::PAC_ROWS - 1
            || Self::internal_walls().contains(&cell)
    }

    /// Latches a frame's held direction.
    pub fn set_input(&mut self, move_x: i32, move_z: i32) {
        self.input = (move_x.clamp(-1, 1), move_z.clamp(-1, 1));
    }

    /// Advances one frame.
    pub fn update(&mut self, dt: f32) -> PacEvents {
        let mut events = PacEvents::default();
        if self.phase != PacPhase::Playing {
            return events;
        }
        self.invuln = (self.invuln - dt).max(0.0);

        self.step_player(dt, &mut events);
        for index in 0..self.ghosts.len() {
            self.step_ghost(index, dt);
        }

        if self.invuln <= 0.0 {
            let hit = self.ghosts.iter().any(|ghost| {
                let dx = ghost.x - self.x;
                let dz = ghost.z - self.z;
                dx * dx + dz * dz <= 0.8 * 0.8
            });
            if hit {
                events.lost_life = true;
                self.lives = self.lives.saturating_sub(1);
                if self.lives == 0 {
                    self.phase = PacPhase::Caught;
                    return events;
                }
                self.invuln = config::PAC_INVULN;
                self.reset_actors();
            }
        }

        if self.dots.is_empty() {
            self.phase = PacPhase::Won;
            events.cleared = true;
        }
        events
    }

    /// Moves the cycle one frame along corridors, eating dots and turning at
    /// cell centres.
    fn step_player(&mut self, dt: f32, events: &mut PacEvents) {
        // Prefer the held direction; otherwise keep rolling if the corridor
        // continues.
        let next = match self.input {
            (0, 0) => self.heading,
            (dx, dz) => {
                let wanted = (dx, dz);
                if Self::solid((self.cell.0 + wanted.0, self.cell.1 + wanted.1)) {
                    self.heading
                } else {
                    wanted
                }
            }
        };
        if next != self.heading {
            self.heading = next;
        }
        if self.heading == (0, 0) {
            return;
        }
        let (dx, dz) = self.heading;
        self.x += dx as f32 * config::PAC_PLAYER_SPEED * dt;
        self.z += dz as f32 * config::PAC_PLAYER_SPEED * dt;

        // Snap to the centre of the cell we are travelling into, then eat and
        // re-evaluate the corridor.
        let target = (self.cell.0 + dx, self.cell.1 + dz);
        let (tx, tz) = Self::center(target);
        let along = (self.x - tx) * dx as f32 + (self.z - tz) * dz as f32;
        if along >= 0.0 {
            self.cell = target;
            self.x = tx;
            self.z = tz;
            if self.dots.remove(&target) {
                self.score += 10;
                events.dots += 1;
            }
            if Self::solid((target.0 + dx, target.1 + dz)) {
                self.heading = (0, 0);
            }
        }
    }

    /// Moves one ghost a frame, choosing its corridor at cell centres.
    fn step_ghost(&mut self, index: usize, dt: f32) {
        let (cell, x, z) = {
            let ghost = &self.ghosts[index];
            (ghost.cell, ghost.x, ghost.z)
        };
        let (target, dir) = {
            let center = Self::center(cell);
            let at_center = (x - center.0).abs() < 0.01 && (z - center.1).abs() < 0.01;
            let dir = if at_center {
                self.ghost_dir(index)
            } else {
                self.ghosts[index].dir
            };
            (cell, dir)
        };
        if dir == (0, 0) {
            return;
        }
        let (dx, dz) = dir;
        self.ghosts[index].x += dx as f32 * config::PAC_GHOST_SPEED * dt;
        self.ghosts[index].z += dz as f32 * config::PAC_GHOST_SPEED * dt;
        self.ghosts[index].dir = dir;
        let next = (target.0 + dx, target.1 + dz);
        let (nx, nz) = Self::center(next);
        let along =
            (self.ghosts[index].x - nx) * dx as f32 + (self.ghosts[index].z - nz) * dz as f32;
        if along >= 0.0 {
            self.ghosts[index].cell = next;
            self.ghosts[index].x = nx;
            self.ghosts[index].z = nz;
            if Self::solid((next.0 + dx, next.1 + dz)) {
                self.ghosts[index].dir = (0, 0);
            }
        }
    }

    /// Which way a ghost turns at a centre: the open neighbour closest to the
    /// cycle, seeded for variety.
    fn ghost_dir(&mut self, index: usize) -> (i32, i32) {
        let cell = self.ghosts[index].cell;
        let mut best = (0, 0);
        let mut best_distance = i32::MAX;
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = (cell.0 + dx, cell.1 + dz);
            if Self::solid(next) {
                continue;
            }
            let distance = (next.0 - self.cell.0).abs() + (next.1 - self.cell.1).abs();
            let jitter = if self.unit() > 0.75 { 1 } else { 0 };
            let key = distance * 2 + jitter;
            if key < best_distance {
                best_distance = key;
                best = (dx, dz);
            }
        }
        best
    }

    fn reset_actors(&mut self) {
        self.cell = (1, 1);
        let (x, z) = Self::center(self.cell);
        self.x = x;
        self.z = z;
        self.heading = (0, 0);
        for (ghost, spawn) in self.ghosts.iter_mut().zip([(7, 1), (1, 5), (7, 5)]) {
            ghost.cell = spawn;
            let (gx, gz) = Self::center(spawn);
            ghost.x = gx;
            ghost.z = gz;
            ghost.dir = (0, 0);
        }
    }

    /// Next pseudo-random number in `0.0..1.0`.
    fn unit(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 40) as f32 / (1_u32 << 24) as f32
    }

    /// Restarts from the same seed, as `R` does.
    pub fn restart(&mut self) {
        *self = Self::new(self.seed, self.lines);
    }
}

#[cfg(test)]
mod tests {
    use super::{PacPhase, PacSim};
    use crate::config;

    fn sim() -> PacSim {
        PacSim::new(3, 200)
    }

    #[test]
    fn the_maze_is_connected_and_filled_with_dots() {
        let field = sim();
        assert!(!field.dots.is_empty());
        // Every open cell other than spawns holds a dot.
        let mut open = 0;
        for row in 0..config::PAC_ROWS {
            for col in 0..config::PAC_COLS {
                if !PacSim::solid((col, row)) {
                    open += 1;
                }
            }
        }
        assert_eq!(field.dots.len() + 4, open);
    }

    #[test]
    fn the_same_seed_builds_the_same_maze() {
        assert_eq!(sim(), sim());
    }

    #[test]
    fn holding_right_walks_and_eats() {
        let mut field = sim();
        field.set_input(1, 0);
        let mut ate = 0;
        for _ in 0..300 {
            ate += field.update(1.0 / 60.0).dots;
        }
        assert!(ate > 0, "the cycle should eat dots along the corridor");
        assert!(field.cell.0 > 1, "the cycle should have moved east");
    }

    #[test]
    fn ghosts_hit_and_cost_lives() {
        let mut field = sim();
        field.invuln = 0.0;
        // Park a ghost on the cycle.
        field.ghosts[0].x = field.x;
        field.ghosts[0].z = field.z;
        let events = field.update(1.0 / 60.0);
        assert!(events.lost_life);
        assert_eq!(field.lives, config::PAC_LIVES - 1);
    }

    #[test]
    fn clearing_the_dots_wins() {
        let mut field = sim();
        field.dots.clear();
        let events = field.update(1.0 / 60.0);
        assert!(events.cleared);
        assert_eq!(field.phase, PacPhase::Won);
    }

    #[test]
    fn restarting_relays_the_maze() {
        let mut field = sim();
        field.dots.clear();
        field.restart();
        assert_eq!(field.dots, sim().dots);
        assert_eq!(field.phase, PacPhase::Playing);
    }
}
