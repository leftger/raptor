//! Bevy-free stealth run: sneak past the patrols and out the far door.
//!
//! The room is a cell grid on the `X`/`Z` plane, so the walk and the guards'
//! sight lines are the same kind of lookup the rest of the game already does.
//! The character walks one cell per tick in whatever direction the keys are
//! holding: hold a direction and it keeps walking that way, let go and it stops
//! where it is, which is how you wait for a patrol to pass. It turns to face
//! the way it is walking. Each guard sweeps a
//! cone of vision as it walks its lane, and anything solid between the two
//! breaks the line of sight. Standing in a cone fills a detection meter; filling
//! it ends the run. Reaching the door at the far side leaves it.

use crate::config;
use crate::lightcycle::logic::Heading;
use std::collections::BTreeSet;

/// A back-and-forth lane a guard walks. `horizontal` lanes run along `X`,
/// otherwise along `Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patrol {
    pub horizontal: bool,
    /// Fixed `Z` for a horizontal lane, fixed `X` otherwise.
    pub lane: i32,
    pub min: i32,
    pub max: i32,
    /// `1` toward `max`, `-1` toward `min`.
    pub dir: i32,
}

impl Patrol {
    /// The cell the guard occupies.
    fn cell(&self, position: i32) -> (i32, i32) {
        if self.horizontal {
            (position, self.lane)
        } else {
            (self.lane, position)
        }
    }

    /// Heading the guard faces as it walks.
    pub fn heading(&self) -> Heading {
        match (self.horizontal, self.dir > 0) {
            (true, true) => Heading::PosX,
            (true, false) => Heading::NegX,
            (false, true) => Heading::PosZ,
            (false, false) => Heading::NegZ,
        }
    }

    /// Advances the lane, reversing at the ends.
    fn advance(&mut self, position: &mut i32) {
        *position += self.dir;
        if *position >= self.max {
            *position = self.max;
            self.dir = -1;
        } else if *position <= self.min {
            *position = self.min;
            self.dir = 1;
        }
    }
}

/// One patrolling enemy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guard {
    pub patrol: Patrol,
    /// Position along the lane, in cells.
    pub position: i32,
    /// Phase of the sweep, in radians.
    pub scan: f32,
}

impl Guard {
    pub fn cell(&self) -> (i32, i32) {
        self.patrol.cell(self.position)
    }

    /// Where the guard is looking, in world radians (`0` is `+X`, growing toward
    /// `+Z`), including the sweep.
    pub fn vision_angle(&self) -> f32 {
        self.patrol.heading().angle()
            + config::STEALTH_SCAN_SWEEP * (self.scan * config::STEALTH_SCAN_RATE).sin()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealthPhase {
    Sneaking,
    /// Reached the door.
    Escaped,
    /// The detection meter filled.
    Caught,
}

impl StealthPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sneaking => "SNEAKING",
            Self::Escaped => "ESCAPED",
            Self::Caught => "SPOTTED",
        }
    }
}

/// What one fixed step produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StealthEvents {
    pub stepped: bool,
    /// This is the tick a guard first laid eyes on the character.
    pub spotted: bool,
    pub escaped: bool,
    pub caught: bool,
}

/// One input frame: a tap to turn, and a hold to wait.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StealthInput {
    /// `1` walks east, `-1` west.
    pub move_x: i32,
    /// `1` walks toward the camera, `-1` away from it.
    pub move_z: i32,
}

impl StealthInput {
    /// True when no direction is held, so the character stands still.
    fn idle(&self) -> bool {
        self.move_x == 0 && self.move_z == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StealthSim {
    pub character: (i32, i32),
    pub heading: Heading,
    pub guards: Vec<Guard>,
    /// Solid cover. The room's outer walls are implicit.
    pub cover: BTreeSet<(i32, i32)>,
    pub exit: (i32, i32),
    pub phase: StealthPhase,
    /// `0.0..=1.0`; fills while a cone has the character, drains when it does not.
    pub detection: f32,
    /// Whether a guard saw the character on the previous step, for the "!" cue.
    pub seen: bool,
    pub input: StealthInput,
    /// True while the held direction is actually walkable, so the renderer can
    /// run a steady gait instead of one tied to the grid clock.
    pub walking: bool,
    /// The wall the player is pressing into, if any. Pressing into a solid is how
    /// a wall is hugged: the character turns to face it and stays put.
    pub hug: Option<Heading>,
    /// The direction along that wall a corner camera should look down: the way
    /// the player is creeping, or the side with more floor when standing still.
    pub peek: Option<Heading>,
    /// Cells walked, for the HUD.
    pub steps: usize,
    bounds: (i32, i32),
    move_clock: f32,
    seed: u64,
}

impl StealthSim {
    /// Builds a room from `seed`: cover to hide behind, lanes to slip past.
    pub fn new(seed: u64) -> Self {
        let (half_w, half_h) = (config::STEALTH_WIDTH / 2, config::STEALTH_HEIGHT / 2);
        let mut rng = seed | 1;

        let start = (-half_w + 1, -half_h + 1);
        let exit = (half_w - 1, half_h - 1);

        let mut cover = BTreeSet::new();
        let mut tries = 0;
        while cover.len() < config::STEALTH_COVER && tries < config::STEALTH_COVER * 40 {
            tries += 1;
            let x = -half_w + 2 + (next_unit(&mut rng) * (config::STEALTH_WIDTH - 4) as f32) as i32;
            let z =
                -half_h + 2 + (next_unit(&mut rng) * (config::STEALTH_HEIGHT - 4) as f32) as i32;
            let cell = (x, z);
            // Keep the doorways and the spawn clear.
            if chebyshev(cell, start) < 3 || chebyshev(cell, exit) < 3 {
                continue;
            }
            cover.insert(cell);
        }

        let mut guards = Vec::new();
        for (index, lane) in guard_lanes(half_w, half_h).into_iter().enumerate() {
            // A lane has to be walkable end to end.
            for position in lane.min..=lane.max {
                cover.remove(&lane.cell(position));
            }
            let dir = if index % 2 == 0 { 1 } else { -1 };
            let position = if dir > 0 { lane.min } else { lane.max };
            guards.push(Guard {
                patrol: Patrol { dir, ..lane },
                position,
                // Stagger the sweeps so the room does not pulse in unison.
                scan: index as f32 * 1.7,
            });
        }

        Self {
            character: start,
            heading: Heading::PosX,
            guards,
            cover,
            exit,
            phase: StealthPhase::Sneaking,
            detection: 0.0,
            seen: false,
            input: StealthInput::default(),
            walking: false,
            hug: None,
            peek: None,
            steps: 0,
            bounds: (half_w, half_h),
            move_clock: 0.0,
            seed,
        }
    }

    /// Latches a frame's input.
    pub fn set_input(&mut self, move_x: i32, move_z: i32) {
        self.input = StealthInput { move_x, move_z };
    }

    /// The direction the held keys ask for. With two axes held the one already
    /// being walked keeps it, so adding a second key mid-stride does not make
    /// the character stutter sideways.
    fn desired_heading(&self) -> Option<Heading> {
        let (x, z) = (
            self.input.move_x.clamp(-1, 1),
            self.input.move_z.clamp(-1, 1),
        );
        match (x, z) {
            (0, 0) => None,
            (x, 0) => Some(if x > 0 { Heading::PosX } else { Heading::NegX }),
            (0, z) => Some(if z > 0 { Heading::PosZ } else { Heading::NegZ }),
            _ => Some(match self.heading {
                Heading::PosX | Heading::NegX => {
                    if x > 0 {
                        Heading::PosX
                    } else {
                        Heading::NegX
                    }
                }
                Heading::PosZ | Heading::NegZ => {
                    if z > 0 {
                        Heading::PosZ
                    } else {
                        Heading::NegZ
                    }
                }
            }),
        }
    }

    /// How far the detection meter has filled, as a percentage.
    pub fn detection_percent(&self) -> u32 {
        (self.detection * 100.0).round() as u32
    }

    /// True when the cell is inside the room.
    pub fn in_bounds(&self, cell: (i32, i32)) -> bool {
        cell.0.abs() < self.bounds.0 && cell.1.abs() < self.bounds.1
    }

    /// How many cells of open floor lie in `heading` before something solid, up
    /// to `STEALTH_PEEK_RUN`. Used to pick which way along a hugged wall is worth
    /// looking down.
    fn open_run(&self, heading: Heading) -> i32 {
        let mut cell = self.character;
        let mut run = 0;
        while run < config::STEALTH_PEEK_RUN {
            cell = step_cell(cell, heading);
            if self.is_solid(cell) {
                break;
            }
            run += 1;
        }
        run
    }

    /// How far a guard can see along each ray across its cone, in cells, stopping
    /// at whatever it runs into. The drawn cone is cut to this, so it shows what
    /// the guard can actually see instead of shining through cover.
    ///
    /// The order matches the cone mesh's vertices, whose local angle runs the
    /// other way round from the world angle: local `b` faces world `centre - b`.
    pub fn vision_radii(&self, index: usize, segments: usize) -> Vec<f32> {
        let segments = segments.max(1);
        let Some(guard) = self.guards.get(index) else {
            return vec![0.0; segments + 1];
        };
        let centre = guard.vision_angle();
        let half = config::STEALTH_VISION_HALF_ANGLE;
        let (gx, gz) = guard.cell();
        let mut radii = Vec::with_capacity(segments + 1);
        for step in 0..=segments {
            let t = step as f32 / segments as f32;
            let world = centre - (-half + t * half * 2.0);
            let (dx, dz) = (world.cos(), world.sin());
            let mut reach = 0.0;
            while reach < config::STEALTH_VISION_RANGE {
                reach += config::STEALTH_SIGHT_SAMPLE;
                let cell = (
                    gx + (dx * reach).round() as i32,
                    gz + (dz * reach).round() as i32,
                );
                if self.is_solid(cell) {
                    // Stop short, so the rim sits against the obstacle rather
                    // than inside it.
                    reach -= config::STEALTH_SIGHT_SAMPLE;
                    break;
                }
            }
            radii.push(reach.clamp(0.0, config::STEALTH_VISION_RANGE));
        }
        radii
    }

    /// True when nothing can walk into the cell.
    pub fn is_solid(&self, cell: (i32, i32)) -> bool {
        !self.in_bounds(cell) || self.cover.contains(&cell)
    }

    /// True when `from` can see `to`: inside the cone, in range, and with no
    /// cover in the way.
    pub fn guard_sees(&self, guard: &Guard, to: (i32, i32)) -> bool {
        let (gx, gz) = guard.cell();
        let (dx, dz) = ((to.0 - gx) as f32, (to.1 - gz) as f32);
        let distance = (dx * dx + dz * dz).sqrt();
        if distance < 0.001 {
            return true;
        }
        if distance > config::STEALTH_VISION_RANGE {
            return false;
        }
        let angle = dz.atan2(dx);
        if angle_delta(angle, guard.vision_angle()).abs() > config::STEALTH_VISION_HALF_ANGLE {
            return false;
        }
        self.line_of_sight(guard.cell(), to)
    }

    /// Samples the segment between two cells for anything solid.
    fn line_of_sight(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        let (dx, dz) = ((to.0 - from.0) as f32, (to.1 - from.1) as f32);
        let distance = (dx * dx + dz * dz).sqrt();
        let steps = (distance / config::STEALTH_SIGHT_SAMPLE).ceil() as i32;
        for step in 1..steps {
            let t = step as f32 / steps as f32;
            let cell = (
                from.0 + (dx * t).round() as i32,
                from.1 + (dz * t).round() as i32,
            );
            if self.is_solid(cell) {
                return false;
            }
        }
        true
    }

    /// True when any guard has the character in view.
    pub fn spotted(&self) -> bool {
        self.guards
            .iter()
            .any(|guard| self.guard_sees(guard, self.character))
    }

    /// Advances one fixed step. Movement is quantised: the character and the
    /// guards each creep a cell every `STEALTH_STEP_SECONDS`.
    pub fn update(&mut self, dt: f32) -> StealthEvents {
        let mut events = StealthEvents::default();
        self.walking = false;
        // The hug from the previous frame, which the creep test below needs.
        let hugged = self.hug;
        self.hug = None;
        self.peek = None;
        if self.phase != StealthPhase::Sneaking {
            return events;
        }

        // Facing is immediate, so the character turns on the spot the moment a
        // key goes down, even if the next step is still due or blocked.
        let wanted = self.desired_heading();
        if let Some(heading) = wanted {
            self.heading = heading;
        }
        self.walking =
            wanted.is_some_and(|heading| !self.is_solid(step_cell(self.character, heading)));

        // Wall hugging, in two parts.
        //
        // Pressing into a solid turns the figure's back to it. That hug then holds
        // while the player keeps a direction into the *same* wall or along it, so
        // the figure creeps along cover with its back to it rather than letting go
        // the moment the key changes. Walking away, letting go, or reaching the
        // end of the wall all release it.
        //
        // Only the facing and the reported wall change here. `tick` moves by the
        // held direction, so the figure never steps into the wall, and creeping is
        // the ordinary movement rules applied along the wall's face.
        let hugging = wanted
            .filter(|heading| self.is_solid(step_cell(self.character, *heading)))
            .or_else(|| {
                hugged.filter(|wall| {
                    self.is_solid(step_cell(self.character, *wall))
                        && wanted.is_some_and(|heading| along_wall(*wall).contains(&heading))
                })
            });
        if let Some(wall) = hugging {
            self.hug = Some(wall);
            self.heading = wall.opposite();
            // Which way a corner camera should look along the wall. While the
            // player is creeping along it, the camera leads toward the corner he
            // is moving toward; standing still falls back to the side with more
            // open floor, so there is always something to look past.
            let along = along_wall(wall);
            self.peek = wanted
                .filter(|heading| along.contains(heading) && self.open_run(*heading) > 0)
                .or_else(|| {
                    along
                        .into_iter()
                        .map(|across| (self.open_run(across), across))
                        .max_by_key(|(run, _)| *run)
                        .filter(|(run, _)| *run > 0)
                        .map(|(_, across)| across)
                });
        }

        // Sweep the cones, and watch, every frame.
        for guard in &mut self.guards {
            guard.scan += dt;
        }
        let seen = self.spotted();
        if seen {
            if !self.seen {
                events.spotted = true;
            }
            // Being close fills the meter faster.
            let nearest = self
                .guards
                .iter()
                .filter(|guard| self.guard_sees(guard, self.character))
                .map(|guard| chebyshev(guard.cell(), self.character) as f32)
                .fold(f32::MAX, f32::min);
            let closeness = (1.0 - nearest / config::STEALTH_VISION_RANGE).clamp(0.0, 1.0);
            let rate = config::STEALTH_DETECT_RATE * (1.0 + closeness);
            self.detection = (self.detection + rate * dt).min(1.0);
            if self.detection >= 1.0 {
                self.phase = StealthPhase::Caught;
                events.caught = true;
            }
        } else {
            self.detection = (self.detection - config::STEALTH_DECAY * dt).max(0.0);
        }
        self.seen = seen;

        self.move_clock += dt;
        while self.move_clock >= config::STEALTH_STEP_SECONDS {
            self.move_clock -= config::STEALTH_STEP_SECONDS;
            if self.phase != StealthPhase::Sneaking {
                break;
            }
            self.tick(&mut events);
        }
        events
    }

    /// One quantised step of everyone on the floor.
    fn tick(&mut self, events: &mut StealthEvents) {
        // Walking is simply the held direction: no keys, no movement, so the
        // character waits for a patrol by standing still.
        if !self.input.idle()
            && let Some(heading) = self.desired_heading()
        {
            let ahead = step_cell(self.character, heading);
            if !self.is_solid(ahead) {
                self.character = ahead;
                self.steps += 1;
                events.stepped = true;
            }
        }

        for index in 0..self.guards.len() {
            let mut guard = self.guards[index];
            guard.patrol.advance(&mut guard.position);
            self.guards[index] = guard;
        }

        if self.character == self.exit {
            self.phase = StealthPhase::Escaped;
            events.escaped = true;
        }
    }

    /// Restarts from the same seed, as `R` does.
    pub fn restart(&mut self) {
        *self = Self::new(self.seed);
    }
}

/// The lanes the guards walk: two across, one down, all inside the room.
fn guard_lanes(half_w: i32, half_h: i32) -> Vec<Patrol> {
    let across = half_w - 2;
    vec![
        Patrol {
            horizontal: true,
            lane: -half_h / 2,
            min: -across,
            max: across,
            dir: 1,
        },
        Patrol {
            horizontal: true,
            lane: half_h / 2,
            min: -across,
            max: across,
            dir: -1,
        },
        Patrol {
            horizontal: false,
            lane: 0,
            min: -half_h + 2,
            max: half_h - 2,
            dir: 1,
        },
    ]
}

/// The cell one step from `cell` along `heading`.
pub fn step_cell(cell: (i32, i32), heading: Heading) -> (i32, i32) {
    match heading {
        Heading::PosX => (cell.0 + 1, cell.1),
        Heading::NegX => (cell.0 - 1, cell.1),
        Heading::PosZ => (cell.0, cell.1 + 1),
        Heading::NegZ => (cell.0, cell.1 - 1),
    }
}

/// Smallest angle between two directions, in `-PI..=PI`.
fn angle_delta(a: f32, b: f32) -> f32 {
    let mut delta = (a - b) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Next value in `0.0..1.0` from a plain LCG, so rooms are reproducible.
fn next_unit(rng: &mut u64) -> f32 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*rng >> 40) as f32 / (1_u32 << 24) as f32
}

/// The two directions along a wall: the ones perpendicular to `heading`.
fn along_wall(heading: Heading) -> [Heading; 2] {
    match heading {
        Heading::PosX | Heading::NegX => [Heading::PosZ, Heading::NegZ],
        Heading::PosZ | Heading::NegZ => [Heading::PosX, Heading::NegX],
    }
}

#[cfg(test)]
mod tests {
    use super::{Guard, Patrol, StealthPhase, StealthSim};
    use crate::config;
    use crate::lightcycle::logic::Heading;

    fn sim(seed: u64) -> StealthSim {
        StealthSim::new(seed)
    }

    /// A guard on a long eastward lane at `position`, sweeping from zero phase.
    fn guard_at(position: i32) -> Guard {
        Guard {
            patrol: Patrol {
                horizontal: true,
                lane: 0,
                min: -8,
                max: 8,
                dir: 1,
            },
            position,
            scan: 0.0,
        }
    }

    #[test]
    fn the_same_seed_builds_the_same_room() {
        assert_eq!(sim(4).cover, sim(4).cover);
        assert_eq!(sim(4).guards, sim(4).guards);
        assert!(
            !sim(4).cover.is_empty(),
            "there should be cover to hide behind"
        );
    }

    #[test]
    fn the_spawn_and_the_door_stay_clear() {
        let room = sim(2);
        assert!(!room.is_solid(room.character));
        assert!(!room.is_solid(room.exit));
        assert_ne!(room.character, room.exit);
    }

    #[test]
    fn every_guard_lane_is_walkable_end_to_end() {
        let room = sim(9);
        for guard in &room.guards {
            for position in guard.patrol.min..=guard.patrol.max {
                let cell = guard.patrol.cell(position);
                assert!(
                    !room.cover.contains(&cell),
                    "cover blocks a lane at {cell:?}"
                );
                assert!(room.in_bounds(cell), "lane leaves the room at {cell:?}");
            }
        }
    }

    #[test]
    fn a_guard_sees_down_its_cone_and_not_behind_it() {
        let mut room = sim(1);
        room.cover.clear();
        room.guards = vec![guard_at(0)];
        let guard = room.guards[0];
        // Straight ahead (east) is seen; directly behind is not.
        assert!(room.guard_sees(&guard, (3, 0)));
        assert!(!room.guard_sees(&guard, (-3, 0)));
        // Out of range is never seen.
        let far = (config::STEALTH_VISION_RANGE as i32 + 4, 0);
        assert!(!room.guard_sees(&guard, far));
    }

    #[test]
    fn cover_breaks_the_line_of_sight() {
        let mut room = sim(1);
        room.cover.clear();
        let guard = guard_at(0);
        assert!(room.guard_sees(&guard, (4, 0)));
        room.cover.insert((2, 0));
        assert!(
            !room.guard_sees(&guard, (4, 0)),
            "a block between them should hide the character"
        );
    }

    #[test]
    fn standing_in_a_cone_fills_the_meter() {
        let mut room = sim(1);
        room.cover.clear();
        room.guards = vec![guard_at(-5)];
        room.character = (0, 0);
        let before = room.detection;
        let events = room.update(1.0 / 60.0);
        assert!(events.spotted, "the guard should notice");
        assert!(room.detection > before, "the meter should start filling");
        assert!(room.detection_percent() > 0);
    }

    #[test]
    fn a_full_meter_ends_the_run() {
        let mut room = sim(1);
        room.cover.clear();
        room.guards = vec![guard_at(-5)];
        room.character = (0, 0);
        room.detection = 0.99;
        let mut caught = false;
        for _ in 0..30 {
            if room.update(1.0 / 60.0).caught {
                caught = true;
                break;
            }
        }
        assert!(caught, "the meter should tip over");
        assert_eq!(room.phase, StealthPhase::Caught);
    }

    #[test]
    fn hiding_drains_the_meter() {
        let mut room = sim(1);
        // Nobody left to see the character.
        room.guards.clear();
        room.detection = 0.8;
        // Half a second at the decay rate should take the edge off, not empty it.
        for _ in 0..30 {
            room.update(1.0 / 60.0);
        }
        assert!(room.detection < 0.8, "the meter should drain when unseen");
        assert!(room.detection > 0.0, "half a second should not empty it");
        assert!(
            (50..65).contains(&room.detection_percent()),
            "expected a partial drain, got {}%",
            room.detection_percent()
        );
    }

    #[test]
    fn the_keys_set_the_way_the_character_faces() {
        let mut room = sim(1);
        for (keys, expected) in [
            ((1, 0), Heading::PosX),
            ((0, 1), Heading::PosZ),
            ((-1, 0), Heading::NegX),
            ((0, -1), Heading::NegZ),
        ] {
            room.set_input(keys.0, keys.1);
            room.update(config::STEALTH_STEP_SECONDS);
            assert_eq!(room.heading, expected, "input {keys:?}");
        }
    }

    #[test]
    fn holding_a_key_walks_a_cell_per_step() {
        let mut room = sim(2);
        room.guards.clear();
        room.cover.clear();
        let (x, z) = room.character;
        room.set_input(1, 0);
        let events = room.update(config::STEALTH_STEP_SECONDS);
        assert!(events.stepped, "the step should have landed");
        assert!(room.walking, "and it should read as walking");
        assert_eq!(room.character, (x + 1, z), "one cell east");
        assert_eq!(room.heading, Heading::PosX);
    }

    #[test]
    fn letting_go_of_the_keys_holds_position() {
        let mut room = sim(1);
        room.guards.clear();
        let before = room.character;
        for _ in 0..30 {
            room.set_input(0, 0);
            room.update(config::STEALTH_STEP_SECONDS);
        }
        assert_eq!(room.character, before, "standing still should not drift");
        assert!(!room.walking);
    }

    #[test]
    fn cover_stops_the_walk_and_turns_the_back_to_it() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        let (x, z) = room.character;
        room.cover.insert((x + 1, z));
        room.set_input(1, 0);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.character, (x, z), "cover should block the step");
        assert!(!room.walking, "and it is not walking into it either");
        assert_eq!(
            room.heading,
            Heading::NegX,
            "hugging turns the back to the wall rather than the face to it"
        );
        assert_eq!(room.hug, Some(Heading::PosX), "and the wall is reported");
    }

    #[test]
    fn a_second_key_does_not_flip_the_walk_mid_stride() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        room.set_input(1, 0);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.heading, Heading::PosX);
        // Adding south while still holding east keeps walking east.
        room.set_input(1, 1);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.heading, Heading::PosX);
        // Letting go of east hands over to the axis still held.
        room.set_input(0, 1);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.heading, Heading::PosZ);
    }

    #[test]
    fn walking_out_of_the_room_is_impossible() {
        let mut room = sim(1);
        room.guards.clear();
        // Push at the west wall for a long time.
        for _ in 0..200 {
            room.set_input(-1, 0);
            room.update(config::STEALTH_STEP_SECONDS);
        }
        assert!(
            room.in_bounds(room.character),
            "the character left the room at {:?}",
            room.character
        );
        assert_eq!(room.character.0, -room.bounds.0 + 1);
    }

    #[test]
    fn pressing_into_something_reports_the_wall_and_an_open_way_along_it() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        let (x, z) = room.character;
        room.cover.insert((x + 1, z));
        room.set_input(1, 0);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(
            room.hug,
            Some(Heading::PosX),
            "the wall it is pressed against"
        );
        assert!(
            matches!(room.peek, Some(Heading::PosZ | Heading::NegZ)),
            "the open way along the wall, got {:?}",
            room.peek
        );
    }

    #[test]
    fn walking_in_the_open_is_not_a_hug() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        room.set_input(1, 0);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.hug, None);
        assert_eq!(room.peek, None);
    }

    #[test]
    fn reaching_the_door_ends_the_run() {
        let mut room = sim(1);
        room.guards.clear();
        let exit = room.exit;
        room.character = (exit.0 - 1, exit.1);
        let mut escaped = false;
        for _ in 0..60 {
            room.set_input(1, 0);
            if room.update(config::STEALTH_STEP_SECONDS).escaped {
                escaped = true;
                break;
            }
        }
        assert!(escaped, "walking into the door should leave the room");
        assert_eq!(room.phase, StealthPhase::Escaped);
    }

    #[test]
    fn the_camera_peeks_down_the_side_with_more_floor() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        room.character = (0, 0);
        room.cover.insert((1, 0)); // the wall being hugged
        room.cover.insert((0, 1)); // so +Z runs into cover at once
        room.set_input(1, 0);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.hug, Some(Heading::PosX));
        assert_eq!(
            room.peek,
            Some(Heading::NegZ),
            "-Z has the floor, and it is the second direction tried"
        );

        // Swap which side is shut off: the choice has to follow the floor rather
        // than settle on a fixed preference.
        room.cover.clear();
        room.cover.insert((1, 0));
        room.cover.insert((0, -1));
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.peek, Some(Heading::PosZ));
    }

    #[test]
    fn the_peek_leads_toward_the_way_the_player_is_creeping() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        room.character = (0, 0);
        room.cover.insert((1, 0)); // the wall being hugged
        // +Z has only one open cell before cover; -Z is wide open. Standing
        // still the camera would pick -Z, but creeping toward +Z has to lead
        // the camera that way, toward the corner the player is approaching.
        room.cover.insert((0, 2));
        // Press into the wall first: that is what starts the hug. A frame time
        // short of one step keeps the creep below from walking the character
        // before the peek is read.
        room.set_input(1, 0);
        room.update(1.0 / 60.0);
        assert_eq!(room.hug, Some(Heading::PosX));
        // Then creep along it toward +Z while still holding the hug.
        room.set_input(0, 1);
        room.update(1.0 / 60.0);
        assert_eq!(room.hug, Some(Heading::PosX));
        assert_eq!(
            room.peek,
            Some(Heading::PosZ),
            "the held direction leads, not the longer floor"
        );

        // Let go of the key and the same room falls back to the longer floor.
        room.set_input(1, 0);
        room.update(1.0 / 60.0);
        assert_eq!(room.peek, Some(Heading::NegZ));
    }

    #[test]
    fn hugging_a_corner_with_no_way_along_it_peeks_nowhere() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        room.character = (0, 0);
        // A pocket: the wall being pushed against, with cover either side.
        room.cover.insert((1, 0));
        room.cover.insert((0, 1));
        room.cover.insert((0, -1));
        room.set_input(1, 0);
        room.update(config::STEALTH_STEP_SECONDS);
        assert_eq!(room.hug, Some(Heading::PosX));
        assert_eq!(room.peek, None, "nothing to look past");
    }

    #[test]
    fn the_cone_is_cut_where_cover_blocks_it() {
        let mut room = sim(1);
        room.guards.clear();
        room.cover.clear();
        // On the lane through the origin, facing +X, sweeping from zero phase.
        room.guards.push(guard_at(0));
        let segments = config::STEALTH_CONE_SEGMENTS;

        let open = room.vision_radii(0, segments);
        assert_eq!(
            open.len(),
            segments + 1,
            "one ray per rim vertex, plus the origin"
        );
        assert!(
            open.iter()
                .all(|reach| *reach > config::STEALTH_VISION_RANGE - 0.5),
            "with nothing in the way every ray should carry to the range, got {open:?}"
        );

        room.cover.insert((3, 0)); // straight ahead of the guard
        let blocked = room.vision_radii(0, segments);
        let middle = blocked[segments / 2];
        assert!(
            middle < config::STEALTH_VISION_RANGE,
            "the blocked ray should stop short, got {middle}"
        );
        assert!(
            middle < 3.0,
            "and it should stop short of the cover itself, got {middle}"
        );
        assert!(
            blocked[0] > middle,
            "rays that miss the cover should carry on: {} vs {middle}",
            blocked[0]
        );
    }

    #[test]
    fn a_cone_with_no_guard_behind_it_collapses() {
        let room = sim(1);
        assert_eq!(
            room.vision_radii(usize::MAX, 4),
            vec![0.0; 5],
            "a missing guard should draw nothing rather than panic"
        );
    }
}
