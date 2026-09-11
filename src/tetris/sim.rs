//! Bevy-free Tetris: falling indentation blocks cleared in lines.
//!
//! The board is `TETRIS_COLS` wide and `TETRIS_ROWS` tall on the X/Y plane.
//! Seven tetrominoes fall, rotate, and lock; a full line clears and everything
//! above drops. Clear the seeded target number of lines to win; lock a piece
//! above the rim to lose.

use crate::config;

/// The seven tetrominoes as four cell offsets each.
pub const TETROMINOES: [[(i32, i32); 4]; 7] = [
    [(0, 0), (1, 0), (0, 1), (1, 1)], // O
    [(0, 0), (0, 1), (0, 2), (0, 3)], // I
    [(0, 0), (0, 1), (0, 2), (1, 2)], // L
    [(1, 0), (1, 1), (1, 2), (0, 2)], // J
    [(0, 0), (1, 0), (1, 1), (2, 1)], // S
    [(1, 0), (2, 0), (0, 1), (1, 1)], // Z
    [(0, 0), (1, 0), (2, 0), (1, 1)], // T
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TetrisPhase {
    Falling,
    Won,
    Lost,
}

impl TetrisPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Falling => "FALLING",
            Self::Won => "CLEARED",
            Self::Lost => "TOPPED OUT",
        }
    }
}

/// What one frame produced, for sound and labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TetrisEvents {
    pub rotated: bool,
    pub locked: bool,
    pub lines: u32,
    pub cleared: bool,
    pub lost: bool,
}

/// One input frame: edge left/right, edge rotate, held soft drop, edge hard drop.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TetrisInput {
    pub slide: i32,
    pub rotate: bool,
    pub soft: bool,
    pub drop: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TetrisSim {
    /// Row-major `cols * rows`; `None` is empty, `Some(colour)` a landed block.
    pub board: Vec<Option<u8>>,
    pub piece: usize,
    pub rotation: usize,
    pub x: i32,
    pub y: i32,
    pub lines: u32,
    pub phase: TetrisPhase,
    pub input: TetrisInput,
    fall_clock: f32,
    rng: u64,
    seed: u64,
    file_lines: usize,
}

impl TetrisSim {
    pub fn new(seed: u64, lines: usize) -> Self {
        let mut sim = Self {
            board: vec![None; config::TETRIS_COLS * config::TETRIS_ROWS],
            piece: 0,
            rotation: 0,
            x: config::TETRIS_COLS as i32 / 2 - 1,
            y: config::TETRIS_ROWS as i32 - 2,
            lines: 0,
            phase: TetrisPhase::Falling,
            input: TetrisInput::default(),
            fall_clock: config::TETRIS_FALL_SECONDS,
            rng: seed | 1,
            seed,
            file_lines: lines,
        };
        sim.piece = sim.next_piece();
        sim
    }

    fn unit(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 40) as f32 / (1_u32 << 24) as f32
    }

    fn next_piece(&mut self) -> usize {
        (self.unit() * TETROMINOES.len() as f32) as usize % TETROMINOES.len()
    }

    pub fn set_input(&mut self, slide: i32, rotate: bool, soft: bool, drop: bool) {
        self.input = TetrisInput {
            slide,
            rotate,
            soft,
            drop,
        };
    }

    fn cells(&self, piece: usize, rotation: usize, x: i32, y: i32) -> [(i32, i32); 4] {
        let mut cells = TETROMINOES[piece];
        for _ in 0..rotation {
            for cell in &mut cells {
                // Rotate 90° clockwise around the piece's origin.
                let (px, py) = *cell;
                *cell = (-py, px);
            }
        }
        cells.map(|(cx, cy)| (x + cx, y - cy))
    }

    fn fits(&self, piece: usize, rotation: usize, x: i32, y: i32) -> bool {
        self.cells(piece, rotation, x, y).iter().all(|&(cx, cy)| {
            cx >= 0
                && cx < config::TETRIS_COLS as i32
                && cy >= 0
                && cy < config::TETRIS_ROWS as i32
                && self.board[cx as usize + cy as usize * config::TETRIS_COLS].is_none()
        })
    }

    fn lock(&mut self) -> bool {
        for (cx, cy) in self.cells(self.piece, self.rotation, self.x, self.y) {
            if cy < 0 {
                self.phase = TetrisPhase::Lost;
                return false;
            }
            self.board[cx as usize + cy as usize * config::TETRIS_COLS] = Some(self.piece as u8);
        }
        self.piece = self.next_piece();
        self.rotation = 0;
        self.x = config::TETRIS_COLS as i32 / 2 - 1;
        self.y = config::TETRIS_ROWS as i32 - 2;
        true
    }

    /// Clears full lines and drops everything above them.
    fn clear_lines(&mut self) -> u32 {
        let cols = config::TETRIS_COLS;
        let rows = config::TETRIS_ROWS;
        let mut cleared = 0;
        let mut row = rows as i32 - 1;
        while row >= 0 {
            let full = (0..cols).all(|col| self.board[col + row as usize * cols].is_some());
            if full {
                cleared += 1;
                for shift in (0..row).rev() {
                    for col in 0..cols {
                        self.board[col + (shift as usize + 1) * cols] =
                            self.board[col + shift as usize * cols];
                    }
                }
                for col in 0..cols {
                    self.board[col] = None;
                }
            } else {
                row -= 1;
            }
        }
        self.lines += cleared;
        cleared
    }

    pub fn update(&mut self, dt: f32) -> TetrisEvents {
        let mut events = TetrisEvents::default();
        let input = std::mem::take(&mut self.input);
        self.input.slide = 0;
        self.input.soft = false;
        if self.phase != TetrisPhase::Falling {
            return events;
        }

        if input.slide != 0 && self.fits(self.piece, self.rotation, self.x + input.slide, self.y) {
            self.x += input.slide;
        }
        if input.rotate {
            let rotation = (self.rotation + 1) % 4;
            if self.fits(self.piece, rotation, self.x, self.y) {
                self.rotation = rotation;
                events.rotated = true;
            }
        }
        if input.drop {
            while self.fits(self.piece, self.rotation, self.x, self.y + 1) {
                self.y += 1;
            }
            self.fall_clock = 0.0;
        }

        let interval = if input.soft {
            config::TETRIS_FALL_SECONDS * 0.2
        } else {
            config::TETRIS_FALL_SECONDS
        };
        self.fall_clock -= dt;
        if self.fall_clock <= 0.0 {
            if self.fits(self.piece, self.rotation, self.x, self.y + 1) {
                self.y += 1;
            } else {
                if !self.lock() {
                    events.lost = true;
                    return events;
                }
                events.locked = true;
                let lines = self.clear_lines();
                events.lines = lines;
            }
            self.fall_clock = interval;
        }

        if self.lines >= config::TETRIS_TARGET_LINES {
            self.phase = TetrisPhase::Won;
            events.cleared = true;
        }
        events
    }

    pub fn restart(&mut self) {
        *self = Self::new(self.seed, self.file_lines);
    }
}

#[cfg(test)]
mod tests {
    use super::{TETROMINOES, TetrisPhase, TetrisSim};
    use crate::config;

    fn sim() -> TetrisSim {
        TetrisSim::new(5, 200)
    }

    #[test]
    fn the_same_seed_builds_the_same_board() {
        assert_eq!(sim(), sim());
    }

    #[test]
    fn all_seven_pieces_fit_at_spawn() {
        for piece in 0..TETROMINOES.len() {
            let mut game = sim();
            game.piece = piece;
            game.rotation = 0;
            assert!(
                game.fits(piece, 0, game.x, game.y),
                "piece {piece} should fit at spawn"
            );
        }
    }

    #[test]
    fn sliding_and_rotating_move_the_piece() {
        let mut game = sim();
        let x = game.x;
        game.set_input(1, false, false, false);
        game.update(1.0 / 60.0);
        assert_eq!(game.x, x + 1);
        game.set_input(0, true, false, false);
        let events = game.update(1.0 / 60.0);
        assert!(events.rotated);
        assert_eq!(game.rotation, 1);
    }

    #[test]
    fn a_full_line_clears() {
        let mut game = sim();
        let cols = config::TETRIS_COLS;
        for col in 0..cols {
            game.board[col + (config::TETRIS_ROWS - 1) * cols] = Some(1);
        }
        assert_eq!(game.clear_lines(), 1);
        assert_eq!(game.lines, 1);
        assert!((0..cols).all(|col| game.board[col].is_none()));
    }

    #[test]
    fn hard_drop_locks_the_piece() {
        let mut game = sim();
        game.set_input(0, false, false, true);
        let events = game.update(1.0 / 60.0);
        assert!(events.locked);
        assert!(game.board.iter().any(|cell| cell.is_some()));
    }

    #[test]
    fn reaching_the_target_wins() {
        let mut game = sim();
        game.lines = config::TETRIS_TARGET_LINES;
        let events = game.update(1.0 / 60.0);
        assert!(events.cleared);
        assert_eq!(game.phase, TetrisPhase::Won);
    }

    #[test]
    fn restarting_relays_the_board() {
        let mut game = sim();
        game.board.clear();
        game.restart();
        assert_eq!(game.board, sim().board);
        assert_eq!(game.phase, TetrisPhase::Falling);
    }
}
