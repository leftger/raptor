//! Bevy-free Frogger: hop the cycle across a highway of moving code.
//!
//! The board is a grid of cells on the X/Z plane; the cycle starts on the
//! bottom row and hops one cell per keypress. Five lanes of obstacles sweep
//! across, wrapping at the edges. Reaching the far row wins; touching an
//! obstacle spends a life and sends the cycle back to the start.

use crate::config;

/// One moving obstacle lane. Obstacles wrap around the row.
#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    pub row: i32,
    pub offset: f32,
    pub speed: f32,
    pub gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FroggerPhase {
    Hopping,
    Won,
    Splatted,
}

impl FroggerPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hopping => "HOPPING",
            Self::Won => "ACROSS",
            Self::Splatted => "SPLATTED",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FroggerEvents {
    pub hopped: bool,
    pub splatted: bool,
    pub cleared: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FroggerSim {
    pub cell: (i32, i32),
    pub lanes: Vec<Lane>,
    pub lives: u8,
    pub phase: FroggerPhase,
    pub invuln: f32,
    seed: u64,
    lines: usize,
}

impl FroggerSim {
    pub fn new(seed: u64, lines: usize) -> Self {
        let mut rng = seed | 1;
        let mut lanes = Vec::with_capacity(config::FROGGER_LANES as usize);
        for row in 1..=config::FROGGER_LANES {
            let speed = 1.6 + unit(&mut rng) * 2.4;
            let gap = 3.0 + unit(&mut rng) * 2.0;
            lanes.push(Lane {
                row,
                offset: unit(&mut rng) * config::FROGGER_COLS as f32,
                speed: if row % 2 == 0 { -speed } else { speed },
                gap,
            });
        }
        let _ = lines;
        Self {
            cell: (config::FROGGER_COLS / 2, config::FROGGER_ROWS - 1),
            lanes,
            lives: config::FROGGER_LIVES,
            phase: FroggerPhase::Hopping,
            invuln: config::FROGGER_INVULN,
            seed,
            lines,
        }
    }

    /// World centre of a board cell, for the renderer.
    pub fn center(cell: (i32, i32)) -> (f32, f32) {
        let x = (cell.0 as f32 - (config::FROGGER_COLS - 1) as f32 * 0.5) * config::GRID_SPACING;
        let z = (cell.1 as f32 - (config::FROGGER_ROWS - 1) as f32 * 0.5) * config::GRID_SPACING;
        (x, z)
    }

    /// Hops one cell, if the edge allows it.
    pub fn hop(&mut self, dx: i32, dz: i32) {
        if self.phase != FroggerPhase::Hopping || (dx == 0 && dz == 0) {
            return;
        }
        let next = (self.cell.0 + dx, self.cell.1 + dz);
        if next.0 < 0
            || next.0 >= config::FROGGER_COLS
            || next.1 < 0
            || next.1 >= config::FROGGER_ROWS
        {
            return;
        }
        self.cell = next;
    }

    /// Advances one frame: moves the lanes and checks the crossing.
    pub fn update(&mut self, dt: f32) -> FroggerEvents {
        let mut events = FroggerEvents::default();
        if self.phase != FroggerPhase::Hopping {
            return events;
        }
        self.invuln = (self.invuln - dt).max(0.0);

        for lane in &mut self.lanes {
            lane.offset += lane.speed * dt;
            let span = config::FROGGER_COLS as f32;
            lane.offset = (lane.offset % span + span) % span;
        }

        if self.invuln <= 0.0 && self.cell.1 >= 1 && self.cell.1 <= config::FROGGER_LANES {
            let lane = &self.lanes[(self.cell.1 - 1) as usize];
            let at = (self.cell.0 as f32 - lane.offset).rem_euclid(config::FROGGER_COLS as f32);
            if at < 0.9 {
                events.splatted = true;
                self.lives = self.lives.saturating_sub(1);
                if self.lives == 0 {
                    self.phase = FroggerPhase::Splatted;
                    return events;
                }
                self.invuln = config::FROGGER_INVULN;
                self.cell = (config::FROGGER_COLS / 2, config::FROGGER_ROWS - 1);
            }
        }

        if self.cell.1 == 0 {
            self.phase = FroggerPhase::Won;
            events.cleared = true;
        }
        events
    }

    /// Which cell an obstacle occupies, for the renderer.
    pub fn obstacle_cells(&self) -> Vec<(i32, i32)> {
        let mut cells = Vec::new();
        for lane in &self.lanes {
            for step in 0..2 {
                let offset = (lane.offset + step as f32 * lane.gap) % config::FROGGER_COLS as f32;
                cells.push((offset as i32, lane.row));
            }
        }
        cells
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
    use super::{FroggerPhase, FroggerSim};
    use crate::config;

    fn sim() -> FroggerSim {
        FroggerSim::new(5, 200)
    }

    #[test]
    fn the_same_seed_builds_the_same_highway() {
        assert_eq!(sim(), sim());
    }

    #[test]
    fn hopping_moves_one_cell_and_stays_in_bounds() {
        let mut game = sim();
        let start = game.cell;
        game.hop(1, 0);
        assert_eq!(game.cell, (start.0 + 1, start.1));
        game.cell = (0, game.cell.1);
        game.hop(-1, 0);
        assert_eq!(game.cell.0, 0, "hopping off the board should be ignored");
    }

    #[test]
    fn lanes_wrap_around_the_row() {
        let mut game = sim();
        game.lanes[0].speed = 20.0;
        game.lanes[0].offset = 0.0;
        for _ in 0..120 {
            game.update(1.0 / 60.0);
        }
        assert!(game.lanes[0].offset >= 0.0 && game.lanes[0].offset < config::FROGGER_COLS as f32);
    }

    #[test]
    fn reaching_the_far_row_wins() {
        let mut game = sim();
        game.cell = (config::FROGGER_COLS / 2, 1);
        game.invuln = 999.0;
        game.hop(0, -1);
        let events = game.update(1.0 / 60.0);
        assert!(events.cleared);
        assert_eq!(game.phase, FroggerPhase::Won);
    }

    #[test]
    fn an_obstacle_splats_and_resets() {
        let mut game = sim();
        game.invuln = 0.0;
        game.cell = (config::FROGGER_COLS / 2, 1);
        // Park a stationary obstacle exactly on the cycle's cell.
        game.lanes[0].speed = 0.0;
        game.lanes[0].offset = game.cell.0 as f32;
        let events = game.update(1.0 / 60.0);
        assert!(events.splatted);
        assert_eq!(game.lives, config::FROGGER_LIVES - 1);
        assert_eq!(
            game.cell,
            (config::FROGGER_COLS / 2, config::FROGGER_ROWS - 1)
        );
    }

    #[test]
    fn restarting_relays_the_highway() {
        let mut game = sim();
        game.lanes.clear();
        game.restart();
        assert_eq!(game.lanes, sim().lanes);
        assert_eq!(game.phase, FroggerPhase::Hopping);
    }
}
