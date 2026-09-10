use crate::config;
use crate::document::parse::{DocBlock, DocBlockKind, ParseLimits, ParseOutcome};
use crate::lightcycle::logic::{Arena, ArenaKind, GatePlacement};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedBlock {
    pub kind: DocBlockKind,
    pub text: String,
    pub preview: String,
    pub spine: Vec<(i32, i32)>,
    pub walls: Vec<(i32, i32)>,
    pub landmark: (i32, i32),
    pub along_x: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLayout {
    pub blocks: Vec<PlacedBlock>,
    pub spine: BTreeSet<(i32, i32)>,
    pub truncated: bool,
    pub lossy_utf8: bool,
}

impl DocumentLayout {
    pub fn focused_block(&self, cell: (i32, i32)) -> Option<usize> {
        self.blocks
            .iter()
            .enumerate()
            .filter_map(|(index, block)| {
                block
                    .spine
                    .iter()
                    .map(|spine| ((spine.0 - cell.0).abs() + (spine.1 - cell.1).abs(), index))
                    .min()
            })
            .min()
            .map(|(_, index)| index)
    }

    pub fn current_heading(&self, cell: (i32, i32)) -> Option<&str> {
        self.heading_for_focus(self.focused_block(cell))
    }

    pub fn heading_for_focus(&self, focused: Option<usize>) -> Option<&str> {
        let focused = focused?;
        self.blocks
            .get(..=focused)?
            .iter()
            .rev()
            .find(|block| matches!(block.kind, DocBlockKind::Heading(_)))
            .map(|block| block.text.as_str())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_document_arena(path: &Path, source: &str) -> (Arena, DocumentLayout) {
    build_document_arena_from_parse(
        path,
        crate::document::parse::parse_markdown_document(source, ParseLimits::default()),
    )
}

pub fn build_document_arena_from_parse(
    path: &Path,
    parsed: ParseOutcome,
) -> (Arena, DocumentLayout) {
    let seed =
        crate::lightcycle::logic::stable_path_seed(path) ^ content_fingerprint(&parsed.blocks);
    let row_width = row_width_for_seed(seed);
    let mut cursor = SpineCursor::new(row_width);
    let mut blocks = Vec::new();
    let mut spine = BTreeSet::new();
    let mut walls = BTreeSet::new();

    for block in parsed.blocks {
        let length = match block.kind {
            DocBlockKind::Heading(_) => 3,
            DocBlockKind::Paragraph => (2 + block.preview.chars().count() as i32 / 4).clamp(2, 6),
        };
        let mut span = Vec::new();
        for _ in 0..length {
            let cell = cursor.next_cell();
            span.push(cell);
            spine.insert(cell);
            if cursor.at_row_end() {
                spine.extend(cursor.plaza_cells());
            }
        }
        let along_x = span
            .first()
            .is_some_and(|first| span.last().is_some_and(|last| first.1 == last.1));
        let landmark = span[span.len() / 2];
        let mut block_walls = Vec::new();
        if matches!(block.kind, DocBlockKind::Paragraph) {
            for &cell in &span {
                let wall = if along_x {
                    (cell.0, cell.1 - 1)
                } else {
                    (cell.0 - 1, cell.1)
                };
                if !spine.contains(&wall) {
                    block_walls.push(wall);
                    walls.insert(wall);
                }
            }
        }
        blocks.push(PlacedBlock {
            kind: block.kind,
            text: block.text,
            preview: block.preview,
            spine: span,
            walls: block_walls,
            landmark,
            along_x,
        });
    }

    let occupied: HashSet<_> = walls.iter().copied().chain(spine.iter().copied()).collect();
    let mut gate_path = path.to_path_buf();
    gate_path.as_mut_os_string().push("\0raptor:close");
    let mut arena = Arena::from_nodes(
        occupied.iter().copied(),
        Some(GatePlacement::for_path(
            &gate_path,
            config::LIGHTCYCLE_PORTAL_WIDTH_CELLS,
        )),
        config::LIGHTCYCLE_ARENA_PADDING,
        config::DOCUMENT_MIN_ARENA_SPAN,
    );
    arena.kind = ArenaKind::Document;
    let approaches = arena.parent_gate_approaches();
    if let Some(&gate) = approaches.get(approaches.len() / 2) {
        for cell in route_to_spine(&arena, gate, &spine, &walls) {
            spine.insert(cell);
            walls.remove(&cell);
        }
    }
    for cell in approaches {
        spine.insert(cell);
        walls.remove(&cell);
    }
    arena.roads = spine
        .iter()
        .copied()
        .filter(|cell| arena.contains(*cell))
        .collect();
    arena.street_walls = walls
        .into_iter()
        .filter(|cell| arena.contains(*cell) && !arena.roads.contains(cell))
        .collect();

    (
        arena,
        DocumentLayout {
            blocks,
            spine,
            truncated: parsed.truncated,
            lossy_utf8: parsed.lossy_utf8,
        },
    )
}

fn content_fingerprint(blocks: &[DocBlock]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for block in blocks {
        hash ^= match block.kind {
            DocBlockKind::Heading(level) => u64::from(level),
            DocBlockKind::Paragraph => 7,
        };
        hash = hash.wrapping_mul(0x100000001b3);
        for byte in block.text.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn route_to_spine(
    arena: &Arena,
    start: (i32, i32),
    spine: &BTreeSet<(i32, i32)>,
    walls: &BTreeSet<(i32, i32)>,
) -> Vec<(i32, i32)> {
    if spine.contains(&start) {
        return vec![start];
    }
    let area = ((arena.max.0 - arena.min.0 + 1) as usize)
        .saturating_mul((arena.max.1 - arena.min.1 + 1) as usize)
        .saturating_add(8);
    let mut queue = VecDeque::from([start]);
    let mut previous = HashMap::new();
    let mut found = None;
    while let Some(cell) = queue.pop_front() {
        if spine.contains(&cell) {
            found = Some(cell);
            break;
        }
        if previous.len() >= area {
            break;
        }
        for delta in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = (cell.0 + delta.0, cell.1 + delta.1);
            if !arena.contains(next) || walls.contains(&next) || next == start {
                continue;
            }
            if previous.contains_key(&next) {
                continue;
            }
            previous.insert(next, cell);
            queue.push_back(next);
        }
    }
    let Some(mut cursor) = found else {
        return vec![start];
    };
    let mut path = vec![cursor];
    let mut seen = HashSet::from([cursor]);
    while cursor != start {
        let Some(&parent) = previous.get(&cursor) else {
            path.push(start);
            break;
        };
        if !seen.insert(parent) {
            break;
        }
        cursor = parent;
        path.push(cursor);
    }
    path
}

fn row_width_for_seed(seed: u64) -> i32 {
    let span = config::DOCUMENT_ROW_WIDTH_MAX - config::DOCUMENT_ROW_WIDTH_MIN + 1;
    config::DOCUMENT_ROW_WIDTH_MIN + (seed % span as u64) as i32
}

struct SpineCursor {
    row_width: i32,
    index: usize,
}

impl SpineCursor {
    fn new(row_width: i32) -> Self {
        Self {
            row_width,
            index: 0,
        }
    }

    fn next_cell(&mut self) -> (i32, i32) {
        let cell = serpentine_cell(self.index, self.row_width);
        self.index += 1;
        cell
    }

    fn at_row_end(&self) -> bool {
        self.index > 0 && self.index as i32 % self.row_width == 0
    }

    fn plaza_cells(&self) -> [(i32, i32); 2] {
        let last = serpentine_cell(self.index.saturating_sub(1), self.row_width);
        let row = last.1;
        [(last.0, row + 1), (last.0, row)]
    }
}

fn serpentine_cell(index: usize, row_width: i32) -> (i32, i32) {
    let width = row_width.max(1) as usize;
    let row = (index / width) as i32;
    let column = (index % width) as i32;
    let x = if row % 2 == 0 {
        column
    } else {
        row_width - 1 - column
    };
    (x, row * 2)
}

#[cfg(test)]
mod tests {
    use super::{build_document_arena, serpentine_cell};
    use crate::document::parse::DocBlockKind;
    use crate::lightcycle::logic::{ArenaKind, LightcycleSim};
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn serpentine_alternates_row_direction() {
        assert_eq!(serpentine_cell(0, 3), (0, 0));
        assert_eq!(serpentine_cell(2, 3), (2, 0));
        assert_eq!(serpentine_cell(3, 3), (2, 2));
        assert_eq!(serpentine_cell(5, 3), (0, 2));
    }

    #[test]
    fn same_path_and_content_rebuild_the_same_page() {
        let path = Path::new("/docs/readme.md");
        let source = "# Title\n\nHello world.\n";
        let (first, first_layout) = build_document_arena(path, source);
        let (second, second_layout) = build_document_arena(path, source);
        assert_eq!(first, second);
        assert_eq!(first_layout, second_layout);
    }

    #[test]
    fn editing_content_changes_the_layout() {
        let path = Path::new("/docs/readme.md");
        let (_, first) = build_document_arena(path, "# A\n\nOne.\n");
        let (_, second) = build_document_arena(path, "# A\n\nOne.\n\nTwo.\n");
        assert_ne!(first.blocks, second.blocks);
    }

    #[test]
    fn spine_is_connected_and_walls_stay_off_it() {
        let (arena, layout) = build_document_arena(
            Path::new("/docs/page.md"),
            "# Heading\n\nA longer paragraph of words.\n\n## Next\n\nMore text here.\n",
        );
        assert_eq!(arena.kind, ArenaKind::Document);
        assert!(arena.parent_portal.is_some());
        let start = *layout.spine.iter().next().unwrap();
        let mut reached = HashSet::from([start]);
        let mut queue = vec![start];
        while let Some(cell) = queue.pop() {
            for delta in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = (cell.0 + delta.0, cell.1 + delta.1);
                if layout.spine.contains(&next) && reached.insert(next) {
                    queue.push(next);
                }
            }
        }
        assert_eq!(reached.len(), layout.spine.len());
        assert!(layout.spine.is_disjoint(&arena.street_walls));
        assert!(
            layout
                .blocks
                .iter()
                .any(|block| matches!(block.kind, DocBlockKind::Heading(_)))
        );
        assert!(
            layout
                .blocks
                .iter()
                .any(|block| matches!(block.kind, DocBlockKind::Paragraph))
        );
    }

    #[test]
    fn close_portal_closes_the_document_instead_of_going_to_parent() {
        let (arena, _) = build_document_arena(Path::new("/docs/page.md"), "# Hi\n\nBody.\n");
        let portal = arena.parent_portal.unwrap();
        let sim = LightcycleSim::start(arena.center(), crate::lightcycle::logic::Heading::PosX);
        let content = crate::lightcycle::logic::classify_next_content(
            portal.from,
            &arena,
            &sim,
            &std::collections::HashMap::new(),
            |_| false,
            |_| false,
        );
        assert_eq!(content, crate::lightcycle::logic::CellContent::ClosePortal);
        let approach = arena.parent_gate_approaches()[arena.parent_gate_approaches().len() / 2];
        let heading = match portal.wall {
            crate::lightcycle::logic::Wall::NegZ => crate::lightcycle::logic::Heading::NegZ,
            crate::lightcycle::logic::Wall::PosZ => crate::lightcycle::logic::Heading::PosZ,
            crate::lightcycle::logic::Wall::NegX => crate::lightcycle::logic::Heading::NegX,
            crate::lightcycle::logic::Wall::PosX => crate::lightcycle::logic::Heading::PosX,
        };
        let mut sim = LightcycleSim::start(approach, heading);
        let outcome = sim.advance(1.0, |cell, sim| {
            crate::lightcycle::logic::classify_next_content(
                cell,
                &arena,
                sim,
                &std::collections::HashMap::new(),
                |_| false,
                |_| false,
            )
        });
        assert_eq!(
            outcome,
            crate::lightcycle::logic::StepOutcome::CloseDocument
        );
    }

    #[test]
    fn empty_document_still_has_a_close_gate_and_spawn() {
        let (arena, layout) = build_document_arena(Path::new("/empty.md"), "");
        assert_eq!(layout.blocks.len(), 1);
        assert!(arena.parent_portal.is_some());
        let spawn = arena
            .nearest_empty_cell(
                |cell| arena.street_walls.contains(&cell),
                crate::config::LIGHTCYCLE_SPAWN_SEARCH_RADIUS,
            )
            .unwrap();
        let sim = LightcycleSim::start(spawn, crate::lightcycle::logic::Heading::PosX);
        assert_eq!(sim.cell, spawn);
    }
}
