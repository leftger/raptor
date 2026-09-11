//! Cheap, deterministic fingerprinting of a source file into a disc-wars ring.
//!
//! Nothing here compiles or fully parses the file. It walks the bytes once with
//! a handful of substring rules, then lays out a circular arena whose geometry,
//! hazards, galleries, and pickups are all derived from the same
//! `stable_path_seed(path) ^ content_hash` fingerprint the document pages use.
//! The same file therefore always produces the same ring.

use crate::config;
use crate::disc::language::SourceLanguage;
use crate::lightcycle::logic::{
    Arena, ArenaKind, CityTheme, Heading, ParentPortal, Wall, stable_path_seed,
};
use std::collections::BTreeSet;
use std::f32::consts::TAU;
use std::path::Path;

/// One pickup verb a construct maps to. See the plan's powerup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    /// `TODO` / `unimplemented!` — next throw phases through one wall.
    Glitch,
    /// `async` / threads / `go` — longer reach (a delayed echo).
    Split,
    /// `match` / `switch` — the disc turns once at a wall instead of returning.
    Fork,
    /// Generics / templates — slower, heavier disc.
    Heavy,
    /// `pub` / exports — extra range.
    Widen,
    /// Panics / `abort` / `raise` — lethal on the return path too.
    Spike,
    /// Doc comments — one free absorb.
    Shield,
    /// `cfg` / `#ifdef` — a short window of intangibility.
    Phase,
    /// Tests / safe pads — the hazard fuse refills.
    Recharge,
}

impl PickupKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Glitch => "glitch disc",
            Self::Split => "split disc",
            Self::Fork => "fork",
            Self::Heavy => "heavy disc",
            Self::Widen => "widen",
            Self::Spike => "spike",
            Self::Shield => "shield",
            Self::Phase => "phase floor",
            Self::Recharge => "recharge",
        }
    }
}

/// One pickup waiting on the ring floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickupSpot {
    pub cell: (i32, i32),
    pub kind: PickupKind,
}

/// A function/method alcove, kept for the folio panel and for spawn landmarks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscBlock {
    /// Full preserved signature/line, for the folio panel.
    pub text: String,
    /// Short glyph preview.
    pub preview: String,
    /// Where the alcove's raised gallery stands.
    pub wall: (i32, i32),
    /// The floor cell the reader stands on to focus this alcove.
    pub landmark: (i32, i32),
}

/// Counts lifted from the file by [`tokenize_source`]. Every one is a cheap
/// substring tally, not a parse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceSignals {
    pub lines: usize,
    pub functions: usize,
    pub comments: usize,
    pub doc_comments: usize,
    pub tests: usize,
    pub todos: usize,
    pub unsafe_count: usize,
    pub generics: usize,
    pub async_count: usize,
    pub match_count: usize,
    pub pub_count: usize,
    pub panics: usize,
    pub cfg_count: usize,
    pub imports: usize,
    /// Trimmed signature lines, capped, used for the alcoves' folio text.
    pub function_lines: Vec<String>,
}

impl SourceSignals {
    /// Tokens per line, used to make a dense file field an aggressive opponent.
    pub fn density(&self) -> f32 {
        if self.lines == 0 {
            return 0.0;
        }
        let tokens = self.functions
            + self.match_count
            + self.async_count
            + self.unsafe_count
            + self.pub_count
            + self.generics
            + self.panics;
        tokens as f32 / self.lines as f32
    }

    /// Fraction of lines that are comments. A mostly-commented file fields a
    /// defensive opponent.
    pub fn comment_ratio(&self) -> f32 {
        if self.lines == 0 {
            return 0.0;
        }
        (self.comments + self.doc_comments) as f32 / self.lines as f32
    }
}

/// Everything the ring adds on top of the shared [`Arena`].
#[derive(Debug, Clone, PartialEq)]
pub struct DiscLayout {
    pub radius: i32,
    pub center: (i32, i32),
    pub hazards: BTreeSet<(i32, i32)>,
    pub safe_pads: BTreeSet<(i32, i32)>,
    pub pickups: Vec<PickupSpot>,
    pub blocks: Vec<DiscBlock>,
    pub player_spawn: (i32, i32),
    pub player_spawn_heading: Heading,
    pub opponent_spawn: (i32, i32),
    /// A tiny practice ring: slow opponent, no real hazards.
    pub practice: bool,
    pub truncated: bool,
    pub lossy_utf8: bool,
    /// The fingerprint the ring was carved from. The asteroid field reuses it so
    /// its wave is as reproducible as the ring itself.
    pub seed: u64,
    pub signals: SourceSignals,
}

impl DiscLayout {
    /// Nearest alcove to `cell`, within a cell or two, for the folio panel.
    pub fn focused_block(&self, cell: (i32, i32)) -> Option<usize> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                (
                    (block.landmark.0 - cell.0).abs() + (block.landmark.1 - cell.1).abs(),
                    index,
                )
            })
            .filter(|(distance, _)| *distance <= 2)
            .min()
            .map(|(_, index)| index)
    }

    pub fn pickup_at(&self, cell: (i32, i32)) -> Option<usize> {
        self.pickups.iter().position(|spot| spot.cell == cell)
    }
}

/// Builds the ring arena and its layout from a source file's bytes.
pub fn build_disc_arena(
    path: &Path,
    language: SourceLanguage,
    bytes: &[u8],
) -> (Arena, DiscLayout) {
    build_arena(path, language, bytes, None)
}

/// Like [`build_disc_arena`], but the ring is capped at `max_radius` cells.
///
/// The asteroid field wants a small, fully visible playfield no matter how long
/// the file is, while still reusing the shared ring geometry, close gate and
/// restore path.
pub fn build_capped_disc_arena(
    path: &Path,
    language: SourceLanguage,
    bytes: &[u8],
    max_radius: i32,
) -> (Arena, DiscLayout) {
    build_arena(path, language, bytes, Some(max_radius))
}

/// A metadata-only arena for source runs played off the grid.
///
/// The platformer, the brick breaker and the stealth run build their own space,
/// so nothing here is carved, walled or gated. They still need what every source
/// run shares: the fingerprint that seeds their procedural layout, the size of
/// the file that was read, and the truncation flags the HUD reports.
pub fn build_flat_arena(
    path: &Path,
    language: SourceLanguage,
    bytes: &[u8],
    half_extent: i32,
) -> (Arena, DiscLayout) {
    let truncated = bytes.len() > config::SOURCE_MAX_BYTES;
    let slice = if truncated {
        let mut end = config::SOURCE_MAX_BYTES;
        while end > 0 && !is_char_boundary(bytes, end) {
            end -= 1;
        }
        &bytes[..end]
    } else {
        bytes
    };
    let (text, lossy_utf8) = match std::str::from_utf8(slice) {
        Ok(text) => (text.to_string(), false),
        Err(_) => (String::from_utf8_lossy(slice).into_owned(), true),
    };

    let signals = tokenize_source(&text, language);
    let seed = stable_path_seed(path) ^ content_hash(&signals) ^ language_pepper(language);
    let half = half_extent.max(2);
    let roads: BTreeSet<(i32, i32)> = (-half..=half)
        .flat_map(|x| (-half..=half).map(move |z| (x, z)))
        .collect();

    let arena = Arena {
        min: (-half, -half),
        max: (half, half),
        parent_portal: None,
        kind: ArenaKind::Disc,
        roads,
        street_walls: BTreeSet::new(),
        structures: Vec::new(),
        city_theme: CityTheme::from_seed(seed),
    };
    let layout = DiscLayout {
        radius: half,
        center: (0, 0),
        hazards: BTreeSet::new(),
        safe_pads: BTreeSet::new(),
        pickups: Vec::new(),
        blocks: Vec::new(),
        player_spawn: (0, 0),
        player_spawn_heading: Heading::PosX,
        opponent_spawn: (0, 0),
        practice: false,
        truncated,
        lossy_utf8,
        seed,
        signals,
    };
    (arena, layout)
}

fn build_arena(
    path: &Path,
    language: SourceLanguage,
    bytes: &[u8],
    max_radius: Option<i32>,
) -> (Arena, DiscLayout) {
    let truncated = bytes.len() > config::SOURCE_MAX_BYTES;
    let slice = if truncated {
        let mut end = config::SOURCE_MAX_BYTES;
        while end > 0 && !is_char_boundary(bytes, end) {
            end -= 1;
        }
        &bytes[..end]
    } else {
        bytes
    };
    let (text, lossy_utf8) = match std::str::from_utf8(slice) {
        Ok(text) => (text.to_string(), false),
        Err(_) => (String::from_utf8_lossy(slice).into_owned(), true),
    };

    let signals = tokenize_source(&text, language);
    let seed = stable_path_seed(path) ^ content_hash(&signals) ^ language_pepper(language);

    let lines = signals.lines.max(1) as i32;
    let practice = signals.lines <= 3 && signals.functions == 0;
    let computed = if practice {
        config::DISC_RADIUS_MIN
    } else {
        (config::DISC_RADIUS_MIN
            + lines / 24
            + signals.functions as i32 / 2
            + signals.unsafe_count as i32 / 3)
            .clamp(config::DISC_RADIUS_MIN, config::DISC_RADIUS_MAX)
    };
    let radius = match max_radius {
        // Never so small that the carved ring or its gate has no room.
        Some(max) => computed.min(max.max(3)),
        None => computed,
    };

    let center = (0, 0);
    let gate_wall = gate_wall(seed);
    let corridor = gate_corridor(center, radius, gate_wall);
    let half = radius + config::DISC_GATE_DEPTH + 1;

    let playable = |cell: (i32, i32)| {
        let dx = cell.0 - center.0;
        let dz = cell.1 - center.1;
        dx * dx + dz * dz <= radius * radius
    };

    // Galleries: one short radial wall stub per function, capped.
    let mut walls = BTreeSet::new();
    let mut blocks = Vec::new();
    let mut reserved = BTreeSet::new();
    reserved.insert(center);
    reserved.extend(corridor.iter().copied());

    let gallery_count = signals
        .functions
        .min(config::DISC_MAX_GALLERIES)
        .min(signals.function_lines.len());
    for index in 0..gallery_count {
        let hash = mix(seed ^ 0x9e37_79b9_7f4a_7c15, index as u64 + 1);
        let angle = (hash % 10_000) as f32 / 10_000.0 * TAU;
        let inner = (radius as f32 * 0.35).max(2.0);
        let outer = (radius as f32 * 0.72).max(inner + 1.0);
        let span = (outer - inner).max(0.0);
        let ring = inner + ((hash >> 16) & 0xFFFF) as f32 / u16::MAX as f32 * span;
        let wall = (
            center.0 + (angle.cos() * ring).round() as i32,
            center.1 + (angle.sin() * ring).round() as i32,
        );
        let toward_center = step_toward(wall, center);
        if !playable(wall)
            || !playable(toward_center)
            || reserved.contains(&wall)
            || reserved.contains(&toward_center)
            || walls.contains(&wall)
        {
            continue;
        }
        walls.insert(wall);
        reserved.insert(wall);
        reserved.insert(toward_center);
        let text = signals
            .function_lines
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("fn {}", language.name()));
        let preview =
            crate::document::parse::truncate_chars(&text, config::DOCUMENT_PARAGRAPH_GLYPHS);
        blocks.push(DiscBlock {
            text,
            preview,
            wall,
            landmark: toward_center,
        });
    }

    // Floor features. Candidate cells are playable, un-walled, un-reserved, and
    // shared across hazards / pads / pickups so nothing overlaps.
    let mut floor_reserved = reserved.clone();
    let mut candidates: Vec<(i32, i32)> = Vec::new();
    for x in (center.0 - radius)..=(center.0 + radius) {
        for z in (center.1 - radius)..=(center.1 + radius) {
            let cell = (x, z);
            if playable(cell) && !walls.contains(&cell) && !reserved.contains(&cell) {
                candidates.push(cell);
            }
        }
    }
    candidates.sort_unstable_by_key(|cell| mix(seed ^ 0x51ed_2701, cell_key(*cell)));

    let take_cell = |candidates: &[(i32, i32)], reserved: &mut BTreeSet<(i32, i32)>| {
        candidates
            .iter()
            .copied()
            .find(|cell| reserved.insert(*cell))
    };

    let mut hazards = BTreeSet::new();
    let hazard_count = if practice {
        0
    } else {
        (signals.unsafe_count * 2 + signals.todos + signals.panics).min(config::DISC_MAX_HAZARDS)
    };
    for _ in 0..hazard_count {
        if let Some(cell) = take_cell(&candidates, &mut floor_reserved) {
            hazards.insert(cell);
        }
    }

    let mut safe_pads = BTreeSet::new();
    for _ in 0..signals.tests.min(config::DISC_MAX_SAFE_PADS) {
        if let Some(cell) = take_cell(&candidates, &mut floor_reserved) {
            safe_pads.insert(cell);
        }
    }

    let mut pickups = Vec::new();
    for kind in pickup_kinds(&signals) {
        if pickups.len() >= config::DISC_MAX_PICKUPS {
            break;
        }
        if let Some(cell) = take_cell(&candidates, &mut floor_reserved) {
            pickups.push(PickupSpot { cell, kind });
        }
    }

    // Spawns: the rider at the center, the opponent opposite the gate.
    let opponent_spawn = opponent_spawn(center, radius, gate_wall);
    let player_spawn = center;
    floor_reserved.insert(player_spawn);
    floor_reserved.insert(opponent_spawn);

    let mut street_walls: BTreeSet<(i32, i32)> = BTreeSet::new();
    for x in (center.0 - half)..=(center.0 + half) {
        for z in (center.1 - half)..=(center.1 + half) {
            let cell = (x, z);
            if !playable(cell) && !corridor.contains(&cell) {
                street_walls.insert(cell);
            }
        }
    }
    street_walls.extend(walls.iter().copied().filter(|cell| playable(*cell)));

    let roads: BTreeSet<(i32, i32)> = (center.0 - radius..=center.0 + radius)
        .flat_map(|x| {
            (center.1 - radius..=center.1 + radius)
                .map(move |z| (x, z))
                .filter(|cell| playable(*cell) && !walls.contains(cell))
        })
        .collect();

    let (from, to) = corridor_span(&corridor);
    let arena = Arena {
        min: (center.0 - half, center.1 - half),
        max: (center.0 + half, center.1 + half),
        parent_portal: Some(ParentPortal {
            wall: gate_wall,
            from,
            to,
        }),
        kind: ArenaKind::Disc,
        roads,
        street_walls,
        structures: Vec::new(),
        city_theme: CityTheme::from_seed(seed),
    };

    let player_spawn_heading = heading_toward(player_spawn, opponent_spawn);
    let layout = DiscLayout {
        radius,
        center,
        hazards,
        safe_pads,
        pickups,
        blocks,
        player_spawn,
        player_spawn_heading,
        opponent_spawn,
        practice,
        truncated,
        lossy_utf8,
        seed,
        signals,
    };
    (arena, layout)
}

/// One pass of cheap construct tallies over the file's lines.
pub fn tokenize_source(text: &str, language: SourceLanguage) -> SourceSignals {
    let mut signals = SourceSignals::default();
    for raw in text.lines().take(config::SOURCE_MAX_LINES) {
        signals.lines += 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let is_comment = line.starts_with("//")
            || line.starts_with('#')
            || line.starts_with("/*")
            || line.starts_with('*');
        if is_comment {
            signals.comments += 1;
        }
        if line.starts_with("///")
            || line.starts_with("//!")
            || line.starts_with("/**")
            || line.starts_with("##")
        {
            signals.doc_comments += 1;
        }

        if contains_any(line, &["TODO", "FIXME", "unimplemented!", "todo!"]) {
            signals.todos += 1;
        }
        if contains_any(
            line,
            &[
                "unsafe",
                "goto ",
                "eval(",
                "as *mut",
                "as *const",
                "reinterpret_cast",
            ],
        ) {
            signals.unsafe_count += 1;
        }
        if contains_any(
            line,
            &[
                "async ",
                "await ",
                "thread::spawn",
                "std::thread",
                "go func",
                "go ",
            ],
        ) {
            signals.async_count += 1;
        }
        if contains_any(line, &["match ", "switch ", "switch("]) {
            signals.match_count += 1;
        }
        if contains_any(line, &["pub ", "export ", "__all__"]) {
            signals.pub_count += 1;
        }
        if contains_any(
            line,
            &["panic!", "abort(", "raise ", ".unwrap()", "expect("],
        ) {
            signals.panics += 1;
        }
        if contains_any(line, &["cfg(", "#ifdef", "#if ", "#[cfg"]) {
            signals.cfg_count += 1;
        }
        if contains_any(line, &["#include", "use ", "import ", "from ", "require("]) {
            signals.imports += 1;
        }
        if contains_any(
            line,
            &["#[test]", "def test_", "fn test_", "_test(", "_test."],
        ) {
            signals.tests += 1;
        }
        if line.contains('<')
            && line.contains('>')
            && contains_any(
                line,
                &[
                    "impl", "template", "typename", "Vec<", "Option<", "fn ", "struct ", "class ",
                ],
            )
        {
            signals.generics += 1;
        }

        if is_function_line(line, language) {
            signals.functions += 1;
            if signals.function_lines.len() < config::DISC_MAX_GALLERIES * 2 {
                signals.function_lines.push(line.to_string());
            }
        }
    }
    signals
}

fn contains_any(line: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| line.contains(needle))
}

/// Approximate function detector: cheap and language-shaped, not a parse.
fn is_function_line(line: &str, language: SourceLanguage) -> bool {
    match language {
        SourceLanguage::Rust => {
            line.starts_with("fn ")
                || line.starts_with("pub fn ")
                || line.starts_with("pub(crate) fn ")
                || line.starts_with("async fn ")
                || line.contains(" fn ")
        }
        SourceLanguage::Python => line.starts_with("def ") || line.starts_with("async def "),
        SourceLanguage::Slint => line.starts_with("callback ") || line.contains("=>"),
        SourceLanguage::Lua => {
            line.starts_with("function ")
                || line.contains("function(")
                || line.contains("= function")
        }
        SourceLanguage::Shell => {
            let head = line.trim_start();
            head.starts_with("function ") || (!head.contains('=') && head.contains("()"))
        }
        // TOML has tables, not functions; the surfer only needs the file's
        // length and fingerprint.
        SourceLanguage::Toml => false,
        // JSON has values, not functions; the Galaga field only needs the
        // file's length and fingerprint.
        SourceLanguage::Json => false,
        // The arcade languages get a cheap, language-shaped detector so their
        // fingerprints still differ by structure.
        SourceLanguage::Go => line.starts_with("func "),
        SourceLanguage::Ruby => line.starts_with("def "),
        SourceLanguage::Yaml => false,
        SourceLanguage::JavaScript => line.starts_with("function ") || line.contains("=>"),
        SourceLanguage::Zig => line.starts_with("fn "),
        SourceLanguage::Php => line.starts_with("function "),
        SourceLanguage::R => line.contains("<- function"),
        SourceLanguage::C | SourceLanguage::Cpp => {
            let Some(open) = line.find('(') else {
                return false;
            };
            let Some(close) = line.rfind(')') else {
                return false;
            };
            if close < open {
                return false;
            }
            let tail = line[close + 1..].trim();
            if tail.contains(';') || line.starts_with(';') {
                return false;
            }
            // Reject control flow and call sites.
            let head = line[..open].trim();
            if head.is_empty()
                || matches!(head, "if" | "for" | "while" | "switch" | "return")
                || head.starts_with("if ")
                || head.starts_with("for ")
                || head.starts_with("while ")
                || head.starts_with("return ")
                || head.starts_with("switch ")
                || head.contains('=')
                || head.ends_with('.')
            {
                return false;
            }
            tail.is_empty()
                || tail == "{"
                || tail.starts_with("const")
                || tail.starts_with("noexcept")
        }
    }
}

fn pickup_kinds(signals: &SourceSignals) -> Vec<PickupKind> {
    let mut kinds = Vec::new();
    if signals.todos > 0 {
        kinds.push(PickupKind::Glitch);
    }
    if signals.async_count > 0 {
        kinds.push(PickupKind::Split);
    }
    if signals.match_count > 0 {
        kinds.push(PickupKind::Fork);
    }
    if signals.generics > 0 {
        kinds.push(PickupKind::Heavy);
    }
    if signals.pub_count > 0 {
        kinds.push(PickupKind::Widen);
    }
    if signals.panics > 0 {
        kinds.push(PickupKind::Spike);
    }
    if signals.doc_comments > 0 {
        kinds.push(PickupKind::Shield);
    }
    if signals.cfg_count > 0 {
        kinds.push(PickupKind::Phase);
    }
    if kinds.is_empty() {
        kinds.push(PickupKind::Recharge);
    }
    kinds
}

fn gate_wall(seed: u64) -> Wall {
    match seed % 4 {
        0 => Wall::NegZ,
        1 => Wall::PosZ,
        2 => Wall::NegX,
        _ => Wall::PosX,
    }
}

fn gate_corridor(center: (i32, i32), radius: i32, wall: Wall) -> Vec<(i32, i32)> {
    let (dx, dz) = cardinal_delta(wall);
    (0..=config::DISC_GATE_DEPTH)
        .map(|step| {
            let distance = radius + step;
            (center.0 + dx * distance, center.1 + dz * distance)
        })
        .collect()
}

/// The gate is the corridor's outermost cell, so the rider travels the whole
/// opening before it closes. The corridor itself is left unwalled but is not a
/// road, so discs cannot fly out through it.
fn corridor_span(corridor: &[(i32, i32)]) -> ((i32, i32), (i32, i32)) {
    let outer = *corridor.last().unwrap_or(&corridor[0]);
    (outer, outer)
}

fn opponent_spawn(center: (i32, i32), radius: i32, wall: Wall) -> (i32, i32) {
    let (dx, dz) = cardinal_delta(wall);
    let distance = (radius - 2).max(1);
    (center.0 - dx * distance, center.1 - dz * distance)
}

fn cardinal_delta(wall: Wall) -> (i32, i32) {
    match wall {
        Wall::NegZ => (0, -1),
        Wall::PosZ => (0, 1),
        Wall::NegX => (-1, 0),
        Wall::PosX => (1, 0),
    }
}

/// Cardinal heading that points from `from` toward `to`, biased to the dominant
/// axis. Used to face the rider at the opponent.
pub fn heading_toward(from: (i32, i32), to: (i32, i32)) -> Heading {
    let dx = to.0 - from.0;
    let dz = to.1 - from.1;
    if dx.abs() >= dz.abs() {
        if dx >= 0 {
            Heading::PosX
        } else {
            Heading::NegX
        }
    } else if dz >= 0 {
        Heading::PosZ
    } else {
        Heading::NegZ
    }
}

fn step_toward(cell: (i32, i32), target: (i32, i32)) -> (i32, i32) {
    let dx = (target.0 - cell.0).signum();
    let dz = (target.1 - cell.1).signum();
    if dx.abs() >= dz.abs() {
        (cell.0 + dx, cell.1)
    } else {
        (cell.0, cell.1 + dz)
    }
}

fn is_char_boundary(bytes: &[u8], index: usize) -> bool {
    bytes
        .get(index)
        .is_none_or(|byte| byte & 0b1100_0000 != 0b1000_0000)
}

/// FNV-1a over the signals, so editing a file changes its ring.
fn content_hash(signals: &SourceSignals) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut push = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    };
    for value in [
        signals.lines as u64,
        signals.functions as u64,
        signals.comments as u64,
        signals.doc_comments as u64,
        signals.tests as u64,
        signals.todos as u64,
        signals.unsafe_count as u64,
        signals.generics as u64,
        signals.async_count as u64,
        signals.match_count as u64,
        signals.pub_count as u64,
        signals.panics as u64,
        signals.cfg_count as u64,
        signals.imports as u64,
    ] {
        push(value);
    }
    for line in signals
        .function_lines
        .iter()
        .take(config::DISC_MAX_GALLERIES)
    {
        for byte in line.as_bytes() {
            push(u64::from(*byte));
        }
    }
    hash
}

fn language_pepper(language: SourceLanguage) -> u64 {
    match language {
        SourceLanguage::Rust => 0x5255_5354,
        SourceLanguage::C => 0x0000_00c0,
        SourceLanguage::Cpp => 0x0000_cafe,
        SourceLanguage::Python => 0x5079_7468,
        SourceLanguage::Slint => 0x536c_696e,
        SourceLanguage::Lua => 0x4c75_6100,
        SourceLanguage::Shell => 0x4241_5348,
        SourceLanguage::Toml => 0x544f_4d4c,
        SourceLanguage::Json => 0x4a53_4f4e,
        SourceLanguage::Go => 0x676f_6c61,
        SourceLanguage::Ruby => 0x7275_6279,
        SourceLanguage::Yaml => 0x7961_6d6c,
        SourceLanguage::JavaScript => 0x6a73_5f5f,
        SourceLanguage::Zig => 0x7a69_675f,
        SourceLanguage::Php => 0x7068_705f,
        SourceLanguage::R => 0x0052_5f5f,
    }
}

fn cell_key(cell: (i32, i32)) -> u64 {
    (cell.0 as u32 as u64) << 32 | cell.1 as u32 as u64
}

/// SplitMix64 finalizer, used to scatter cells deterministically.
fn mix(seed: u64, value: u64) -> u64 {
    let mut z = seed ^ value.wrapping_mul(0x9e37_79b9_85eb_ca87);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::{PickupKind, SourceSignals, build_disc_arena, heading_toward, tokenize_source};
    use crate::config;
    use crate::disc::language::SourceLanguage;
    use crate::lightcycle::logic::{ArenaKind, Heading, Wall};
    use std::path::Path;

    #[test]
    fn the_close_gate_sits_at_the_end_of_a_rideable_corridor() {
        let (arena, layout) = build_disc_arena(
            Path::new("/src/gate.rs"),
            SourceLanguage::Rust,
            b"fn a() {}\n",
        );
        let portal = arena.parent_portal.expect("a ring has a close gate");
        let (dx, dz) = match portal.wall {
            Wall::NegZ => (0, -1),
            Wall::PosZ => (0, 1),
            Wall::NegX => (-1, 0),
            Wall::PosX => (1, 0),
        };
        for step in 1..=config::DISC_GATE_DEPTH {
            let cell = (
                layout.center.0 + dx * (layout.radius + step),
                layout.center.1 + dz * (layout.radius + step),
            );
            assert!(
                !arena.street_walls.contains(&cell),
                "the gate corridor is walled at {cell:?}"
            );
        }
        let outer = (
            layout.center.0 + dx * (layout.radius + config::DISC_GATE_DEPTH),
            layout.center.1 + dz * (layout.radius + config::DISC_GATE_DEPTH),
        );
        assert!(
            portal.contains(outer),
            "the gate is not at the corridor end"
        );
    }

    #[test]
    fn the_same_file_rebuilds_the_same_ring() {
        let path = Path::new("/src/main.rs");
        let source = "fn main() {\n    println!(\"hi\");\n}\n\n// TODO: more\nunsafe { x }\n";
        let (first, first_layout) = build_disc_arena(path, SourceLanguage::Rust, source.as_bytes());
        let (second, second_layout) =
            build_disc_arena(path, SourceLanguage::Rust, source.as_bytes());
        assert_eq!(first, second);
        assert_eq!(first_layout, second_layout);
    }

    #[test]
    fn different_languages_pepper_the_layout() {
        let path = Path::new("/src/thing");
        let source = "fn a() {}\nfn b() {}\n// TODO\nunsafe {}\nfn c() {}\n";
        let (_, rust) = build_disc_arena(path, SourceLanguage::Rust, source.as_bytes());
        let (_, c) = build_disc_arena(path, SourceLanguage::C, source.as_bytes());
        assert!(
            rust != c,
            "the language pepper has to reach the ring layout"
        );
    }

    #[test]
    fn editing_the_file_changes_the_fingerprint() {
        let path = Path::new("/src/lib.rs");
        let (_, first) = build_disc_arena(path, SourceLanguage::Rust, b"fn a() {}\n");
        let (_, second) = build_disc_arena(path, SourceLanguage::Rust, b"fn a() {}\nfn b() {}\n");
        assert_ne!(first.radius, second.radius);
    }

    #[test]
    fn every_spawn_and_gate_is_on_the_ring() {
        let path = Path::new("/src/combat.rs");
        let source = "fn a() {}\nfn b() {}\nunsafe { goto x; }\n// TODO fix\n#[test]\nfn t() {}\n";
        let (arena, layout) = build_disc_arena(path, SourceLanguage::Rust, source.as_bytes());
        assert_eq!(arena.kind, ArenaKind::Disc);
        for cell in [layout.player_spawn, layout.opponent_spawn] {
            let dx = cell.0 - layout.center.0;
            let dz = cell.1 - layout.center.1;
            assert!(dx * dx + dz * dz <= layout.radius * layout.radius);
            assert!(!arena.street_walls.contains(&cell));
        }
        let portal = arena.parent_portal.expect("a disc ring has a close gate");
        let gate = (
            portal.from.0.max(portal.to.0),
            portal.from.1.max(portal.to.1),
        );
        assert!(!arena.street_walls.contains(&gate));
    }

    #[test]
    fn a_stub_file_is_a_small_practice_ring() {
        let (arena, layout) =
            build_disc_arena(Path::new("/src/stub.rs"), SourceLanguage::Rust, b"");
        assert!(layout.practice);
        assert_eq!(layout.radius, config::DISC_RADIUS_MIN);
        assert!(arena.parent_portal.is_some());
    }

    #[test]
    fn hazards_pickups_and_galleries_never_share_a_cell() {
        let source = "fn a() {}\nfn b() {}\n// TODO\n// TODO\nunsafe {}\nunsafe {}\npanic!(\"x\");\n#[test]\nfn t() {}\n";
        let (arena, layout) = build_disc_arena(
            Path::new("/src/busy.rs"),
            SourceLanguage::Rust,
            source.as_bytes(),
        );
        let walls = &arena.street_walls;
        for hazard in &layout.hazards {
            assert!(!walls.contains(hazard));
            assert!(!layout.safe_pads.contains(hazard));
        }
        for spot in &layout.pickups {
            assert!(!walls.contains(&spot.cell));
            assert!(!layout.hazards.contains(&spot.cell));
        }
        for block in &layout.blocks {
            assert!(walls.contains(&block.wall));
            assert!(!walls.contains(&block.landmark));
        }
    }

    #[test]
    fn pickups_never_exceed_the_cap() {
        let mut source = String::new();
        for _ in 0..200 {
            source.push_str("// TODO\n// #[cfg(x)]\nunsafe {}\npub fn f<T>() {}\n");
        }
        let (_, layout) = build_disc_arena(
            Path::new("/src/huge.rs"),
            SourceLanguage::Rust,
            source.as_bytes(),
        );
        assert!(layout.pickups.len() <= config::DISC_MAX_PICKUPS);
        assert!(layout.hazards.len() <= config::DISC_MAX_HAZARDS);
    }

    #[test]
    fn known_constructs_map_to_their_verbs() {
        let source = "// TODO\nasync fn a() {}\nmatch x {}\nfn f<T>(t: T) {}\npub fn b() {}\npanic!(\"x\");\n/// doc\nfn c() {}\n#[cfg(test)]\nfn d() {}\n";
        let signals = tokenize_source(source, SourceLanguage::Rust);
        let kinds: Vec<_> = super::pickup_kinds(&signals);
        for expected in [
            PickupKind::Glitch,
            PickupKind::Split,
            PickupKind::Fork,
            PickupKind::Heavy,
            PickupKind::Widen,
            PickupKind::Spike,
        ] {
            assert!(kinds.contains(&expected), "missing {expected:?}");
        }
    }

    #[test]
    fn tokenizer_counts_rust_functions_and_tests() {
        let source = "fn a() {}\nfn b() {}\n#[test]\nfn t() {}\n";
        let signals = tokenize_source(source, SourceLanguage::Rust);
        assert_eq!(signals.functions, 3);
        assert_eq!(signals.tests, 1);
        assert_eq!(signals.function_lines.len(), 3);
    }

    #[test]
    fn c_control_flow_is_not_a_function() {
        for line in [
            "if (x) {",
            "for (i = 0; i < 3; i++) {",
            "return foo(1);",
            "value = call(x);",
        ] {
            assert!(
                !super::is_function_line(line, SourceLanguage::C),
                "{line} was read as a function"
            );
        }
        assert!(super::is_function_line(
            "int main(int argc, char **argv) {",
            SourceLanguage::C
        ));
        assert!(super::is_function_line("void rust_c()", SourceLanguage::C));
    }

    #[test]
    fn heading_toward_picks_the_dominant_axis() {
        assert_eq!(heading_toward((0, 0), (5, 1)), Heading::PosX);
        assert_eq!(heading_toward((0, 0), (-1, -5)), Heading::NegZ);
    }

    #[test]
    fn signals_density_and_comment_ratio_are_bounded() {
        let signals = SourceSignals {
            lines: 10,
            functions: 4,
            comments: 6,
            ..Default::default()
        };
        assert!(signals.density() > 0.0);
        assert!((signals.comment_ratio() - 0.6).abs() < 1e-6);
        assert_eq!(SourceSignals::default().density(), 0.0);
        assert_eq!(SourceSignals::default().comment_ratio(), 0.0);
    }
}
