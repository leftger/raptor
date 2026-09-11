//! Bevy-free Galaga field: the cycle slides along the bottom and shoots upward
//! at a formation of bugs.
//!
//! The field lives on the `X`/`Z` plane like the asteroid ring: the cycle is
//! parked on the bottom edge, facing `+Z`, and the bugs hold a swaying grid
//! formation above it. Bugs peel off one at a time and dive at the cycle; a
//! diver that misses loops back into the formation. Clear the formation and the
//! run is won; lose all three lives, or let the formation march down onto the
//! cycle, and it is lost.

use crate::config;

/// One bug in the formation. The grid slot is its `row`/`col`; the renderer
/// pools one entity per bug and this sim drives whether it is visible.
#[derive(Debug, Clone, PartialEq)]
pub struct Bug {
    pub x: f32,
    pub z: f32,
    pub row: usize,
    pub col: usize,
    pub alive: bool,
    pub diving: bool,
    /// X the diver is aiming at, picked when it peels off.
    dive_target_x: f32,
}

/// One beam in flight, moving straight up the field.
#[derive(Debug, Clone, PartialEq)]
pub struct Beam {
    pub x: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalagaPhase {
    Fighting,
    /// The formation is gone.
    Won,
    /// Out of lives, or the formation reached the cycle.
    Lost,
}

impl GalagaPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fighting => "FIGHT",
            Self::Won => "WIN",
            Self::Lost => "LOSE",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GalagaEvents {
    pub fired: bool,
    pub killed: u32,
    pub lost_life: bool,
    /// The formation marched down onto the cycle, ending the run.
    pub overrun: bool,
    pub cleared: bool,
    /// The field stopped being live this frame, exactly once, either way.
    pub ended: bool,
}

/// One input frame: a held lateral axis and an edge-triggered shot.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GalagaInput {
    pub slide: f32,
    pub fire: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GalagaSim {
    pub player_x: f32,
    pub lives: u8,
    pub score: u32,
    pub phase: GalagaPhase,
    pub bugs: Vec<Bug>,
    pub beams: Vec<Beam>,
    /// Mercy window left after losing a life.
    pub invuln: f32,
    pub fire_clock: f32,
    pub input: GalagaInput,
    /// Horizontal offset the whole formation sways with.
    formation_x: f32,
    formation_dir: f32,
    /// Z of the top row; steps downward over time.
    formation_z: f32,
    step_clock: f32,
    step_seconds: f32,
    dive_clock: f32,
    rng: u64,
    seed: u64,
    lines: usize,
}

impl GalagaSim {
    /// Builds the opening formation from `seed`; longer files march a little
    /// quicker, so the run reads differently per file.
    pub fn new(seed: u64, lines: usize) -> Self {
        let mut sim = Self {
            player_x: 0.0,
            lives: config::GALAGA_LIVES,
            score: 0,
            phase: GalagaPhase::Fighting,
            bugs: Vec::new(),
            beams: Vec::new(),
            // Grace at the start, so the opening dive cannot blindside a player
            // who has not found the slide keys yet.
            invuln: config::GALAGA_INVULN,
            fire_clock: 0.0,
            input: GalagaInput::default(),
            formation_x: 0.0,
            formation_dir: 1.0,
            formation_z: config::GALAGA_FORMATION_TOP,
            step_clock: config::GALAGA_FORMATION_STEP_SECONDS,
            step_seconds: config::GALAGA_FORMATION_STEP_SECONDS
                * (0.8 + lines as f32 * 0.0006).clamp(0.8, 1.3),
            dive_clock: config::GALAGA_DIVE_COOLDOWN,
            rng: seed | 1,
            seed,
            lines,
        };
        sim.spawn_formation();
        sim
    }

    /// Grid slot of a bug, in world units, from the formation's current origin.
    fn formation_slot(formation_x: f32, formation_z: f32, row: usize, col: usize) -> (f32, f32) {
        let x = (col as f32 - (config::GALAGA_COLS - 1) as f32 * 0.5) * config::GALAGA_CELL_X
            + formation_x;
        let z = formation_z - row as f32 * config::GALAGA_CELL_Z;
        (x, z)
    }

    fn spawn_formation(&mut self) {
        for row in 0..config::GALAGA_ROWS {
            for col in 0..config::GALAGA_COLS {
                let (x, z) = Self::formation_slot(self.formation_x, self.formation_z, row, col);
                self.bugs.push(Bug {
                    x,
                    z,
                    row,
                    col,
                    alive: true,
                    diving: false,
                    dive_target_x: 0.0,
                });
            }
        }
    }

    /// Score for a bug, higher rows worth more.
    pub fn bug_score(row: usize) -> u32 {
        (config::GALAGA_ROWS - row) as u32 * 40
    }

    /// Latches a frame's input. `slide` is held; `fire` is an edge.
    pub fn set_input(&mut self, slide: f32, fire: bool) {
        self.input = GalagaInput { slide, fire };
    }

    /// Advances one frame.
    pub fn update(&mut self, dt: f32) -> GalagaEvents {
        let mut events = GalagaEvents::default();
        let input = std::mem::take(&mut self.input);
        self.input.slide = input.slide;

        if self.phase != GalagaPhase::Fighting {
            return events;
        }

        // The cycle slides along the bottom and fires upward.
        let half_x = config::GALAGA_HALF_X - config::GALAGA_PLAYER_RADIUS;
        self.player_x = (self.player_x
            + input.slide.clamp(-1.0, 1.0) * config::GALAGA_PLAYER_SPEED * dt)
            .clamp(-half_x, half_x);
        self.fire_clock = (self.fire_clock - dt).max(0.0);
        self.invuln = (self.invuln - dt).max(0.0);

        if input.fire && self.fire_clock <= 0.0 && self.beams.len() < config::GALAGA_MAX_BEAMS {
            self.beams.push(Beam {
                x: self.player_x,
                z: config::GALAGA_PLAYER_Z + config::GALAGA_BEAM_MUZZLE,
            });
            self.fire_clock = config::GALAGA_FIRE_COOLDOWN;
            events.fired = true;
        }

        // The formation sways side to side and steps down.
        self.formation_x += self.formation_dir * config::GALAGA_FORMATION_SPEED * dt;
        if self.formation_x.abs() > config::GALAGA_FORMATION_SWAY {
            self.formation_x = self.formation_x.clamp(
                -config::GALAGA_FORMATION_SWAY,
                config::GALAGA_FORMATION_SWAY,
            );
            self.formation_dir = -self.formation_dir;
        }
        self.step_clock -= dt;
        if self.step_clock <= 0.0 {
            self.step_clock = self.step_seconds;
            self.formation_z -= config::GALAGA_FORMATION_STEP;
        }

        // Beams fly up and die at the top of the field.
        for beam in &mut self.beams {
            beam.z += config::GALAGA_BEAM_SPEED * dt;
        }
        self.beams
            .retain(|beam| beam.z < config::GALAGA_HALF_Z + 1.0);

        // Send one bug diving whenever the dive clock runs out.
        self.dive_clock -= dt;
        if self.dive_clock <= 0.0 {
            self.dive_clock = config::GALAGA_DIVE_COOLDOWN;
            if let Some(index) = self.pick_diver() {
                self.bugs[index].diving = true;
                self.bugs[index].dive_target_x = self.player_x + (self.unit() * 2.0 - 1.0) * 3.0;
            }
        }

        // Move bugs. Divers chase their target and wrap back into formation
        // past the bottom; the rest hold their grid slot.
        for index in 0..self.bugs.len() {
            if !self.bugs[index].alive {
                continue;
            }
            let (formation_x, formation_z) = (self.formation_x, self.formation_z);
            if self.bugs[index].diving {
                let bug = &mut self.bugs[index];
                bug.z -= config::GALAGA_DIVE_SPEED * dt;
                let steer = (bug.dive_target_x - bug.x).clamp(
                    -config::GALAGA_DIVE_STEER * dt,
                    config::GALAGA_DIVE_STEER * dt,
                );
                bug.x += steer;
                if bug.z < config::GALAGA_PLAYER_Z - 2.0 {
                    bug.diving = false;
                    let (x, z) = Self::formation_slot(formation_x, formation_z, bug.row, bug.col);
                    bug.x = x;
                    bug.z = z;
                }
            } else {
                let (x, z) = Self::formation_slot(
                    formation_x,
                    formation_z,
                    self.bugs[index].row,
                    self.bugs[index].col,
                );
                self.bugs[index].x = x;
                self.bugs[index].z = z;
            }
        }

        // Beam vs bug: a beam is spent on the first bug it reaches.
        let reach = config::GALAGA_BEAM_RADIUS + config::GALAGA_BUG_RADIUS;
        let beams = std::mem::take(&mut self.beams);
        for beam in beams {
            let hit = self.bugs.iter().position(|bug| {
                bug.alive && distance_sq(beam.x, beam.z, bug.x, bug.z) <= reach * reach
            });
            match hit {
                Some(index) => {
                    self.bugs[index].alive = false;
                    self.bugs[index].diving = false;
                    self.score += Self::bug_score(self.bugs[index].row);
                    events.killed += 1;
                }
                None => self.beams.push(beam),
            }
        }

        // A bug reaching the cycle costs a life, and the bug with it.
        if self.invuln <= 0.0 {
            let reach = config::GALAGA_PLAYER_RADIUS + config::GALAGA_BUG_RADIUS;
            let hit = self.bugs.iter().position(|bug| {
                bug.alive
                    && distance_sq(bug.x, bug.z, self.player_x, config::GALAGA_PLAYER_Z)
                        <= reach * reach
            });
            if let Some(index) = hit {
                self.bugs[index].alive = false;
                self.bugs[index].diving = false;
                events.lost_life = true;
                self.lives = self.lives.saturating_sub(1);
                if self.lives == 0 {
                    self.phase = GalagaPhase::Lost;
                    events.ended = true;
                    return events;
                }
                self.invuln = config::GALAGA_INVULN;
            }
        }

        // The whole formation marching down onto the cycle ends the run.
        let lowest = self.formation_z - (config::GALAGA_ROWS - 1) as f32 * config::GALAGA_CELL_Z;
        if lowest <= config::GALAGA_PLAYER_Z + 1.5 {
            self.phase = GalagaPhase::Lost;
            events.overrun = true;
            events.ended = true;
            return events;
        }

        if self.bugs.iter().all(|bug| !bug.alive) {
            self.phase = GalagaPhase::Won;
            events.cleared = true;
            events.ended = true;
        }

        events
    }

    /// A random live bug that is still holding formation, if any.
    fn pick_diver(&mut self) -> Option<usize> {
        let candidates: Vec<usize> = self
            .bugs
            .iter()
            .enumerate()
            .filter(|(_, bug)| bug.alive && !bug.diving)
            .map(|(index, _)| index)
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let pick = (self.unit() * candidates.len() as f32) as usize;
        candidates.get(pick.min(candidates.len() - 1)).copied()
    }

    /// Next pseudo-random number in `0.0..1.0`.
    fn unit(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 40) as f32 / (1_u32 << 24) as f32
    }

    /// Restarts from the same seed and file length, as `R` does.
    pub fn restart(&mut self) {
        *self = Self::new(self.seed, self.lines);
    }
}

fn distance_sq(x0: f32, z0: f32, x1: f32, z1: f32) -> f32 {
    let dx = x0 - x1;
    let dz = z0 - z1;
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use super::{GalagaPhase, GalagaSim};
    use crate::config;

    fn sim(lines: usize) -> GalagaSim {
        let mut sim = GalagaSim::new(7, lines);
        sim.invuln = 0.0;
        sim
    }

    #[test]
    fn the_same_seed_builds_the_same_formation() {
        let first = GalagaSim::new(11, 200);
        let second = GalagaSim::new(11, 200);
        assert_eq!(first, second);
        assert_eq!(first.bugs.len(), config::GALAGA_ROWS * config::GALAGA_COLS);
        // The opening grid is identical across seeds; the seeds diverge once
        // dives start peeling off.
        let mut a = GalagaSim::new(11, 200);
        let mut b = GalagaSim::new(12, 200);
        for _ in 0..180 {
            a.set_input(0.0, false);
            b.set_input(0.0, false);
            a.update(1.0 / 60.0);
            b.update(1.0 / 60.0);
        }
        assert_ne!(
            a.bugs, b.bugs,
            "different seeds should pick different divers"
        );
    }

    #[test]
    fn every_bug_starts_inside_the_field() {
        let field = GalagaSim::new(3, 200);
        for bug in &field.bugs {
            assert!(bug.x.abs() <= config::GALAGA_HALF_X + 0.01);
            assert!(bug.z <= config::GALAGA_HALF_Z + 0.01);
        }
    }

    #[test]
    fn the_cycle_slides_and_stays_inside_the_field() {
        let mut field = sim(200);
        field.set_input(1.0, false);
        for _ in 0..600 {
            field.update(1.0 / 60.0);
        }
        let limit = config::GALAGA_HALF_X - config::GALAGA_PLAYER_RADIUS;
        assert!(
            field.player_x <= limit + 0.01,
            "the cycle slid off the right"
        );
        field.set_input(-1.0, false);
        for _ in 0..1200 {
            field.update(1.0 / 60.0);
        }
        assert!(
            field.player_x >= -limit - 0.01,
            "the cycle slid off the left"
        );
    }

    #[test]
    fn firing_has_a_cooldown_and_beams_fly_up() {
        let mut field = sim(200);
        field.set_input(0.0, true);
        let events = field.update(1.0 / 60.0);
        assert!(events.fired);
        assert_eq!(field.beams.len(), 1);
        let start = field.beams[0].z;
        field.set_input(0.0, true);
        let events = field.update(1.0 / 60.0);
        assert!(
            !events.fired,
            "a second shot inside the cooldown must not leave"
        );
        assert!(field.beams[0].z > start, "the beam should fly upward");
    }

    #[test]
    fn a_beam_kills_the_bug_it_reaches() {
        let mut field = sim(200);
        // Kill everything else so the beam's only possible victim is the bug
        // parked in its path. It has to be a diver: formation bugs are snapped
        // back to their grid slot every frame.
        for bug in &mut field.bugs {
            bug.alive = false;
            bug.diving = false;
        }
        let bug = field.bugs.len() - 1;
        field.bugs[bug].alive = true;
        field.bugs[bug].diving = true;
        field.bugs[bug].x = 0.0;
        field.bugs[bug].z = config::GALAGA_PLAYER_Z + 3.0;
        field.player_x = 0.0;
        field.set_input(0.0, true);
        let mut killed = field.update(1.0 / 60.0).killed;
        for _ in 0..120 {
            field.set_input(0.0, false);
            killed += field.update(1.0 / 60.0).killed;
            if !field.bugs[bug].alive {
                break;
            }
        }
        assert!(killed > 0, "the beam should reach the bug");
        assert!(!field.bugs[bug].alive);
        assert_eq!(field.score, GalagaSim::bug_score(field.bugs[bug].row));
    }

    #[test]
    fn clearing_the_formation_wins() {
        let mut field = sim(200);
        for bug in &mut field.bugs {
            bug.alive = false;
        }
        let events = field.update(1.0 / 60.0);
        assert!(events.cleared);
        assert!(events.ended);
        assert_eq!(field.phase, GalagaPhase::Won);
        assert!(
            !field.update(1.0 / 60.0).ended,
            "ending reports exactly once"
        );
    }

    #[test]
    fn a_diver_that_misses_loops_back_into_formation() {
        let mut field = sim(200);
        let bug = field.bugs.iter().position(|bug| bug.alive).unwrap();
        field.bugs[bug].diving = true;
        field.bugs[bug].x = 0.0;
        field.bugs[bug].z = 0.0;
        for _ in 0..600 {
            field.set_input(0.0, false);
            field.update(1.0 / 60.0);
            if !field.bugs[bug].diving {
                break;
            }
        }
        assert!(!field.bugs[bug].diving, "the diver should wrap back");
        assert!(field.bugs[bug].z > config::GALAGA_PLAYER_Z);
    }

    #[test]
    fn collisions_cost_lives_and_then_the_field() {
        let mut field = sim(200);
        for bug in &mut field.bugs {
            bug.alive = false;
            bug.diving = false;
        }
        // One bystander in the formation so the field does not read as cleared
        // when the diving bug is destroyed on contact.
        field.bugs[1].alive = true;
        field.player_x = 0.0;

        for life in (0..config::GALAGA_LIVES).rev() {
            field.bugs[0].alive = true;
            field.bugs[0].diving = true;
            field.bugs[0].x = 0.0;
            field.bugs[0].z = config::GALAGA_PLAYER_Z;
            field.invuln = 0.0;
            let events = field.update(1.0 / 60.0);
            assert!(events.lost_life, "the bug on the cycle should hit");
            assert_eq!(field.lives, life, "one life per hit");
        }
        assert_eq!(field.phase, GalagaPhase::Lost);
    }

    #[test]
    fn the_formation_marching_down_ends_the_run() {
        let mut field = sim(200);
        field.formation_z = config::GALAGA_PLAYER_Z + 1.0;
        let events = field.update(1.0 / 60.0);
        assert!(events.overrun);
        assert!(events.ended);
        assert_eq!(field.phase, GalagaPhase::Lost);
    }

    #[test]
    fn restarting_relays_the_formation() {
        let mut field = sim(200);
        for bug in &mut field.bugs {
            bug.alive = false;
        }
        field.restart();
        assert_eq!(field.bugs, sim(200).bugs);
        assert_eq!(field.phase, GalagaPhase::Fighting);
    }
}
