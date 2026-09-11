//! Bevy-free snake run over the shared lightcycle grid.
//!
//! Snake is the base lightcycle game with two twists: the trail has a finite
//! length, and eating scattered power-ups lengthens it. Once enough are eaten
//! the ring's exit gate unlocks and the rider can leave. Movement, queued turns
//! and collision stay in [`LightcycleSim`]; this module owns the power-ups, the
//! tail cap and the exit state, so it unit-tests without a window.

use crate::config;
use crate::lightcycle::logic::LightcycleSim;

/// One power-up waiting on the ring floor.
#[derive(Debug, Clone, PartialEq)]
pub struct Food {
    pub cell: (i32, i32),
    pub eaten: bool,
}

/// What changed over one [`SnakeSim::eat`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnakeEvents {
    pub ate: bool,
    pub opened_exit: bool,
}

/// One run of snake over a ring.
#[derive(Debug, Clone, PartialEq)]
pub struct SnakeSim {
    pub food: Vec<Food>,
    pub eaten: usize,
    /// Power-ups needed before the exit opens.
    pub target: usize,
    /// Current maximum tail length, in cells. Grows with every power-up.
    pub max_tail: usize,
    /// Once enough power-ups are eaten the gate is passable.
    pub exit_open: bool,
    /// Set the first time the caller reports the run crashed.
    dead: bool,
}

impl SnakeSim {
    /// Scatters `target` power-ups over `candidates`, deterministically from
    /// `seed`, keeping them a few cells clear of the rider's spawn.
    pub fn new(seed: u64, spawn: (i32, i32), candidates: &[(i32, i32)], target: usize) -> Self {
        Self {
            food: scatter(seed, spawn, candidates, target.max(1)),
            eaten: 0,
            target: target.max(1),
            max_tail: config::SNAKE_TAIL_START,
            exit_open: false,
            dead: false,
        }
    }

    /// Power-ups still waiting.
    pub fn remaining(&self) -> usize {
        self.food.iter().filter(|food| !food.eaten).count()
    }

    /// Collects the power-up on `cell`, if any is there and uneaten.
    pub fn eat(&mut self, cell: (i32, i32)) -> SnakeEvents {
        let mut events = SnakeEvents::default();
        let Some(slot) = self
            .food
            .iter_mut()
            .find(|food| !food.eaten && food.cell == cell)
        else {
            return events;
        };
        slot.eaten = true;
        self.eaten += 1;
        self.max_tail += config::SNAKE_TAIL_GROWTH;
        events.ate = true;
        if self.eaten >= self.target && !self.exit_open {
            self.exit_open = true;
            events.opened_exit = true;
        }
        events
    }

    /// Caps `sim`'s trail to the current tail length, dropping the oldest cells.
    /// Returns how many cells fell off the end.
    pub fn trim_tail(&self, sim: &mut LightcycleSim) -> usize {
        let excess = sim.trail.len().saturating_sub(self.max_tail);
        if excess > 0 {
            sim.trail.drain(0..excess);
        }
        excess
    }

    /// True exactly once, the first time the run is seen crashed, so the caller
    /// can play a death cue without tracking the phase itself.
    pub fn note_crash(&mut self) -> bool {
        if self.dead {
            return false;
        }
        self.dead = true;
        true
    }
}

/// Picks `target` distinct candidate cells, `SNAKE_MIN_FOOD_DISTANCE` or more
/// from the spawn. Falls back to unspaced cells if the ring is too tight.
fn scatter(seed: u64, spawn: (i32, i32), candidates: &[(i32, i32)], target: usize) -> Vec<Food> {
    let mut food: Vec<Food> = Vec::with_capacity(target);
    if candidates.is_empty() {
        return food;
    }

    let mut rng = seed | 1;
    let mut tries = 0;
    while food.len() < target && tries < target * 64 {
        tries += 1;
        let index = (next_unit(&mut rng) * candidates.len() as f32) as usize % candidates.len();
        let cell = candidates[index];
        if chebyshev(cell, spawn) < config::SNAKE_MIN_FOOD_DISTANCE {
            continue;
        }
        if food.iter().any(|food| food.cell == cell) {
            continue;
        }
        food.push(Food { cell, eaten: false });
    }

    // A stub file's ring can be too small to satisfy the spacing; take whatever
    // distinct cells are left rather than shipping a shorter game.
    for &cell in candidates {
        if food.len() >= target {
            break;
        }
        if food.iter().any(|food| food.cell == cell) {
            continue;
        }
        food.push(Food { cell, eaten: false });
    }
    food
}

fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Next value in `0.0..1.0` from a plain LCG, so the scatter is reproducible
/// across platforms without a dependency.
fn next_unit(rng: &mut u64) -> f32 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*rng >> 40) as f32 / (1_u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::{SnakeSim, chebyshev};
    use crate::config;
    use crate::lightcycle::logic::{Heading, LightcycleSim};

    /// A 21x21 block of cells, plenty for a scatter.
    fn cells() -> Vec<(i32, i32)> {
        (-10..=10)
            .flat_map(|x| (-10..=10).map(move |z| (x, z)))
            .collect()
    }

    #[test]
    fn the_same_seed_scatters_the_same_food() {
        let first = SnakeSim::new(9, (0, 0), &cells(), 6);
        let second = SnakeSim::new(9, (0, 0), &cells(), 6);
        assert_eq!(first.food, second.food);
        assert_eq!(first.food.len(), 6);
    }

    #[test]
    fn food_is_distinct_and_clear_of_the_spawn() {
        let sim = SnakeSim::new(4, (0, 0), &cells(), 6);
        assert_eq!(sim.food.len(), 6);
        for (index, food) in sim.food.iter().enumerate() {
            assert!(
                chebyshev(food.cell, (0, 0)) >= config::SNAKE_MIN_FOOD_DISTANCE,
                "food landed on top of the rider"
            );
            for other in &sim.food[index + 1..] {
                assert_ne!(food.cell, other.cell, "food cells must be distinct");
            }
        }
    }

    #[test]
    fn a_tight_ring_still_gets_a_full_set() {
        // Four cells only, and all within the preferred spacing.
        let tight = [(1, 1), (1, 2), (2, 1), (2, 2)];
        let sim = SnakeSim::new(1, (0, 0), &tight, 4);
        assert_eq!(sim.food.len(), 4);
        assert_eq!(sim.remaining(), 4);
    }

    #[test]
    fn eating_grows_the_tail_and_opens_the_exit_at_the_target() {
        let mut sim = SnakeSim::new(3, (0, 0), &cells(), 3);
        let start = sim.max_tail;

        let food: Vec<_> = sim.food.iter().map(|food| food.cell).collect();
        let first = sim.eat(food[0]);
        assert!(first.ate);
        assert!(!first.opened_exit);
        assert_eq!(sim.max_tail, start + config::SNAKE_TAIL_GROWTH);
        assert_eq!(sim.remaining(), 2);

        // Eating the same cell again does nothing.
        assert!(!sim.eat(food[0]).ate);
        assert_eq!(sim.remaining(), 2);

        sim.eat(food[1]);
        let last = sim.eat(food[2]);
        assert!(last.ate);
        assert!(last.opened_exit, "the exit opens on the last power-up");
        assert!(sim.exit_open);
        assert_eq!(sim.remaining(), 0);
    }

    #[test]
    fn the_tail_is_trimmed_to_the_current_length() {
        let sim = SnakeSim::new(5, (0, 0), &cells(), 1);
        let mut cycle = LightcycleSim::start((0, 0), Heading::PosX);
        let body: Vec<(i32, i32)> = (0..12).map(|x| (x, 0)).collect();
        cycle.trail = body.clone();
        let dropped = sim.trim_tail(&mut cycle);
        assert_eq!(dropped, body.len() - config::SNAKE_TAIL_START);
        assert_eq!(cycle.trail.len(), config::SNAKE_TAIL_START);
        // The newest cells survive; the oldest fall off the back.
        assert!(cycle.trail.contains(&(11, 0)));
        assert!(!cycle.trail.contains(&(0, 0)));
    }

    #[test]
    fn a_crash_is_reported_exactly_once() {
        let mut sim = SnakeSim::new(1, (0, 0), &cells(), 1);
        assert!(sim.note_crash());
        assert!(!sim.note_crash());
    }
}
