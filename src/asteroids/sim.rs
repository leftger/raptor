//! Bevy-free asteroid field, stepped on the shared lightcycle fixed clock.
//!
//! A second source-file game: the cycle is parked in the middle of a small ring
//! and only pivots, shooting beams at drifting rocks. This module owns the
//! rocks, the beams, the lives and the score. Like the disc fight it is a pure
//! function of its inputs, so it unit-tests without a window or an audio device.

use crate::config;
use std::f32::consts::{PI, TAU};

/// Size tier of a drifting rock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RockSize {
    Large,
    Medium,
    Small,
}

impl RockSize {
    pub fn radius(self) -> f32 {
        match self {
            Self::Large => config::ASTEROIDS_ROCK_LARGE,
            Self::Medium => config::ASTEROIDS_ROCK_MEDIUM,
            Self::Small => config::ASTEROIDS_ROCK_SMALL,
        }
    }

    /// What a hit turns this rock into. The smallest simply derezzes.
    pub fn split_into(self) -> Option<Self> {
        match self {
            Self::Large => Some(Self::Medium),
            Self::Medium => Some(Self::Small),
            Self::Small => None,
        }
    }

    pub fn score(self) -> u32 {
        match self {
            Self::Large => 20,
            Self::Medium => 50,
            Self::Small => 100,
        }
    }
}

/// One drifting rock, in world units on the ground plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Rock {
    pub x: f32,
    pub z: f32,
    pub vx: f32,
    pub vz: f32,
    pub size: RockSize,
    /// Spin around its own axis, for the renderer.
    pub angle: f32,
    pub spin: f32,
}

/// One beam in flight.
#[derive(Debug, Clone, PartialEq)]
pub struct Beam {
    pub x: f32,
    pub z: f32,
    pub vx: f32,
    pub vz: f32,
    pub life: f32,
}

/// How the field is going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsteroidsPhase {
    Flying,
    Won,
    Lost,
}

impl AsteroidsPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Flying => "FLY",
            Self::Won => "WIN",
            Self::Lost => "LOSE",
        }
    }
}

/// What happened over one [`AsteroidsSim::update`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AsteroidsEvents {
    /// Rocks derezzed this step, for one sound per pop.
    pub destroyed: u32,
    /// A rock reached the parked cycle and a life was spent.
    pub lost_life: bool,
    /// The last rock is gone: the field is won.
    pub cleared: bool,
    /// The field stopped being live this step, exactly once, for either outcome.
    /// The caller uses it to hand the cycle back to the player.
    pub ended: bool,
}

/// One run of the asteroid field.
#[derive(Debug, Clone, PartialEq)]
pub struct AsteroidsSim {
    /// Facing of the parked cycle, radians: `0` is `+X`, growing toward `+Z`.
    pub angle: f32,
    pub lives: u8,
    pub score: u32,
    pub phase: AsteroidsPhase,
    pub rocks: Vec<Rock>,
    pub beams: Vec<Beam>,
    /// Mercy window left after losing a life.
    pub invuln: f32,
    pub fire_clock: f32,
    /// Steering for the next step: `-1`, `0`, or `1`.
    pub turn: f32,
    /// Middle of the ring, in world units.
    pub center: (f32, f32),
    /// Inner radius of the ring wall, in world units.
    pub radius: f32,
    rng: u64,
}

impl AsteroidsSim {
    /// Parks a cycle at `center` and scatters the opening wave.
    pub fn new(seed: u64, center: (f32, f32), radius: f32) -> Self {
        let mut sim = Self {
            angle: 0.0,
            lives: config::ASTEROIDS_LIVES,
            score: 0,
            phase: AsteroidsPhase::Flying,
            rocks: Vec::new(),
            beams: Vec::new(),
            // Grace at the start, so the first wave cannot blindside a player
            // who has not found the pivot keys yet.
            invuln: config::ASTEROIDS_INVULN,
            fire_clock: 0.0,
            turn: 0.0,
            center,
            radius,
            rng: seed | 1,
        };
        sim.spawn_wave();
        sim
    }

    /// The opening ring of large rocks. Positions and headings come from the
    /// file's seed, so the same file always fields the same field.
    fn spawn_wave(&mut self) {
        let count = config::ASTEROIDS_WAVE_SIZE.max(1);
        for index in 0..count {
            let bearing = index as f32 / count as f32 * TAU + self.unit() * 0.6;
            let distance = self.radius * (0.45 + self.unit() * 0.27);
            let x = self.center.0 + bearing.cos() * distance;
            let z = self.center.1 + bearing.sin() * distance;
            // Aim at a point scattered around the cycle, so rocks drift through
            // the middle instead of pinballing around the rim.
            let target = (
                self.center.0 + (self.unit() - 0.5) * self.radius * 0.5,
                self.center.1 + (self.unit() - 0.5) * self.radius * 0.5,
            );
            let (dx, dz) = normalize(target.0 - x, target.1 - z);
            let speed = config::ASTEROIDS_ROCK_SPEED_MIN
                + self.unit()
                    * (config::ASTEROIDS_ROCK_SPEED_MAX - config::ASTEROIDS_ROCK_SPEED_MIN);
            let angle = self.unit() * TAU;
            let spin = (self.unit() - 0.5) * 1.6;
            self.rocks.push(Rock {
                x,
                z,
                vx: dx * speed,
                vz: dz * speed,
                size: RockSize::Large,
                angle,
                spin,
            });
        }
    }

    /// Next pseudo-random number in `0.0..1.0`. A plain LCG keeps the sim
    /// dependency-free and deterministic across platforms.
    fn unit(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 40) as f32 / (1_u32 << 24) as f32
    }

    /// Steering for the next step, clamped to a full turn either way.
    pub fn set_turn(&mut self, turn: f32) {
        self.turn = turn.clamp(-1.0, 1.0);
    }

    /// Whether the rocks are still live. Once the field is won or lost the cycle
    /// is handed back and the rocks become drifting scenery.
    pub fn is_active(&self) -> bool {
        self.phase == AsteroidsPhase::Flying
    }

    /// Fires along the current facing if the cooldown allows it.
    pub fn fire(&mut self) -> bool {
        if self.phase != AsteroidsPhase::Flying
            || self.fire_clock > 0.0
            || self.beams.len() >= config::ASTEROIDS_MAX_BEAMS
        {
            return false;
        }
        let (dx, dz) = (self.angle.cos(), self.angle.sin());
        let muzzle = config::ASTEROIDS_BIKE_RADIUS + 0.2;
        self.beams.push(Beam {
            x: self.center.0 + dx * muzzle,
            z: self.center.1 + dz * muzzle,
            vx: dx * config::ASTEROIDS_BEAM_SPEED,
            vz: dz * config::ASTEROIDS_BEAM_SPEED,
            life: config::ASTEROIDS_BEAM_LIFE,
        });
        self.fire_clock = config::ASTEROIDS_FIRE_COOLDOWN;
        true
    }

    /// Advances the field by `dt`.
    ///
    /// Once the field is decided, the rocks keep drifting as scenery but nothing
    /// collides: the player is driving again and the rocks are harmless.
    pub fn update(&mut self, dt: f32) -> AsteroidsEvents {
        let mut events = AsteroidsEvents::default();
        if dt <= 0.0 {
            return events;
        }
        let live = self.is_active();

        if live {
            self.angle = wrap_angle(self.angle + self.turn * config::ASTEROIDS_TURN_RATE * dt);
            self.invuln = (self.invuln - dt).max(0.0);
            self.fire_clock = (self.fire_clock - dt).max(0.0);
        }

        let field = self.radius + config::ASTEROIDS_BEAM_LENGTH;
        for beam in &mut self.beams {
            beam.x += beam.vx * dt;
            beam.z += beam.vz * dt;
            beam.life -= dt;
        }
        let center = self.center;
        self.beams
            .retain(|beam| beam.life > 0.0 && within(center, field, beam.x, beam.z));

        for rock in &mut self.rocks {
            rock.x += rock.vx * dt;
            rock.z += rock.vz * dt;
            rock.angle = wrap_angle(rock.angle + rock.spin * dt);
        }
        let ring = self.radius;
        for rock in &mut self.rocks {
            bounce(rock, center, ring);
        }

        if !live {
            return events;
        }

        // Beam vs rock: a beam is spent on the first rock it reaches.
        let mut rock_hit = vec![false; self.rocks.len()];
        let mut beam_spent = vec![false; self.beams.len()];
        for (beam_index, beam) in self.beams.iter().enumerate() {
            for (rock_index, rock) in self.rocks.iter().enumerate() {
                if rock_hit[rock_index] {
                    continue;
                }
                let reach = rock.size.radius() + config::ASTEROIDS_BEAM_RADIUS;
                if distance_sq(beam.x, beam.z, rock.x, rock.z) <= reach * reach {
                    rock_hit[rock_index] = true;
                    beam_spent[beam_index] = true;
                    break;
                }
            }
        }
        if rock_hit.iter().any(|hit| *hit) {
            let hit_rocks = std::mem::take(&mut self.rocks);
            let mut survivors = Vec::with_capacity(hit_rocks.len() + 4);
            for (index, rock) in hit_rocks.into_iter().enumerate() {
                if !rock_hit[index] {
                    survivors.push(rock);
                    continue;
                }
                self.score += rock.size.score();
                events.destroyed += 1;
                let Some(child) = rock.size.split_into() else {
                    continue;
                };
                // Two children peel off either side of the parent's path, a
                // little faster, so a split rock fans out instead of stacking.
                for side in [-1.0_f32, 1.0] {
                    let (vx, vz) = rotate(rock.vx, rock.vz, side * config::ASTEROIDS_SPLIT_SPREAD);
                    survivors.push(Rock {
                        x: rock.x,
                        z: rock.z,
                        vx: vx * 1.25,
                        vz: vz * 1.25,
                        size: child,
                        angle: rock.angle,
                        spin: -rock.spin,
                    });
                }
            }
            self.rocks = survivors;
            let beams = std::mem::take(&mut self.beams);
            self.beams = beams
                .into_iter()
                .zip(beam_spent)
                .filter_map(|(beam, spent)| (!spent).then_some(beam))
                .collect();
        }

        if self.invuln <= 0.0 {
            let bike = config::ASTEROIDS_BIKE_RADIUS;
            let struck = self.rocks.iter().any(|rock| {
                let reach = bike + rock.size.radius();
                distance_sq(rock.x, rock.z, center.0, center.1) <= reach * reach
            });
            if struck {
                events.lost_life = true;
                self.lives = self.lives.saturating_sub(1);
                if self.lives == 0 {
                    self.phase = AsteroidsPhase::Lost;
                    events.ended = true;
                    return events;
                }
                self.invuln = config::ASTEROIDS_INVULN;
                // Respawn shockwave: clear what would otherwise hit us again
                // before the mercy window is even over.
                let shock = config::ASTEROIDS_SHOCKWAVE;
                let rocks = std::mem::take(&mut self.rocks);
                self.rocks = rocks
                    .into_iter()
                    .filter(|rock| distance_sq(rock.x, rock.z, center.0, center.1) > shock * shock)
                    .collect();
                self.beams.clear();
            }
        }

        if self.rocks.is_empty() {
            self.phase = AsteroidsPhase::Won;
            events.cleared = true;
            events.ended = true;
        }
        events
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn normalize(x: f32, z: f32) -> (f32, f32) {
    let length = (x * x + z * z).sqrt();
    if length <= f32::EPSILON {
        (1.0, 0.0)
    } else {
        (x / length, z / length)
    }
}

fn rotate(x: f32, z: f32, angle: f32) -> (f32, f32) {
    let (sin, cos) = angle.sin_cos();
    (x * cos - z * sin, x * sin + z * cos)
}

fn distance_sq(x0: f32, z0: f32, x1: f32, z1: f32) -> f32 {
    let dx = x0 - x1;
    let dz = z0 - z1;
    dx * dx + dz * dz
}

fn within(center: (f32, f32), radius: f32, x: f32, z: f32) -> bool {
    distance_sq(x, z, center.0, center.1) <= radius * radius
}

/// Reflects a rock off the inner wall and keeps it inside the ring.
fn bounce(rock: &mut Rock, center: (f32, f32), ring: f32) {
    let dx = rock.x - center.0;
    let dz = rock.z - center.1;
    let distance = (dx * dx + dz * dz).sqrt();
    let limit = (ring - rock.size.radius()).max(0.5);
    if distance <= limit || distance <= f32::EPSILON {
        return;
    }
    let (nx, nz) = (dx / distance, dz / distance);
    let outward = rock.vx * nx + rock.vz * nz;
    if outward > 0.0 {
        rock.vx -= 2.0 * outward * nx;
        rock.vz -= 2.0 * outward * nz;
    }
    rock.x = center.0 + nx * limit;
    rock.z = center.1 + nz * limit;
}

#[cfg(test)]
mod tests {
    use super::{AsteroidsPhase, AsteroidsSim, Rock, RockSize};
    use crate::config;

    const RING: f32 = 10.0;

    fn field() -> AsteroidsSim {
        let mut sim = AsteroidsSim::new(7, (0.0, 0.0), RING);
        sim.invuln = 0.0;
        sim
    }

    fn empty() -> AsteroidsSim {
        let mut sim = field();
        sim.rocks.clear();
        sim
    }

    fn rock(x: f32, z: f32, vx: f32, vz: f32, size: RockSize) -> Rock {
        Rock {
            x,
            z,
            vx,
            vz,
            size,
            angle: 0.0,
            spin: 0.0,
        }
    }

    #[test]
    fn the_same_seed_builds_the_same_wave() {
        let first = AsteroidsSim::new(11, (0.0, 0.0), RING);
        let second = AsteroidsSim::new(11, (0.0, 0.0), RING);
        assert_eq!(first, second);
        assert_eq!(first.rocks.len(), config::ASTEROIDS_WAVE_SIZE);
        let other = AsteroidsSim::new(12, (0.0, 0.0), RING);
        assert_ne!(first.rocks, other.rocks);
    }

    #[test]
    fn every_rock_starts_inside_the_ring() {
        let sim = AsteroidsSim::new(3, (0.0, 0.0), RING);
        for rock in &sim.rocks {
            let distance = (rock.x * rock.x + rock.z * rock.z).sqrt();
            assert!(
                distance + rock.size.radius() <= RING + 0.01,
                "rock spawned outside the ring at {distance}"
            );
        }
    }

    #[test]
    fn a_beam_splits_a_large_rock() {
        let mut sim = empty();
        sim.angle = 0.0;
        sim.rocks.push(rock(8.0, 0.0, 0.0, 0.0, RockSize::Large));
        assert!(sim.fire());
        for _ in 0..40 {
            sim.update(0.05);
        }
        assert_eq!(sim.rocks.len(), 2, "a large rock should become two medium");
        assert!(sim.rocks.iter().all(|rock| rock.size == RockSize::Medium));
        assert_eq!(sim.score, RockSize::Large.score());
    }

    #[test]
    fn a_beam_has_a_cooldown() {
        let mut sim = empty();
        assert!(sim.fire());
        assert!(
            !sim.fire(),
            "a second shot inside the cooldown must not leave"
        );
    }

    #[test]
    fn rocks_bounce_back_from_the_wall() {
        let mut sim = empty();
        // Ignore the bike: this test is only about the wall.
        sim.invuln = 999.0;
        sim.rocks
            .push(rock(RING - 3.0, 0.0, 9.0, 0.0, RockSize::Large));
        for _ in 0..20 {
            sim.update(0.05);
        }
        let rock = &sim.rocks[0];
        let limit = RING - rock.size.radius();
        assert!(rock.x.abs() <= limit + 0.01, "rock escaped at x={}", rock.x);
        assert!(rock.vx < 0.0, "an outward rock should turn back");
    }

    #[test]
    fn steering_turns_the_parked_cycle() {
        // The opening wave keeps the field alive; the mercy window keeps the
        // bike out of the collision path while we only steer.
        let mut sim = field();
        sim.invuln = 999.0;
        sim.set_turn(1.0);
        sim.update(1.0);
        assert!((sim.angle - config::ASTEROIDS_TURN_RATE).abs() < 1e-4);
        sim.set_turn(-1.0);
        sim.update(1.0);
        assert!(sim.angle.abs() < 1e-4, "steering back should face +X again");
    }

    #[test]
    fn clearing_the_field_wins() {
        let mut sim = empty();
        let events = sim.update(0.1);
        assert!(events.cleared);
        assert!(events.ended, "the win reports the field ending once");
        assert_eq!(sim.phase, AsteroidsPhase::Won);
        assert!(!sim.is_active());
        assert!(!sim.update(0.1).ended, "ending is reported exactly once");
    }

    #[test]
    fn rocks_drift_as_scenery_once_the_field_is_over() {
        let mut sim = empty();
        sim.phase = AsteroidsPhase::Won;
        sim.rocks.push(rock(5.0, 0.0, 3.0, 0.0, RockSize::Large));
        let events = sim.update(0.5);
        assert!(sim.rocks[0].x > 5.0, "scenery rocks should keep drifting");
        assert!(!events.lost_life, "scenery rocks cannot cost a life");
        assert_eq!(sim.lives, config::ASTEROIDS_LIVES);
    }

    #[test]
    fn collisions_cost_lives_and_then_the_field() {
        let mut sim = empty();
        // A bystander outside the shockwave radius keeps the field from reading
        // as "cleared" the moment the hit rock is swept away.
        sim.rocks.push(rock(0.0, 9.0, 0.0, 0.0, RockSize::Small));
        sim.rocks.push(rock(0.0, 0.0, 0.0, 0.0, RockSize::Large));
        let events = sim.update(0.1);
        assert!(events.lost_life);
        assert_eq!(sim.lives, config::ASTEROIDS_LIVES - 1);
        assert_eq!(
            sim.rocks.len(),
            1,
            "the shockwave should clear the hit rock"
        );
        assert!(sim.invuln > 0.0, "losing a life grants mercy time");

        // The remaining hits drain the rest of the lives.
        for _ in 1..config::ASTEROIDS_LIVES {
            sim.invuln = 0.0;
            sim.rocks.push(rock(0.0, 0.0, 0.0, 0.0, RockSize::Large));
            sim.update(0.1);
        }
        assert_eq!(sim.lives, 0);
        assert_eq!(sim.phase, AsteroidsPhase::Lost);
    }

    #[test]
    fn splitting_is_a_bounded_tree() {
        assert_eq!(RockSize::Large.split_into(), Some(RockSize::Medium));
        assert_eq!(RockSize::Medium.split_into(), Some(RockSize::Small));
        assert_eq!(RockSize::Small.split_into(), None);
        let tiers = [RockSize::Large, RockSize::Medium, RockSize::Small];
        for pair in tiers.windows(2) {
            assert!(pair[0].radius() > pair[1].radius());
            assert!(pair[0].score() < pair[1].score());
        }
    }
}
