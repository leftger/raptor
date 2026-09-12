//! The process scheduler, played as a race.
//!
//! Rival threads share the directory's roads and run for the same gate the
//! rider is heading for. Everything here is Bevy-free so the pathing can be
//! unit-tested; the plugin layer owns the entities and the collision check.

use std::collections::BTreeSet;

use crate::config;

/// One rival thread running the roads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Racer {
    pub cell: (i32, i32),
    /// The cell it came from, so it never doubles straight back.
    pub prev: Option<(i32, i32)>,
    /// Cells covered, for tie-breaks and for the HUD.
    pub travelled: u32,
}

impl Racer {
    fn new(cell: (i32, i32)) -> Self {
        Self {
            cell,
            prev: None,
            travelled: 0,
        }
    }

    /// True once this thread has reached the gate.
    ///
    /// The gate cell itself is not always part of the road set, so standing on
    /// a road cell orthogonally next to it counts as an arrival too.
    pub fn finished(&self, target: (i32, i32)) -> bool {
        (target.0 - self.cell.0).abs() + (target.1 - self.cell.1).abs() <= 1
    }
}

/// What one update of the race produced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RaceEvents {
    /// A rival thread reached the gate this update.
    pub rival_reached_gate: bool,
}

/// Every rival thread in one directory, running toward `target`.
#[derive(Debug, Clone)]
pub struct RaceSim {
    pub racers: Vec<Racer>,
    pub target: (i32, i32),
    /// Leftover time toward the next cell step, shared by all racers.
    timer: f32,
}

impl RaceSim {
    /// Starts one thread per start cell. Fewer than one start means no race.
    pub fn new(starts: Vec<(i32, i32)>, target: (i32, i32)) -> Self {
        Self {
            racers: starts.into_iter().map(Racer::new).collect(),
            target,
            timer: 0.0,
        }
    }

    /// Advances the field one cell per [`config::SCHEDULER_STEP_SECONDS`].
    ///
    /// A racer that already stands on the gate stops moving and reports the
    /// arrival; the plugin turns that into a context switch.
    pub fn update(&mut self, dt: f32, roads: &BTreeSet<(i32, i32)>) -> RaceEvents {
        let mut events = RaceEvents::default();
        self.timer += dt;
        while self.timer >= config::SCHEDULER_STEP_SECONDS {
            self.timer -= config::SCHEDULER_STEP_SECONDS;
            for racer in &mut self.racers {
                if racer.finished(self.target) {
                    events.rival_reached_gate = true;
                    continue;
                }
                let Some(next) = next_cell(racer, self.target, roads) else {
                    continue;
                };
                racer.prev = Some(racer.cell);
                racer.cell = next;
                racer.travelled += 1;
                if racer.finished(self.target) {
                    events.rival_reached_gate = true;
                }
            }
        }
        events
    }
}

/// Picks the neighbouring road cell that closes on the target.
///
/// Reversing is avoided unless it is the only road left, so a thread in a dead
/// end walks back out instead of stalling. Ties break on the fixed neighbour
/// order, which keeps a given arena reproducible.
pub fn next_cell(
    racer: &Racer,
    target: (i32, i32),
    roads: &BTreeSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    let (cx, cz) = racer.cell;
    let neighbours = [(cx + 1, cz), (cx - 1, cz), (cx, cz + 1), (cx, cz - 1)];
    let mut best: Option<((i32, i32), i32)> = None;
    for cell in neighbours {
        if !roads.contains(&cell) || Some(cell) == racer.prev {
            continue;
        }
        let distance = (target.0 - cell.0).abs() + (target.1 - cell.1).abs();
        if best.is_none_or(|(_, best_distance)| distance < best_distance) {
            best = Some((cell, distance));
        }
    }
    if best.is_none()
        && let Some(prev) = racer.prev
        && roads.contains(&prev)
    {
        return Some(prev);
    }
    best.map(|(cell, _)| cell)
}

/// Start cells for a race: the road cells farthest from the gate, so the
/// threads come from across the directory rather than on top of the rider.
pub fn start_cells(
    roads: &BTreeSet<(i32, i32)>,
    target: (i32, i32),
    count: usize,
) -> Vec<(i32, i32)> {
    let mut candidates: Vec<((i32, i32), i32)> = roads
        .iter()
        .map(|cell| (*cell, (target.0 - cell.0).abs() + (target.1 - cell.1).abs()))
        .filter(|(_, distance)| *distance > 1)
        .collect();
    candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    candidates
        .into_iter()
        .take(count)
        .map(|(cell, _)| cell)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{RaceSim, Racer, next_cell, start_cells};
    use crate::config;
    use std::collections::BTreeSet;

    fn road(cells: &[(i32, i32)]) -> BTreeSet<(i32, i32)> {
        cells.iter().copied().collect()
    }

    #[test]
    fn a_racer_steps_toward_the_gate_and_never_reverses() {
        let roads = road(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        let mut racer = Racer {
            cell: (0, 0),
            prev: None,
            travelled: 0,
        };
        racer.prev = Some(racer.cell);
        racer.cell = next_cell(&racer, (3, 0), &roads).expect("a step");
        assert_eq!(racer.cell, (1, 0));
        // From here the only closing road is onward; the cell behind is skipped.
        assert_eq!(next_cell(&racer, (3, 0), &roads), Some((2, 0)));
    }

    #[test]
    fn a_thread_in_a_dead_end_walks_back_out() {
        let roads = road(&[(0, 0), (1, 0)]);
        let racer = Racer {
            cell: (1, 0),
            prev: Some((0, 0)),
            travelled: 1,
        };
        assert_eq!(next_cell(&racer, (9, 9), &roads), Some((0, 0)));
    }

    #[test]
    fn a_boxed_in_thread_has_nowhere_to_go() {
        let roads = road(&[(0, 0)]);
        let racer = Racer {
            cell: (0, 0),
            prev: None,
            travelled: 0,
        };
        assert_eq!(next_cell(&racer, (5, 5), &roads), None);
    }

    #[test]
    fn the_race_moves_one_cell_per_step_and_reports_the_arrival() {
        let roads = road(&[(0, 0), (1, 0), (2, 0)]);
        let mut race = RaceSim::new(vec![(0, 0)], (2, 0));
        // The first step lands beside the gate, which already counts.
        let events = race.update(config::SCHEDULER_STEP_SECONDS, &roads);
        assert!(events.rival_reached_gate, "adjacency is an arrival");
        assert_eq!(race.racers[0].cell, (1, 0));
        assert_eq!(race.racers[0].travelled, 1);
    }

    #[test]
    fn threads_start_across_the_directory_not_on_the_gate() {
        let roads = road(&[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]);
        let starts = start_cells(&roads, (0, 0), 2);
        assert_eq!(starts, vec![(4, 0), (3, 0)]);
        assert!(!starts.contains(&(0, 0)), "never on the gate itself");
    }
}
