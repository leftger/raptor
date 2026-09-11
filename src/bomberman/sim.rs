//! Bevy-free Bomberman: blast crates, then reach the unlocked exit.
//!
//! The room is a grid of cells on the X/Z plane. Border walls and seeded crates
//! are solid; the cycle walks cell to cell, plants bombs, and the cross-shaped
//! blasts destroy crates (and the cycle, if it is still in the way). Clearing
//! every crate unlocks the exit at the far corner; reaching it wins.

use crate::config;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bomb {
    pub cell: (i32, i32),
    pub fuse: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomberPhase {
    Walking,
    Won,
    Lost,
}

impl BomberPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Walking => "WALKING",
            Self::Won => "ESCAPED",
            Self::Lost => "BLASTED",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BomberEvents {
    pub planted: bool,
    pub crates: u32,
    pub lost_life: bool,
    pub cleared: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BomberSim {
    pub cell: (i32, i32),
    pub crates: BTreeSet<(i32, i32)>,
    pub exit: (i32, i32),
    pub bombs: Vec<Bomb>,
    pub lives: u8,
    pub phase: BomberPhase,
    pub invuln: f32,
    move_clock: f32,
    seed: u64,
    lines: usize,
}

impl BomberSim {
    pub fn new(seed: u64, lines: usize) -> Self {
        let mut rng = seed | 1;
        let mut crates = BTreeSet::new();
        let mut tries = 0;
        while crates.len() < config::BOMBER_CRATES && tries < config::BOMBER_CRATES * 40 {
            tries += 1;
            let cell = (
                1 + (unit(&mut rng) * (config::BOMBER_COLS - 2) as f32) as i32,
                1 + (unit(&mut rng) * (config::BOMBER_ROWS - 2) as f32) as i32,
            );
            if cell == (1, 1)
                || cell == (config::BOMBER_COLS - 2, config::BOMBER_ROWS - 2)
                || cell == (config::BOMBER_COLS - 2, 1)
            {
                continue;
            }
            crates.insert(cell);
        }
        let _ = lines;
        Self {
            cell: (1, 1),
            crates,
            exit: (config::BOMBER_COLS - 2, config::BOMBER_ROWS - 2),
            bombs: Vec::new(),
            lives: config::BOMBER_LIVES,
            phase: BomberPhase::Walking,
            invuln: config::BOMBER_INVULN,
            move_clock: 0.0,
            seed,
            lines,
        }
    }

    /// World centre of a room cell, for the renderer.
    pub fn center(cell: (i32, i32)) -> (f32, f32) {
        let x = (cell.0 as f32 - (config::BOMBER_COLS - 1) as f32 * 0.5) * config::GRID_SPACING;
        let z = (cell.1 as f32 - (config::BOMBER_ROWS - 1) as f32 * 0.5) * config::GRID_SPACING;
        (x, z)
    }

    pub fn solid(&self, cell: (i32, i32)) -> bool {
        cell.0 < 0
            || cell.0 >= config::BOMBER_COLS
            || cell.1 < 0
            || cell.1 >= config::BOMBER_ROWS
            || self.crates.contains(&cell)
    }

    /// Steps one cell in the held direction, if the room allows it.
    pub fn step(&mut self, dx: i32, dz: i32) {
        if self.phase != BomberPhase::Walking || (dx == 0 && dz == 0) {
            return;
        }
        let next = (self.cell.0 + dx, self.cell.1 + dz);
        if !self.solid(next) {
            self.cell = next;
        }
    }

    /// Plants a bomb under the cycle, up to the pool limit.
    pub fn plant(&mut self) -> bool {
        if self.phase != BomberPhase::Walking || self.bombs.len() >= config::BOMBER_MAX_BOMBS {
            return false;
        }
        if self.bombs.iter().any(|bomb| bomb.cell == self.cell) {
            return false;
        }
        self.bombs.push(Bomb {
            cell: self.cell,
            fuse: config::BOMBER_FUSE,
        });
        true
    }

    /// Advances one frame: fuses burn, blasts clear crates, the exit opens.
    pub fn update(&mut self, dt: f32) -> BomberEvents {
        let mut events = BomberEvents::default();
        if self.phase != BomberPhase::Walking {
            return events;
        }
        self.invuln = (self.invuln - dt).max(0.0);

        let mut blasts = Vec::new();
        for bomb in &mut self.bombs {
            bomb.fuse -= dt;
            if bomb.fuse <= 0.0 {
                blasts.push(bomb.cell);
            }
        }
        self.bombs.retain(|bomb| bomb.fuse > 0.0);

        for blast in blasts {
            let mut hit = BTreeSet::new();
            hit.insert(blast);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                for reach in 1..=config::BOMBER_BLAST {
                    let cell = (blast.0 + dx * reach, blast.1 + dz * reach);
                    if cell.0 < 0
                        || cell.0 >= config::BOMBER_COLS
                        || cell.1 < 0
                        || cell.1 >= config::BOMBER_ROWS
                    {
                        break;
                    }
                    hit.insert(cell);
                }
            }
            for cell in hit {
                if self.crates.remove(&cell) {
                    events.crates += 1;
                }
                if self.invuln <= 0.0 && self.cell == cell {
                    events.lost_life = true;
                    self.lives = self.lives.saturating_sub(1);
                    if self.lives == 0 {
                        self.phase = BomberPhase::Lost;
                        return events;
                    }
                    self.invuln = config::BOMBER_INVULN;
                }
            }
        }

        if self.cell == self.exit && self.crates.is_empty() {
            self.phase = BomberPhase::Won;
            events.cleared = true;
        }
        events
    }

    pub fn restart(&mut self) {
        *self = Self::new(self.seed, self.lines);
    }
}

fn unit(rng: &mut u64) -> f32 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*rng >> 40) as f32 / (1_u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::{BomberPhase, BomberSim};
    use crate::config;

    fn sim() -> BomberSim {
        BomberSim::new(5, 200)
    }

    #[test]
    fn the_same_seed_builds_the_same_room() {
        assert_eq!(sim(), sim());
    }

    #[test]
    fn walking_is_blocked_by_walls_and_crates() {
        let mut room = sim();
        room.crates.insert((2, 1));
        room.step(1, 0);
        assert_eq!(room.cell, (1, 1), "a crate should block the step");
        room.crates.clear();
        room.step(1, 0);
        assert_eq!(room.cell, (2, 1));
    }

    #[test]
    fn a_bomb_blast_clears_crates() {
        let mut room = sim();
        room.crates.clear();
        room.crates.insert((2, 1));
        room.plant();
        room.invuln = 999.0;
        let mut crates = 0;
        for _ in 0..300 {
            crates += room.update(1.0 / 60.0).crates;
        }
        assert_eq!(crates, 1, "the blast should clear the crate");
        assert!(room.crates.is_empty());
    }

    #[test]
    fn clearing_crates_and_reaching_the_exit_wins() {
        let mut room = sim();
        room.crates.clear();
        room.cell = room.exit;
        let events = room.update(1.0 / 60.0);
        assert!(events.cleared);
        assert_eq!(room.phase, BomberPhase::Won);
    }

    #[test]
    fn a_blast_on_the_cycle_costs_a_life() {
        let mut room = sim();
        room.crates.clear();
        room.plant();
        room.invuln = 0.0;
        let mut lost = false;
        for _ in 0..300 {
            if room.update(1.0 / 60.0).lost_life {
                lost = true;
                break;
            }
        }
        assert!(lost, "standing on the bomb should hurt");
        assert_eq!(room.lives, config::BOMBER_LIVES - 1);
    }

    #[test]
    fn restarting_relays_the_room() {
        let mut room = sim();
        room.crates.clear();
        room.restart();
        assert_eq!(room.crates, sim().crates);
        assert_eq!(room.phase, BomberPhase::Walking);
    }
}
