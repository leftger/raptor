//! Bevy-free Plinko: balls dropped through seeded pins into scored buckets.
//!
//! The board is upright on the X/Y plane. The cycle slides along the top and
//! drops one ball per press; balls fall through rows of pins, deflect off them,
//! and land in one of eight buckets. Drop the whole rack; beat the seeded score
//! target and the board is cleared.

use crate::config;

#[derive(Debug, Clone, PartialEq)]
pub struct Pin {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub scored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlinkoPhase {
    Dropping,
    Won,
    Lost,
}

impl PlinkoPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dropping => "DROPPING",
            Self::Won => "CLEARED",
            Self::Lost => "BUSTED",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlinkoEvents {
    pub dropped: bool,
    pub scored: u32,
    pub cleared: bool,
    pub lost: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlinkoSim {
    pub aim: f32,
    pub pins: Vec<Pin>,
    pub balls: Vec<Ball>,
    pub balls_left: usize,
    pub score: u32,
    pub target: u32,
    pub phase: PlinkoPhase,
    rng: u64,
    seed: u64,
    lines: usize,
}

impl PlinkoSim {
    pub fn new(seed: u64, lines: usize) -> Self {
        let mut rng = seed | 1;
        let mut pins = Vec::new();
        for row in 0..config::PLINKO_PIN_ROWS {
            let y = config::PLINKO_HEIGHT * 0.75 - row as f32 * 2.4;
            let count = 7 + row % 2;
            for index in 0..count {
                let x =
                    (index as f32 - (count - 1) as f32 * 0.5) * 2.0 + (unit(&mut rng) - 0.5) * 0.4;
                pins.push(Pin { x, y });
            }
        }
        let target = config::PLINKO_TARGET + (unit(&mut rng) * 250.0) as u32;
        let _ = lines;
        Self {
            aim: 0.0,
            pins,
            balls: Vec::new(),
            balls_left: config::PLINKO_BALLS,
            score: 0,
            target,
            phase: PlinkoPhase::Dropping,
            rng,
            seed,
            lines,
        }
    }

    /// Bucket scores, seeded so each board reads differently.
    pub fn bucket_scores(&self) -> [u32; 8] {
        let mut scores = [0_u32; 8];
        let mut rng = self.seed.wrapping_mul(31).wrapping_add(7) | 1;
        for slot in &mut scores {
            *slot = 40 + (unit(&mut rng) * 120.0) as u32;
        }
        scores
    }

    pub fn set_aim(&mut self, aim: f32) {
        self.aim = aim.clamp(
            -config::PLINKO_WIDTH * 0.5 + 1.0,
            config::PLINKO_WIDTH * 0.5 - 1.0,
        );
    }

    pub fn drop_ball(&mut self) -> bool {
        if self.phase != PlinkoPhase::Dropping || self.balls_left == 0 {
            return false;
        }
        self.balls_left -= 1;
        self.balls.push(Ball {
            x: self.aim,
            y: config::PLINKO_HEIGHT * 0.5 - 1.0,
            vx: 0.0,
            vy: -6.0,
            scored: false,
        });
        true
    }

    /// Advances one frame: balls fall, bounce off pins and walls, and land.
    pub fn update(&mut self, dt: f32) -> PlinkoEvents {
        let mut events = PlinkoEvents::default();
        if self.phase != PlinkoPhase::Dropping {
            return events;
        }

        let bucket_scores = self.bucket_scores();
        for ball in &mut self.balls {
            if ball.scored {
                continue;
            }
            ball.vy -= 26.0 * dt;
            ball.x += ball.vx * dt;
            ball.y += ball.vy * dt;

            let half = config::PLINKO_WIDTH * 0.5;
            if ball.x < -half + 0.4 {
                ball.x = -half + 0.4;
                ball.vx = ball.vx.abs();
            } else if ball.x > half - 0.4 {
                ball.x = half - 0.4;
                ball.vx = -ball.vx.abs();
            }

            for pin in &self.pins {
                let dx = ball.x - pin.x;
                let dy = ball.y - pin.y;
                if dx * dx + dy * dy <= 0.55 * 0.55 {
                    let push = if dx.abs() > 0.01 {
                        dx.signum() * 5.0
                    } else if unit(&mut self.rng) > 0.5 {
                        5.0
                    } else {
                        -5.0
                    };
                    ball.vx = push;
                    ball.vy = -ball.vy.abs() * 0.55;
                    break;
                }
            }

            if ball.y < -config::PLINKO_HEIGHT * 0.5 {
                ball.scored = true;
                let slots = 8;
                let slot = (((ball.x + config::PLINKO_WIDTH * 0.5) / config::PLINKO_WIDTH)
                    * slots as f32) as usize;
                let slot = slot.min(slots - 1);
                let score = bucket_scores[slot];
                self.score += score;
                events.scored = score;
            }
        }
        self.balls.retain(|ball| !ball.scored);

        if self.balls_left == 0 && self.balls.is_empty() {
            if self.score >= self.target {
                self.phase = PlinkoPhase::Won;
                events.cleared = true;
            } else {
                self.phase = PlinkoPhase::Lost;
                events.lost = true;
            }
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
    use super::{PlinkoPhase, PlinkoSim};
    use crate::config;

    fn sim() -> PlinkoSim {
        PlinkoSim::new(5, 200)
    }

    #[test]
    fn the_same_seed_builds_the_same_board() {
        assert_eq!(sim(), sim());
    }

    #[test]
    fn aiming_stays_on_the_rail() {
        let mut board = sim();
        board.set_aim(999.0);
        assert!(board.aim <= config::PLINKO_WIDTH * 0.5);
        board.set_aim(-999.0);
        assert!(board.aim >= -config::PLINKO_WIDTH * 0.5);
    }

    #[test]
    fn dropping_spends_a_ball() {
        let mut board = sim();
        assert!(board.drop_ball());
        assert_eq!(board.balls_left, config::PLINKO_BALLS - 1);
        assert_eq!(board.balls.len(), 1);
    }

    #[test]
    fn a_ball_eventually_scores() {
        let mut board = sim();
        board.drop_ball();
        let mut scored = 0;
        for _ in 0..2_000 {
            scored += board.update(1.0 / 60.0).scored;
            if board.balls.is_empty() {
                break;
            }
        }
        assert!(scored > 0, "the ball should land in a bucket");
    }

    #[test]
    fn the_rack_decides_the_board() {
        let mut board = sim();
        board.balls_left = 0;
        board.balls.clear();
        board.score = board.target;
        let events = board.update(1.0 / 60.0);
        assert!(events.cleared);
        assert_eq!(board.phase, PlinkoPhase::Won);
    }

    #[test]
    fn restarting_relays_the_board() {
        let mut board = sim();
        board.pins.clear();
        board.restart();
        assert_eq!(board.pins, sim().pins);
        assert_eq!(board.phase, PlinkoPhase::Dropping);
    }
}
