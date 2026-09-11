//! Bevy-free river surfer: a hoverbike runs a procedural river to the finish.
//!
//! The course runs down the `Z` axis of the lightcycle world, on the `X`/`Z`
//! plane every other game uses. The river is a ribbon whose centreline sways
//! with a seeded sine, and the bike rides it with an always-on throttle: steer
//! to stay between the banks, hold boost through the gates, and cross the
//! finish gate at the far end. Running onto a bank or into a rock ends the run.

use crate::config;

/// A rock sticking out of the river. Touching one ends the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rock {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
}

/// A floating gate. Riding through it gives a burst of speed, once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoostGate {
    pub x: f32,
    pub z: f32,
    pub taken: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurferPhase {
    Riding,
    /// Crossed the finish gate.
    Finished,
    /// Beached on a bank or wrapped around a rock.
    Crashed,
}

impl SurferPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Riding => "RIDING",
            Self::Finished => "FINISHED",
            Self::Crashed => "WRECKED",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurferEvents {
    pub boosted: bool,
    pub banked: bool,
    pub hit_rock: bool,
    pub finished: bool,
}

/// One input frame: a held lateral axis and a held boost.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SurferInput {
    /// `-1.0` left, `1.0` right.
    pub steer: f32,
    pub boost: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurferSim {
    /// World position on the `X`/`Z` plane.
    pub x: f32,
    pub z: f32,
    /// Facing on the `X`/`Z` plane: `0` is `+X`, growing toward `+Z`.
    pub heading: f32,
    pub speed: f32,
    /// Hover height over the water, with the wave bob included.
    pub height: f32,
    pub phase: SurferPhase,
    /// Length of the course, in world units.
    pub length: f32,
    /// Half width of the playable river.
    pub width: f32,
    pub rocks: Vec<Rock>,
    pub gates: Vec<BoostGate>,
    /// Latched from the keyboard each frame.
    pub input: SurferInput,
    time: f32,
    seed: u64,
    lines: usize,
}

impl SurferSim {
    /// Lays out a river course from `seed`, scaled by how many lines the source
    /// file holds.
    pub fn new(seed: u64, lines: usize) -> Self {
        let length = (lines as f32 * config::SURFER_METRES_PER_LINE)
            .clamp(config::SURFER_MIN_LENGTH, config::SURFER_MAX_LENGTH);
        let mut rng = seed | 1;
        let width = config::SURFER_HALF_WIDTH;

        let usable = length - config::SURFER_START_CLEAR - config::SURFER_FINISH_MARGIN;
        let rock_count = ((usable / config::SURFER_ROCK_SPACING).floor() as usize)
            .clamp(4, config::SURFER_MAX_ROCKS);
        let mut rocks = Vec::with_capacity(rock_count);
        for index in 0..rock_count {
            let t = (index as f32 + 0.5) / rock_count as f32;
            // A little jitter around the even spacing, but never so much that
            // two rocks crowd into one stretch of clear water.
            let jitter = (next_unit(&mut rng) * 2.0 - 1.0) * config::SURFER_ROCK_SPACING * 0.12;
            let z = (config::SURFER_START_CLEAR + t * usable + jitter).clamp(
                config::SURFER_START_CLEAR,
                length - config::SURFER_FINISH_MARGIN,
            );
            let radius = config::SURFER_ROCK_RADIUS * (0.85 + next_unit(&mut rng) * 0.35);
            // Rocks alternate sides and stay far enough off both banks that the
            // other side of the river is always a passable line.
            let max_sway =
                width - radius - config::SURFER_BOAT_RADIUS - config::SURFER_ROCK_CLEAR_GAP;
            let side = if index % 2 == 0 { 1.0 } else { -1.0 };
            let sway = side * max_sway * (0.5 + next_unit(&mut rng) * 0.4);
            let x = centerline_at(z, seed) + sway;
            rocks.push(Rock { x, z, radius });
        }

        let mut gates = Vec::with_capacity(config::SURFER_GATES);
        for index in 0..config::SURFER_GATES {
            let t = (index as f32 + 0.5) / config::SURFER_GATES as f32;
            let z = length * (0.18 + 0.66 * t);
            let sway = (next_unit(&mut rng) * 2.0 - 1.0) * width * 0.35;
            gates.push(BoostGate {
                x: centerline_at(z, seed) + sway,
                z,
                taken: false,
            });
        }

        Self {
            x: centerline_at(0.0, seed),
            z: 0.0,
            heading: std::f32::consts::FRAC_PI_2,
            speed: config::SURFER_BASE_SPEED,
            height: config::SURFER_HOVER_HEIGHT,
            phase: SurferPhase::Riding,
            length,
            width,
            rocks,
            gates,
            input: SurferInput::default(),
            time: 0.0,
            seed,
            lines,
        }
    }

    /// The river centreline at `z`, in world `X`. Shared by the sim and the
    /// renderer so the water ribbon and the banks always agree.
    pub fn centerline(&self, z: f32) -> f32 {
        centerline_at(z, self.seed)
    }

    /// Latches a frame's input.
    pub fn set_input(&mut self, steer: f32, boost: bool) {
        self.input = SurferInput { steer, boost };
    }

    /// How far along the course the bike is, `0.0..=1.0`.
    pub fn progress(&self) -> f32 {
        (self.z / self.length).clamp(0.0, 1.0)
    }

    /// Advances one frame. The bike always has the throttle open; the player
    /// only steers and chooses when to boost.
    pub fn update(&mut self, dt: f32) -> SurferEvents {
        let mut events = SurferEvents::default();
        let input = std::mem::take(&mut self.input);
        self.input.steer = input.steer;
        self.input.boost = input.boost;

        if self.phase != SurferPhase::Riding {
            return events;
        }

        self.heading += input.steer.clamp(-1.0, 1.0) * config::SURFER_TURN_RATE * dt;
        let target = if input.boost {
            config::SURFER_BOOST_SPEED
        } else {
            config::SURFER_BASE_SPEED
        };
        let blend = 1.0 - (-config::SURFER_ACCEL * dt).exp();
        self.speed += (target - self.speed) * blend;

        let (dx, dz) = (self.heading.cos(), self.heading.sin());
        let prev_z = self.z;
        self.x += dx * self.speed * dt;
        self.z += dz * self.speed * dt;
        self.time += dt;
        self.height = config::SURFER_HOVER_HEIGHT
            + config::SURFER_WAVE_AMPLITUDE
                * (self.time * config::SURFER_WAVE_RATE + self.x * config::SURFER_WAVE_SPACE).sin();

        // The bank is the river's edge. The bike beaches the moment its body
        // reaches the edge, not when its centre crosses it.
        if (self.x - self.centerline(self.z)).abs() > self.width - config::SURFER_BOAT_RADIUS {
            self.phase = SurferPhase::Crashed;
            events.banked = true;
            return events;
        }

        // Rocks end the run on contact, measured from the visible bike body.
        for rock in &self.rocks {
            let dx = self.x - rock.x;
            let dz = self.z - rock.z;
            let reach = config::SURFER_BOAT_RADIUS + rock.radius;
            if dx * dx + dz * dz <= reach * reach {
                self.phase = SurferPhase::Crashed;
                events.hit_rock = true;
                return events;
            }
        }

        // Gates: crossing the plane gives a burst of speed, once each.
        for gate in &mut self.gates {
            if gate.taken {
                continue;
            }
            if (prev_z < gate.z && self.z >= gate.z)
                && (self.x - gate.x).abs() < config::SURFER_GATE_SPAN
            {
                gate.taken = true;
                self.speed = self.speed.max(config::SURFER_BOOST_SPEED);
                events.boosted = true;
            }
        }

        if self.z >= self.length {
            self.phase = SurferPhase::Finished;
            events.finished = true;
        }

        events
    }

    /// Restarts from the same seed and file length, as `R` does.
    pub fn restart(&mut self) {
        *self = Self::new(self.seed, self.lines);
    }
}

/// The seeded sine the river follows. Deterministic per file.
fn centerline_at(z: f32, seed: u64) -> f32 {
    let phase = (seed as f32 / u32::MAX as f32) * std::f32::consts::TAU;
    let wave = std::f32::consts::TAU / config::SURFER_RIVER_WAVELENGTH;
    config::SURFER_RIVER_AMP * (wave * z + phase).sin()
}

/// Next value in `0.0..1.0` from a plain LCG, so courses are reproducible.
fn next_unit(rng: &mut u64) -> f32 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*rng >> 40) as f32 / (1_u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::{SurferPhase, SurferSim};
    use crate::config;

    fn sim(lines: usize) -> SurferSim {
        SurferSim::new(4, lines)
    }

    #[test]
    fn the_same_seed_lays_out_the_same_river() {
        let a = sim(200);
        let b = sim(200);
        assert_eq!(a.length, b.length);
        assert_eq!(a.rocks, b.rocks);
        assert_eq!(a.gates, b.gates);
        assert!(a.length >= config::SURFER_MIN_LENGTH);
        assert!(!a.rocks.is_empty());
        assert!(!a.gates.is_empty());
    }

    #[test]
    fn a_longer_file_makes_a_longer_river() {
        assert!(sim(600).length > sim(200).length);
        assert!(sim(20_000).length <= config::SURFER_MAX_LENGTH);
    }

    #[test]
    fn the_bike_spawns_on_the_river() {
        for seed in [1_u64, 7, 42, 999] {
            let course = SurferSim::new(seed, 200);
            let bank_gap = (course.x - course.centerline(course.z)).abs();
            assert!(
                bank_gap <= course.width,
                "seed {seed} spawns the bike {bank_gap} from the centreline, past the bank"
            );
        }
    }

    #[test]
    fn every_rock_leaves_a_passable_line_past_it() {
        for seed in [1_u64, 3, 42, 999] {
            let course = SurferSim::new(seed, 200);
            for rock in &course.rocks {
                let sway = (rock.x - course.centerline(rock.z)).abs();
                let open_side = course.width - sway - rock.radius;
                assert!(
                    open_side >= config::SURFER_BOAT_RADIUS + config::SURFER_ROCK_CLEAR_GAP - 0.001,
                    "seed {seed} rock at z={} leaves only {open_side} of clear water on the open side",
                    rock.z
                );
            }
        }
    }

    #[test]
    fn rocks_keep_their_distance_down_the_river() {
        for seed in [1_u64, 3, 42, 999] {
            let mut course = SurferSim::new(seed, 200);
            course.rocks.sort_by(|a, b| a.z.total_cmp(&b.z));
            for pair in course.rocks.windows(2) {
                let gap = pair[1].z - pair[0].z;
                assert!(
                    gap >= config::SURFER_ROCK_SPACING * 0.7,
                    "seed {seed} squeezes two rocks {gap} apart, leaving no room to dodge"
                );
            }
        }
    }

    #[test]
    fn the_throttle_is_always_open() {
        let mut course = sim(200);
        let start = course.z;
        for _ in 0..30 {
            course.set_input(0.0, false);
            course.update(1.0 / 60.0);
        }
        assert!(course.z > start, "the bike should ride forward on its own");
        assert_eq!(course.phase, SurferPhase::Riding);
    }

    #[test]
    fn steering_turns_the_bike() {
        let mut course = sim(200);
        let start = course.heading;
        course.set_input(1.0, false);
        course.update(1.0 / 60.0);
        assert!(
            course.heading > start,
            "holding right should turn the nose the way the lightcycle's right turn does"
        );
        let after_right = course.heading;
        course.set_input(-1.0, false);
        course.update(1.0 / 60.0);
        assert!(
            course.heading < after_right,
            "holding left should turn it back"
        );
    }

    #[test]
    fn boost_speeds_the_bike_up() {
        let mut course = sim(200);
        course.set_input(0.0, true);
        for _ in 0..60 {
            course.update(1.0 / 60.0);
        }
        assert!(course.speed > config::SURFER_BASE_SPEED + 1.0);
    }

    #[test]
    fn riding_through_a_gate_boosts_once() {
        let mut course = sim(200);
        let gate = course.gates[0];
        course.heading = std::f32::consts::FRAC_PI_2;
        course.x = gate.x;
        course.z = gate.z - 1.0;
        course.speed = config::SURFER_BASE_SPEED;
        let mut boosted = false;
        for _ in 0..120 {
            course.set_input(0.0, false);
            if course.update(1.0 / 60.0).boosted {
                boosted = true;
            }
            if course.z > gate.z + 2.0 {
                break;
            }
        }
        assert!(boosted, "crossing the gate should boost");
        assert!(course.gates[0].taken);
    }

    #[test]
    fn running_onto_the_bank_wrecks_the_bike() {
        let mut course = sim(200);
        course.heading = 0.0; // straight at the +X bank
        course.x = course.centerline(0.0);
        course.z = 0.0;
        let mut banked = false;
        for _ in 0..600 {
            course.set_input(0.0, false);
            if course.update(1.0 / 60.0).banked {
                banked = true;
                break;
            }
        }
        assert!(banked, "the bike should beach on the bank");
        assert_eq!(course.phase, SurferPhase::Crashed);
    }

    #[test]
    fn hitting_a_rock_wrecks_the_bike() {
        let mut course = sim(200);
        let rock = course.rocks[0];
        course.heading = std::f32::consts::FRAC_PI_2;
        course.x = rock.x;
        course.z = rock.z - 1.0;
        let mut hit = false;
        for _ in 0..600 {
            course.set_input(0.0, false);
            if course.update(1.0 / 60.0).hit_rock {
                hit = true;
                break;
            }
        }
        assert!(hit, "the bike should hit the rock");
        assert_eq!(course.phase, SurferPhase::Crashed);
    }

    #[test]
    fn crossing_the_finish_ends_the_run() {
        let mut course = sim(200);
        course.z = course.length - 0.5;
        course.x = course.centerline(course.z);
        let mut finished = false;
        for _ in 0..30 {
            course.set_input(0.0, false);
            if course.update(1.0 / 60.0).finished {
                finished = true;
                break;
            }
        }
        assert!(finished, "crossing the line should finish the run");
        assert_eq!(course.phase, SurferPhase::Finished);
    }

    #[test]
    fn restarting_relays_the_same_course() {
        let mut course = sim(200);
        course.rocks.clear();
        course.restart();
        assert_eq!(course.rocks, sim(200).rocks);
        assert_eq!(course.phase, SurferPhase::Riding);
    }
}
