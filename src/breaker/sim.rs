//! Bevy-free brick breaker: the bike is the paddle.
//!
//! The court is upright on the `X`/`Y` plane, `X` across and `Y` up. A ball
//! bounces off the side and top walls, the paddle reflects it, and a brick it
//! touches breaks. The wall below the paddle is the one that ends the run: if
//! the ball gets past the bike, it is gone. Clearing every brick clears the
//! level, which is how this run is left.

use crate::config;

/// One brick in the wall. `col`/`row` are grid coordinates, `0` at the top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brick {
    pub col: i32,
    pub row: i32,
    pub alive: bool,
}

/// The ball, as a square of `2 * BREAKER_BALL_RADIUS` a side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// Waiting to be launched off the paddle.
    pub held: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerPhase {
    /// Ball on the paddle, waiting for the launch.
    Ready,
    Running,
    /// Every brick is gone.
    Cleared,
    /// The ball got past the bike.
    Missed,
}

impl BreakerPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Running => "BREAKING",
            Self::Cleared => "CLEARED",
            Self::Missed => "MISSED",
        }
    }
}

/// What one fixed step produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BreakerEvents {
    pub launched: bool,
    pub bounced_off_paddle: bool,
    pub broke_bricks: usize,
    pub cleared: bool,
    pub missed: bool,
}

/// One input frame: a lateral axis and an edge-triggered launch.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BreakerInput {
    /// `-1.0`, `0.0` or `1.0` along the paddle's rail.
    pub slide: f32,
    pub launch: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreakerSim {
    pub bricks: Vec<Brick>,
    pub ball: Ball,
    /// Centre of the paddle, on the `X` axis.
    pub paddle_x: f32,
    pub phase: BreakerPhase,
    pub court: (f32, f32),
    pub paddle_width: f32,
    /// Latched from the keyboard each frame; `launch` is an edge.
    pub input: BreakerInput,
    seed: u64,
}

impl BreakerSim {
    /// Lays out a wall of bricks from `seed`.
    pub fn new(seed: u64) -> Self {
        let (width, height) = (config::BREAKER_WIDTH, config::BREAKER_HEIGHT);
        let cols = config::BREAKER_COLS;
        let rows = config::BREAKER_ROWS;
        let mut rng = seed | 1;
        let mut bricks = Vec::with_capacity((cols * rows) as usize);
        for row in 0..rows {
            for col in 0..cols {
                // Punch a few gaps so the wall is not a solid slab, but keep the
                // bottom rows mostly intact.
                let hole = next_unit(&mut rng) < config::BREAKER_HOLE_CHANCE;
                bricks.push(Brick {
                    col,
                    row,
                    alive: !(hole && row > 0),
                });
            }
        }

        Self {
            bricks,
            ball: Ball {
                x: 0.0,
                y: config::BREAKER_PADDLE_Y + config::BREAKER_BALL_RADIUS,
                vx: 0.0,
                vy: 0.0,
                held: true,
            },
            paddle_x: 0.0,
            phase: BreakerPhase::Ready,
            court: (width, height),
            paddle_width: config::BREAKER_PADDLE_WIDTH,
            input: BreakerInput::default(),
            seed,
        }
    }

    /// Live bricks left.
    pub fn remaining(&self) -> usize {
        self.bricks.iter().filter(|brick| brick.alive).count()
    }

    /// Latches a frame's input. `slide` is held; `launch` is an edge.
    pub fn set_input(&mut self, slide: f32, launch: bool) {
        self.input = BreakerInput { slide, launch };
    }

    /// Left edge of a brick in court space.
    pub fn brick_box(&self, brick: &Brick) -> (f32, f32, f32, f32) {
        let stride = config::BREAKER_BRICK_WIDTH + config::BREAKER_BRICK_GAP;
        let wall =
            self.court.0 - (config::BREAKER_COLS as f32 * stride - config::BREAKER_BRICK_GAP);
        let x = -self.court.0 * 0.5 + wall * 0.5 + brick.col as f32 * stride;
        let y = self.court.1 - config::BREAKER_WALL_TOP - brick.row as f32 * stride;
        (
            x,
            y - config::BREAKER_BRICK_HEIGHT,
            config::BREAKER_BRICK_WIDTH,
            config::BREAKER_BRICK_HEIGHT,
        )
    }

    /// Advances one fixed step.
    pub fn update(&mut self, dt: f32) -> BreakerEvents {
        let mut events = BreakerEvents::default();
        let input = std::mem::take(&mut self.input);
        self.input.slide = input.slide;

        if matches!(self.phase, BreakerPhase::Cleared | BreakerPhase::Missed) {
            return events;
        }

        // Slide, clamped so the paddle stays inside the court.
        let half = self.court.0 * 0.5 - self.paddle_width * 0.5;
        self.paddle_x = (self.paddle_x
            + input.slide.clamp(-1.0, 1.0) * config::BREAKER_PADDLE_SPEED * dt)
            .clamp(-half, half);

        if self.phase == BreakerPhase::Ready {
            self.ball.x = self.paddle_x;
            self.ball.y = config::BREAKER_PADDLE_Y + config::BREAKER_BALL_RADIUS;
            if input.launch {
                self.ball.held = false;
                // Off the paddle at a slight angle so it never travels straight
                // up and stalls.
                let angle = config::BREAKER_LAUNCH_ANGLE;
                self.ball.vx = angle.sin() * config::BREAKER_BALL_SPEED * self.launch_bias();
                self.ball.vy = angle.cos() * config::BREAKER_BALL_SPEED;
                self.phase = BreakerPhase::Running;
                events.launched = true;
            }
            return events;
        }

        if self.ball.held {
            self.ball.x = self.paddle_x;
            return events;
        }

        // Move in small slices so a fast ball cannot tunnel through a brick.
        let speed = (self.ball.vx * self.ball.vx + self.ball.vy * self.ball.vy).sqrt();
        let steps = ((speed * dt) / config::BREAKER_MAX_STEP).ceil().max(1.0) as u32;
        let slice = dt / steps as f32;
        for _ in 0..steps {
            if matches!(self.phase, BreakerPhase::Cleared | BreakerPhase::Missed) {
                break;
            }
            self.advance(slice, &mut events);
        }
        events
    }

    /// One collision slice.
    fn advance(&mut self, dt: f32, events: &mut BreakerEvents) {
        let radius = config::BREAKER_BALL_RADIUS;
        self.ball.x += self.ball.vx * dt;
        self.ball.y += self.ball.vy * dt;

        // Side walls.
        let limit = self.court.0 * 0.5 - radius;
        if self.ball.x < -limit {
            self.ball.x = -limit;
            self.ball.vx = self.ball.vx.abs();
        } else if self.ball.x > limit {
            self.ball.x = limit;
            self.ball.vx = -self.ball.vx.abs();
        }
        // Ceiling.
        if self.ball.y > self.court.1 - radius {
            self.ball.y = self.court.1 - radius;
            self.ball.vy = -self.ball.vy.abs();
        }

        // The bike: reflect, with the angle set by where it landed.
        let paddle_half = self.paddle_width * 0.5;
        if self.ball.vy < 0.0
            && self.ball.y - radius <= config::BREAKER_PADDLE_Y + config::BREAKER_PADDLE_HEIGHT
            && self.ball.y + radius >= config::BREAKER_PADDLE_Y
            && (self.ball.x - self.paddle_x).abs() <= paddle_half + radius
        {
            let offset = ((self.ball.x - self.paddle_x) / paddle_half).clamp(-1.0, 1.0);
            let deflect = offset * config::BREAKER_MAX_DEFLECT;
            let speed = (self.ball.vx * self.ball.vx + self.ball.vy * self.ball.vy)
                .sqrt()
                .max(config::BREAKER_BALL_SPEED);
            self.ball.vx = deflect * speed;
            self.ball.vy = (1.0 - deflect * deflect).max(0.0).sqrt() * speed;
            self.ball.y = config::BREAKER_PADDLE_Y + config::BREAKER_PADDLE_HEIGHT + radius;
            events.bounced_off_paddle = true;
        }

        // Bricks.
        for index in 0..self.bricks.len() {
            if !self.bricks[index].alive {
                continue;
            }
            let brick = self.bricks[index];
            let (bx, by, bw, bh) = self.brick_box(&brick);
            let overlaps = self.ball.x + radius > bx
                && self.ball.x - radius < bx + bw
                && self.ball.y + radius > by
                && self.ball.y - radius < by + bh;
            if !overlaps {
                continue;
            }
            self.bricks[index].alive = false;
            events.broke_bricks += 1;
            // Come off whichever face is nearer, so the ball does not stick.
            let from_side = (self.ball.y - (by + bh * 0.5)).abs() < bh * 0.5 + radius * 0.5
                && (self.ball.x - (bx + bw * 0.5)).abs() > bw * 0.5 - radius;
            if from_side {
                self.ball.vx = -self.ball.vx;
            } else {
                self.ball.vy = -self.ball.vy;
            }
            break;
        }

        // The wall below the bike: past the paddle line is the end of the run.
        if self.ball.y + radius < 0.0 {
            self.phase = BreakerPhase::Missed;
            events.missed = true;
            return;
        }

        if self.remaining() == 0 {
            self.phase = BreakerPhase::Cleared;
            events.cleared = true;
        }
    }

    /// Which way the ball leaves the paddle, alternating per serve so the level
    /// does not open the same way every time.
    fn launch_bias(&self) -> f32 {
        if self.seed & 1 == 0 { 1.0 } else { -1.0 }
    }

    /// Restarts from the same seed, as `R` does.
    pub fn restart(&mut self) {
        let fresh = Self::new(self.seed);
        *self = fresh;
    }
}

/// Next value in `0.0..1.0` from a plain LCG, so walls are reproducible.
fn next_unit(rng: &mut u64) -> f32 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*rng >> 40) as f32 / (1_u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::{BreakerPhase, BreakerSim};
    use crate::config;

    fn sim(seed: u64) -> BreakerSim {
        BreakerSim::new(seed)
    }

    fn serve(level: &mut BreakerSim) {
        level.set_input(0.0, true);
        level.update(1.0 / 60.0);
        assert_eq!(level.phase, BreakerPhase::Running);
    }

    #[test]
    fn the_same_seed_lays_out_the_same_wall() {
        assert_eq!(sim(3).bricks, sim(3).bricks);
        assert!(sim(3).remaining() > 0);
    }

    #[test]
    fn the_ball_waits_on_the_paddle_until_launched() {
        let mut level = sim(1);
        assert_eq!(level.phase, BreakerPhase::Ready);
        assert!(level.ball.held);
        level.set_input(0.0, false);
        for _ in 0..30 {
            level.update(1.0 / 60.0);
        }
        assert!(level.ball.held, "the ball should not drift on its own");
        serve(&mut level);
        assert!(!level.ball.held);
    }

    #[test]
    fn the_paddle_stays_inside_the_court() {
        let mut level = sim(2);
        for _ in 0..600 {
            level.set_input(1.0, false);
            level.update(1.0 / 60.0);
        }
        let limit = level.court.0 * 0.5 - level.paddle_width * 0.5;
        assert!(level.paddle_x <= limit + 0.001, "paddle ran off the right");
        for _ in 0..1200 {
            level.set_input(-1.0, false);
            level.update(1.0 / 60.0);
        }
        assert!(level.paddle_x >= -limit - 0.001, "paddle ran off the left");
    }

    #[test]
    fn a_served_ball_breaks_bricks() {
        let mut level = sim(4);
        serve(&mut level);
        let before = level.remaining();
        let mut broke = 0;
        for _ in 0..6_000 {
            // Track the ball so the run does not end before it hits anything.
            let slide = ((level.ball.x - level.paddle_x) * 0.5).clamp(-1.0, 1.0);
            level.set_input(slide, false);
            let events = level.update(1.0 / 60.0);
            broke += events.broke_bricks;
            if level.phase != BreakerPhase::Running {
                break;
            }
        }
        assert!(broke > 0, "the ball never reached the wall");
        assert!(level.remaining() < before);
    }

    #[test]
    fn clearing_the_wall_clears_the_level() {
        let mut level = sim(5);
        level
            .bricks
            .iter_mut()
            .for_each(|brick| brick.alive = false);
        level.bricks[0].alive = true;
        serve(&mut level);
        // Sit the ball right under the last brick and let it fly into it.
        let brick = level.bricks[0];
        let (bx, by, bw, _) = level.brick_box(&brick);
        level.ball.x = bx + bw * 0.5;
        level.ball.y = by - config::BREAKER_BALL_RADIUS - 0.01;
        level.ball.vx = 0.0;
        level.ball.vy = config::BREAKER_BALL_SPEED;
        let events = level.update(1.0 / 60.0);
        assert_eq!(events.broke_bricks, 1);
        assert_eq!(level.phase, BreakerPhase::Cleared);
        assert!(events.cleared);
    }

    #[test]
    fn a_ball_past_the_paddle_ends_the_run() {
        let mut level = sim(6);
        serve(&mut level);
        // Drop it down the middle with the paddle parked far away.
        level.paddle_x = level.court.0 * 0.5;
        level.ball.x = -level.court.0 * 0.5 + 1.0;
        level.ball.y = 1.0;
        level.ball.vx = 0.0;
        level.ball.vy = -config::BREAKER_BALL_SPEED;
        let mut missed = false;
        for _ in 0..600 {
            if level.update(1.0 / 60.0).missed {
                missed = true;
                break;
            }
        }
        assert!(missed, "the ball should have fallen past the bike");
        assert_eq!(level.phase, BreakerPhase::Missed);
    }

    #[test]
    fn the_paddle_sends_the_ball_back_up() {
        let mut level = sim(7);
        serve(&mut level);
        level.ball.x = level.paddle_x;
        level.ball.y = config::BREAKER_PADDLE_Y + config::BREAKER_PADDLE_HEIGHT + 0.3;
        level.ball.vx = 0.0;
        level.ball.vy = -config::BREAKER_BALL_SPEED;
        let events = level.update(1.0 / 60.0);
        assert!(
            events.bounced_off_paddle,
            "the bike should rebound the ball"
        );
        assert!(level.ball.vy > 0.0, "the ball must come back up");
    }

    #[test]
    fn restarting_relays_the_wall() {
        let mut level = sim(8);
        serve(&mut level);
        level
            .bricks
            .iter_mut()
            .for_each(|brick| brick.alive = false);
        level.restart();
        assert_eq!(level.phase, BreakerPhase::Ready);
        assert_eq!(level.bricks, sim(8).bricks);
        assert!(level.ball.held);
    }
}
