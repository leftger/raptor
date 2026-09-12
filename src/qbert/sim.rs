//! Bevy-free Q*bert: diagonal hops that light a cube pyramid.
//!
//! The pyramid has `QBERT_ROWS` rows; row `r` has `r + 1` cubes. The cycle hops
//! to a neighbouring cube on each keypress, lighting the cube it lands on, while
//! two enemies hop randomly between cubes. Light every cube to win; hop off the
//! pyramid or into an enemy and a life is spent.

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QbertPhase {
    Hopping,
    Won,
    Lost,
}

impl QbertPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hopping => "HOPPING",
            Self::Won => "LIT",
            Self::Lost => "FELL",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QbertEvents {
    pub hopped: bool,
    pub lit: u32,
    pub lost_life: bool,
    pub cleared: bool,
}

/// One enemy, hopping between cubes on a fixed clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enemy {
    pub row: usize,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QbertSim {
    pub row: usize,
    pub index: usize,
    pub lit: Vec<bool>,
    pub enemies: Vec<Enemy>,
    pub lives: u8,
    pub phase: QbertPhase,
    pub invuln: f32,
    enemy_clock: f32,
    rng: u64,
    seed: u64,
    lines: usize,
}

impl QbertSim {
    pub fn new(seed: u64, lines: usize) -> Self {
        let cubes = (0..config::QBERT_ROWS).map(|row| row + 1).sum::<usize>();
        let rng = seed | 1;
        let enemies = vec![
            Enemy {
                row: config::QBERT_ROWS - 1,
                index: 0,
            },
            Enemy {
                row: config::QBERT_ROWS - 1,
                index: config::QBERT_ROWS - 1,
            },
        ];
        let _ = lines;
        let mut sim = Self {
            row: 0,
            index: 0,
            lit: vec![false; cubes],
            enemies,
            lives: config::QBERT_LIVES,
            phase: QbertPhase::Hopping,
            invuln: config::QBERT_INVULN,
            enemy_clock: config::QBERT_ENEMY_STEP,
            rng,
            seed,
            lines,
        };
        sim.light(sim.row, sim.index);
        sim
    }

    /// Cube index for a row/index pair.
    pub fn cube_index(row: usize, index: usize) -> usize {
        (0..row).map(|row| row + 1).sum::<usize>() + index
    }

    /// World position of a cube, for the renderer.
    pub fn cube_position(row: usize, index: usize) -> (f32, f32) {
        let x = (index as f32 - row as f32 * 0.5) * config::QBERT_CUBE_SPACING;
        let z = (config::QBERT_ROWS as f32 - 1.0 - row as f32) * config::QBERT_CUBE_SPACING * 0.85;
        (x, z)
    }

    fn light(&mut self, row: usize, index: usize) {
        self.lit[Self::cube_index(row, index)] = true;
    }

    /// Hops in one of the four diagonal directions, if a cube is there.
    pub fn hop(&mut self, dx: i32, dz: i32) {
        if self.phase != QbertPhase::Hopping {
            return;
        }
        let next = match (dx, dz) {
            // A = down-left, D = down-right, W = up-left, S = up-right.
            (-1, 0) => Some((self.row + 1, self.index)),
            (1, 0) => Some((self.row + 1, self.index + 1)),
            (0, 1) => self.row.checked_sub(1).map(|row| (row, self.index)),
            (0, -1) => self
                .row
                .checked_sub(1)
                .and_then(|row| (self.index > 0).then(|| (row, self.index - 1))),
            _ => None,
        };
        let Some((row, index)) = next else {
            return;
        };
        if row < config::QBERT_ROWS && index <= row {
            self.row = row;
            self.index = index;
            self.light(row, index);
        }
    }

    /// Advances one frame: moves enemies and checks the pyramid.
    pub fn update(&mut self, dt: f32) -> QbertEvents {
        let mut events = QbertEvents::default();
        if self.phase != QbertPhase::Hopping {
            return events;
        }
        self.invuln = (self.invuln - dt).max(0.0);

        self.enemy_clock -= dt;
        if self.enemy_clock <= 0.0 {
            self.enemy_clock = config::QBERT_ENEMY_STEP;
            for index in 0..self.enemies.len() {
                let (row, col) = (self.enemies[index].row, self.enemies[index].index);
                let options = self.neighbours(row, col);
                let pick = (self.unit() * options.len() as f32) as usize;
                if let Some(&(row, col)) = options.get(pick.min(options.len().saturating_sub(1))) {
                    self.enemies[index].row = row;
                    self.enemies[index].index = col;
                }
            }
        }

        if self.invuln <= 0.0
            && self
                .enemies
                .iter()
                .any(|enemy| enemy.row == self.row && enemy.index == self.index)
        {
            events.lost_life = true;
            self.lives = self.lives.saturating_sub(1);
            if self.lives == 0 {
                self.phase = QbertPhase::Lost;
                return events;
            }
            self.invuln = config::QBERT_INVULN;
            self.row = 0;
            self.index = 0;
        }

        if self.lit.iter().all(|lit| *lit) {
            self.phase = QbertPhase::Won;
            events.cleared = true;
        }
        events
    }

    fn neighbours(&self, row: usize, index: usize) -> Vec<(usize, usize)> {
        let mut options = Vec::new();
        if row + 1 < config::QBERT_ROWS {
            options.push((row + 1, index));
            options.push((row + 1, index + 1));
        }
        if row > 0 {
            if index < row {
                options.push((row - 1, index));
            }
            if index > 0 {
                options.push((row - 1, index - 1));
            }
        }
        options
    }

    fn unit(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 40) as f32 / (1_u32 << 24) as f32
    }

    pub fn restart(&mut self) {
        *self = Self::new(self.seed, self.lines);
    }
}

#[cfg(test)]
mod tests {
    use super::{QbertPhase, QbertSim};
    use crate::config;

    fn sim() -> QbertSim {
        QbertSim::new(5, 200)
    }

    #[test]
    fn the_same_seed_builds_the_same_pyramid() {
        assert_eq!(sim(), sim());
    }

    #[test]
    fn hops_light_cubes_and_stay_on_the_pyramid() {
        let mut game = sim();
        game.hop(-1, 0); // down-left from the top
        assert_eq!((game.row, game.index), (1, 0));
        game.hop(1, 0); // down-right
        assert_eq!((game.row, game.index), (2, 1));
        game.hop(1, 0);
        game.hop(1, 0);
        game.hop(1, 0);
        let (row, index) = (game.row, game.index);
        game.hop(1, 0); // off the bottom edge, ignored
        assert_eq!((game.row, game.index), (row, index));
    }

    #[test]
    fn hopping_up_right_from_the_left_edge_is_safe() {
        let mut game = sim();
        game.row = 1;
        game.index = 0;
        game.hop(0, -1); // no cube up-right of the left edge: must not panic
        assert_eq!((game.row, game.index), (1, 0));
    }

    #[test]
    fn enemies_move_on_their_clock() {
        let mut game = sim();
        let before = game.enemies.clone();
        game.update(config::QBERT_ENEMY_STEP);
        assert_ne!(game.enemies, before, "enemies should hop on the clock");
    }

    #[test]
    fn lighting_every_cube_wins() {
        let mut game = sim();
        game.lit.fill(true);
        let events = game.update(1.0 / 60.0);
        assert!(events.cleared);
        assert_eq!(game.phase, QbertPhase::Won);
    }

    #[test]
    fn an_enemy_on_the_same_cube_costs_a_life() {
        let mut game = sim();
        game.invuln = 0.0;
        game.enemies[0] = super::Enemy {
            row: game.row,
            index: game.index,
        };
        let events = game.update(1.0 / 60.0);
        assert!(events.lost_life);
        assert_eq!(game.lives, config::QBERT_LIVES - 1);
    }

    #[test]
    fn restarting_relays_the_pyramid() {
        let mut game = sim();
        game.lit.clear();
        game.restart();
        assert_eq!(game.lit, sim().lit);
        assert_eq!(game.phase, QbertPhase::Hopping);
    }
}
