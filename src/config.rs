use bevy::prelude::{Color, Vec3};

pub const WINDOW_TITLE: &str = "Raptor";
pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

pub const DEFAULT_CAMERA_YAW: f32 = 0.8;
pub const DEFAULT_CAMERA_PITCH: f32 = 0.6;
pub const DEFAULT_CAMERA_DISTANCE: f32 = 25.0;
pub const MIN_CAMERA_DISTANCE: f32 = 5.0;
pub const MAX_CAMERA_DISTANCE: f32 = 50.0;
pub const MIN_CAMERA_PITCH: f32 = 0.1;
pub const MAX_CAMERA_PITCH: f32 = 1.4;
pub const CAMERA_ROTATION_SPEED: f32 = 0.01;
pub const CAMERA_ZOOM_SPEED: f32 = 2.0;
pub const CAMERA_LERP_FACTOR: f32 = 0.1;

pub const GRID_SPACING: f32 = 2.5;
pub const GRID_SIZE: i32 = 20;
pub const MAX_DIRECTORY_ENTRIES: usize = 30_000;
pub const DIR_CHILD_COUNT_CAP: usize = 500;
pub const MESH_CHUNK_SIZE: usize = 512;
pub const LABEL_BUDGET: usize = 512;

pub const BLOCK_WIDTH: f32 = 2.0;
pub const BLOCK_DEPTH: f32 = 2.0;
pub const MIN_BLOCK_HEIGHT: f32 = 0.5;
pub const MAX_BLOCK_HEIGHT: f32 = 3.0;
pub const DIR_HEIGHT_MULTIPLIER: f32 = 0.5;
pub const DIR_HEIGHT_BASE: f32 = 1.0;
pub const FILE_HEIGHT_LOG_SCALE: f32 = 0.3;

pub const BACKGROUND_COLOR: Color = Color::srgb(0.02, 0.02, 0.02);
pub const GRID_COLOR: Color = Color::srgba(0.0, 1.0, 0.25, 0.15);
pub const DIR_COLOR: Color = Color::srgba(0.0, 1.0, 0.25, 0.8);
pub const FILE_COLOR: Color = Color::srgba(0.0, 0.83, 1.0, 0.8);
pub const SELECTED_COLOR: Color = Color::srgba(1.0, 0.0, 1.0, 0.9);
pub const HOVER_DIR_COLOR: Color = Color::srgba(0.3, 1.0, 0.5, 0.9);
pub const HOVER_FILE_COLOR: Color = Color::srgba(0.3, 0.9, 1.0, 0.9);
pub const SCAN_COLOR: Color = Color::srgba(0.0, 1.0, 0.25, 0.3);
pub const UI_PANEL_COLOR: Color = Color::srgba(0.0, 0.05, 0.0, 0.9);
pub const UI_BREADCRUMB_COLOR: Color = Color::srgba(0.0, 0.03, 0.0, 0.95);
pub const TEXT_PRIMARY: Color = Color::srgb(0.0, 1.0, 0.25);
pub const TEXT_SECONDARY: Color = Color::srgb(0.0, 0.83, 1.0);
pub const TEXT_HIGHLIGHT: Color = Color::srgb(1.0, 0.0, 1.0);
pub const TEXT_WARNING: Color = Color::srgba(1.0, 1.0, 0.0, 0.9);

pub const SCAN_SPEED: f32 = 20.0;
pub const SCAN_MAX_HEIGHT: f32 = 30.0;

pub const HEADER_FONT_SIZE: f32 = 20.0;
pub const INFO_FONT_SIZE: f32 = 14.0;
pub const LABEL_FONT_SIZE: f32 = 12.0;
pub const LABEL_FOCUSED_FONT_SIZE: f32 = 16.0;

pub const HEADER_HEIGHT: f32 = 50.0;
pub const BREADCRUMB_HEIGHT: f32 = 25.0;
pub const FOOTER_HEIGHT: f32 = 80.0;
pub const LABEL_MAX_LENGTH: usize = 12;

pub const VIGNETTE_ALPHA: f32 = 0.2;
pub const SCANLINE_STEP: usize = 4;
pub const SCANLINE_HEIGHT: f32 = 2.0;
pub const SCANLINE_ALPHA: f32 = 0.1;

pub const LIGHTCYCLE_CELLS_PER_SEC: f32 = 3.5;
pub const LIGHTCYCLE_FIXED_STEP: f32 = 1.0 / 60.0;
pub const LIGHTCYCLE_MAX_SUBSTEPS: usize = 4;
/// How early (in cells) the rendered path begins curving before an intersection.
pub const LIGHTCYCLE_TURN_RADIUS: f32 = 0.4;
/// How far the cycle banks into a corner, in radians.
pub const LIGHTCYCLE_LEAN_ANGLE: f32 = 0.55;
pub const LIGHTCYCLE_CRASH_FX_DURATION: f32 = 0.6;
/// Duration of the Recognizer-style directory transport. The filesystem load
/// starts at the apex so the animation remains visible even for cached folders.
pub const LIGHTCYCLE_ENTRY_FX_DURATION: f32 = 1.15;
pub const LIGHTCYCLE_ENTRY_FX_REQUEST_AT: f32 = 0.82;
pub const LIGHTCYCLE_ENTRY_BEAM_HEIGHT: f32 = 15.0;
pub const LIGHTCYCLE_ENTRY_BEAM_RADIUS: f32 = 1.45;
pub const LIGHTCYCLE_ENTRY_HALO_COUNT: usize = 7;
pub const LIGHTCYCLE_ENTRY_HALO_HEIGHT: f32 = 10.5;
// Wider than a directory tower so the rings remain visible while the bike is
// still inside the tower mesh at the beginning of the transport.
pub const LIGHTCYCLE_ENTRY_HALO_INNER_RADIUS: f32 = 1.25;
pub const LIGHTCYCLE_ENTRY_HALO_OUTER_RADIUS: f32 = 1.5;
pub const LIGHTCYCLE_ARENA_PADDING: i32 = 2;
/// Smallest arena side, in cells. Arenas are square, so a folder with one or two
/// entries still gets a plaza, streets, and room to turn around.
pub const LIGHTCYCLE_MIN_ARENA_SPAN: i32 = 13;
/// Percentage of buildable cells that seed a short architecture run.
pub const LIGHTCYCLE_CITY_STRUCTURE_SEED_CHANCE: u8 = 18;
pub const LIGHTCYCLE_SPAWN_SEARCH_RADIUS: i32 = 4096;
/// Clear cells the spawn search tries to leave straight ahead of the cycle. At
/// `LIGHTCYCLE_CELLS_PER_SEC` this is over a second and a half of runway, so
/// landing in an unfamiliar folder leaves time to read the streets and pick a
/// turn instead of reacting to whatever sits in the next cell.
pub const LIGHTCYCLE_SPAWN_RUNWAY_CELLS: i32 = 6;
/// Tower lattice spacing in cells. Each original grid row/column is multiplied
/// by this stride, leaving plazas and a two-cell development strip between
/// neighboring filesystem landmarks.
pub const LIGHTCYCLE_TOWER_STRIDE: i32 = 5;
pub const LIGHTCYCLE_TOWER_SIZE: f32 = 2.2;

/// glTF scene rendered as the player's cycle, relative to the `assets` directory.
pub const LIGHTCYCLE_MODEL_ASSET: &str = "models/light_cycle/scene.gltf";
/// Uniform scale for the cycle model. The source asset is 3.31 units long, so
/// this renders the cycle just under one grid cell long.
pub const LIGHTCYCLE_MODEL_SCALE: f32 = 0.72;
/// The model's nose already points down its local +X, which is also the axis
/// gameplay rotates onto the direction of travel, so the mesh needs no spin.
pub const LIGHTCYCLE_MODEL_YAW: f32 = 0.0;
/// Rendered height of the scaled cycle model.
pub const LIGHTCYCLE_CYCLE_HEIGHT: f32 = 1.0;
pub const LIGHTCYCLE_TRAIL_HEIGHT: f32 = 1.85;
/// Thin enough to read as a sheet of glass rather than a stack of bricks.
pub const LIGHTCYCLE_TRAIL_THICKNESS: f32 = 0.14;
/// How far behind the cycle origin the wall is born, in cells. Matches the
/// scaled model's rear axle so the sheet appears to leave the tail, not the
/// cell center.
pub const LIGHTCYCLE_TRAIL_TAIL: f32 = 0.42;
/// Distance along the ribbon, in cells, over which the wall grows from a
/// meniscus at the tail to full height.
pub const LIGHTCYCLE_TRAIL_EMANATE: f32 = 0.55;
pub const LIGHTCYCLE_TRAIL_SPAWN_HEIGHT: f32 = 0.16;
pub const LIGHTCYCLE_WALL_HEIGHT: f32 = 1.4;
pub const LIGHTCYCLE_WALL_THICKNESS: f32 = 0.2;
pub const LIGHTCYCLE_CITY_FOUNDATION_HEIGHT: f32 = 0.28;
pub const LIGHTCYCLE_CITY_STRUCTURE_SIZE: f32 = 1.9;
pub const LIGHTCYCLE_CITY_BARRIER_HEIGHT: f32 = 1.15;
pub const LIGHTCYCLE_CITY_GLASS_HEIGHT: f32 = 2.1;
pub const LIGHTCYCLE_CITY_PYLON_HEIGHT: f32 = 3.4;
pub const LIGHTCYCLE_CITY_FIN_THICKNESS: f32 = 0.22;
pub const LIGHTCYCLE_CITY_CAP_HEIGHT: f32 = 0.1;
/// Neon light-line laid where a structure or arena wall meets the ground. The
/// foundations and the floor are both nearly black, so without this skirt the
/// two surfaces merge into one shape and the base of a wall is invisible.
pub const LIGHTCYCLE_CITY_BASE_TRIM_HEIGHT: f32 = 0.07;
/// How far the skirt sticks out past the surface it outlines, in world units.
pub const LIGHTCYCLE_CITY_BASE_TRIM_OVERHANG: f32 = 0.24;
pub const LIGHTCYCLE_CITY_BEACON_LIMIT: usize = 24;
pub const LIGHTCYCLE_CITY_ROAD_RENDER_LIMIT: usize = 16_384;
pub const LIGHTCYCLE_PORTAL_HEIGHT: f32 = 2.6;
/// How many wall cells the parent gate covers. Wide enough that reaching the
/// parent directory does not need single-cell precision.
pub const LIGHTCYCLE_PORTAL_WIDTH_CELLS: i32 = 3;
/// Thickness of the gate's posts and lintel.
pub const LIGHTCYCLE_PORTAL_FRAME_THICKNESS: f32 = 0.28;
/// Radians per second of the gate frame's brightness pulse.
pub const LIGHTCYCLE_PORTAL_PULSE_SPEED: f32 = 3.2;
/// Light bars sweeping up through the gate's opening.
pub const LIGHTCYCLE_PORTAL_BAR_COUNT: usize = 4;
/// Full sweeps of the opening per second.
pub const LIGHTCYCLE_PORTAL_BAR_SPEED: f32 = 0.45;
pub const LIGHTCYCLE_PORTAL_BAR_HEIGHT: f32 = 0.18;
pub const LIGHTCYCLE_PORTAL_BAR_ALPHA: f32 = 0.5;

pub const LIGHTCYCLE_TRAIL_COLOR: Color = Color::srgba(0.55, 0.95, 1.0, 1.0);
/// Tint of light passing through the trail, slightly greener than the surface
/// so the sheet reads as thick glass rather than a cyan decal.
pub const LIGHTCYCLE_TRAIL_ATTENUATION: Color = Color::srgba(0.35, 0.9, 0.85, 1.0);
pub const LIGHTCYCLE_WALL_COLOR: Color = Color::srgba(0.0, 0.7, 0.6, 1.0);
pub const LIGHTCYCLE_CITY_FLOOR_COLOR: Color = Color::srgb(0.008, 0.012, 0.025);
pub const LIGHTCYCLE_CITY_FOUNDATION_COLOR: Color = Color::srgb(0.018, 0.025, 0.055);
pub const LIGHTCYCLE_CITY_GLASS_COLOR: Color = Color::srgba(0.08, 0.18, 0.32, 0.62);
pub const LIGHTCYCLE_CITY_CYAN: Color = Color::srgb(0.0, 0.9, 1.0);
pub const LIGHTCYCLE_CITY_BLUE: Color = Color::srgb(0.08, 0.38, 1.0);
pub const LIGHTCYCLE_CITY_MAGENTA: Color = Color::srgb(1.0, 0.03, 0.72);
pub const LIGHTCYCLE_CITY_PINK: Color = Color::srgb(1.0, 0.22, 0.48);
pub const LIGHTCYCLE_CITY_VIOLET: Color = Color::srgb(0.58, 0.16, 1.0);
pub const LIGHTCYCLE_CITY_AMBER: Color = Color::srgb(1.0, 0.56, 0.04);
pub const LIGHTCYCLE_PORTAL_COLOR: Color = Color::srgba(1.0, 0.85, 0.1, 1.0);
/// Trough of the gate frame's pulse.
pub const LIGHTCYCLE_PORTAL_DIM_COLOR: Color = Color::srgba(0.32, 0.25, 0.03, 1.0);

pub const DOCUMENT_MAX_BYTES: usize = 256 * 1024;
pub const DOCUMENT_MAX_BLOCKS: usize = 256;
pub const DOCUMENT_MAX_TEXT_CHARS: usize = 2_000;
pub const DOCUMENT_HEADING_GLYPHS: usize = 24;
pub const DOCUMENT_PARAGRAPH_GLYPHS: usize = 12;
pub const DOCUMENT_MAX_GLYPHS: usize = 512;
pub const DOCUMENT_MIN_ARENA_SPAN: i32 = 13;
pub const DOCUMENT_ROW_WIDTH_MIN: i32 = 7;
pub const DOCUMENT_ROW_WIDTH_MAX: i32 = 11;
pub const DOCUMENT_FLOOR_COLOR: Color = Color::srgb(0.86, 0.80, 0.68);
pub const DOCUMENT_RULE_COLOR: Color = Color::srgb(0.62, 0.48, 0.32);
pub const DOCUMENT_INK_COLOR: Color = Color::srgb(0.12, 0.09, 0.07);
pub const DOCUMENT_INK_EMISSIVE: Color = Color::srgb(0.35, 0.18, 0.05);
pub const DOCUMENT_MARGIN_COLOR: Color = Color::srgb(0.72, 0.22, 0.18);
pub const DOCUMENT_HEADING_COLOR: Color = Color::srgb(0.18, 0.12, 0.08);
pub const DOCUMENT_FOLIO_COLOR: Color = Color::srgb(0.42, 0.22, 0.12);
pub const DOCUMENT_FOLIO_DIM_COLOR: Color = Color::srgb(0.18, 0.10, 0.06);
pub const DOCUMENT_FOCUS_COLOR: Color = Color::srgb(0.85, 0.45, 0.12);
pub const MARKDOWN_TOWER_COLOR: Color = Color::srgb(0.92, 0.82, 0.58);

// --- Procedural music -------------------------------------------------------

/// Hard cap on simultaneously modulated per-entry voices. The voice bank is
/// always compiled at this width so switching profiles never rebuilds the graph.
pub const MUSIC_MAX_VOICES: usize = 8;
/// Master volume when music starts, before `[` / `]` adjustments.
pub const MUSIC_DEFAULT_VOLUME: f32 = 0.35;
pub const MUSIC_VOLUME_STEP: f32 = 0.1;
pub const MUSIC_MIN_VOLUME: f32 = 0.0;
pub const MUSIC_MAX_VOLUME: f32 = 1.0;

/// Calm (Explorer) base tempo range, in BPM.
pub const MUSIC_CALM_BPM_MIN: f32 = 60.0;
pub const MUSIC_CALM_BPM_MAX: f32 = 80.0;
/// Action (Lightcycle) tempo is the folder theme scaled by this factor.
pub const MUSIC_ACTION_TEMPO_MULTIPLIER: f32 = 1.6;

/// How many per-entry voices may sound at once per profile.
pub const MUSIC_CALM_VOICE_BUDGET: usize = 4;
pub const MUSIC_ACTION_VOICE_BUDGET: usize = MUSIC_MAX_VOICES;

/// Distance (world units; the grid spacing is [`GRID_SPACING`]) at which a
/// block's voice has fallen to half weight.
pub const MUSIC_CALM_PROXIMITY_RADIUS: f32 = 6.0;
pub const MUSIC_ACTION_PROXIMITY_RADIUS: f32 = 13.0;
/// Voices beyond `radius * this` are ignored entirely.
pub const MUSIC_PROXIMITY_CUTOFF_MULTIPLIER: f32 = 3.0;

/// Peak gain a single proximity voice may reach.
pub const MUSIC_CALM_GAIN_CEILING: f32 = 0.25;
pub const MUSIC_ACTION_GAIN_CEILING: f32 = 0.6;
/// One-pole time constant for voice gain / pan / filter smoothing, in seconds.
pub const MUSIC_CALM_SMOOTHING_TAU: f32 = 0.15;
pub const MUSIC_ACTION_SMOOTHING_TAU: f32 = 0.04;

/// A voice slot is released once its node's weight drops below this, and a new
/// node must exceed [`MUSIC_SLOT_ACQUIRE_THRESHOLD`] to take a free slot. The gap
/// between the two gives the assignment hysteresis, so voices do not thrash
/// between nearby blocks.
pub const MUSIC_SLOT_RELEASE_THRESHOLD: f32 = 0.12;
pub const MUSIC_SLOT_ACQUIRE_THRESHOLD: f32 = 0.25;
/// Stereo spread applied to a voice at the edge of the proximity radius.
pub const MUSIC_PAN_RANGE: f32 = 0.8;

/// Voice filter sweep: cutoff moves from the node's base cutoff up to
/// `base + weight * span` as the listener closes in.
pub const MUSIC_VOICE_CUTOFF_SPAN: f32 = 2400.0;
pub const MUSIC_VOICE_CUTOFF_MIN: f32 = 300.0;

pub const MUSIC_CALM_PAD_GAIN: f32 = 0.16;
pub const MUSIC_ACTION_BASS_GAIN: f32 = 0.22;
pub const MUSIC_ACTION_LEAD_GAIN: f32 = 0.14;

/// Action-only amplitude pumping on the bass/lead, expressed as a fraction of
/// full gain. `RATE` is pumps per beat, so tempo changes the pulse speed.
pub const MUSIC_ACTION_PUMP_DEPTH: f32 = 0.5;
pub const MUSIC_ACTION_PUMP_RATE: f32 = 2.0;

/// Seconds the graph crossfade takes when a room or profile changes. Long
/// enough to read as a blend, short enough to feel responsive while browsing.
pub const MUSIC_CROSSFADE_SECONDS: f32 = 0.7;

/// How often voice parameters are published to the audio thread, in Hz. The
/// payload is coalesced, so a faster frame loop cannot backlog the audio thread.
pub const MUSIC_PARAMS_HZ: f32 = 60.0;

/// Arpeggiator. `BEATS` is step length in beats, so tempo drives the rate:
/// Explorer gets one soft note per beat, Lightcycle eighths. `TAU` is the
/// per-note decay time constant, and `GAIN` the per-note peak.
pub const MUSIC_CALM_ARP_BEATS: f32 = 1.0;
pub const MUSIC_ACTION_ARP_BEATS: f32 = 2.0;
pub const MUSIC_CALM_ARP_TAU: f32 = 0.26;
pub const MUSIC_ACTION_ARP_TAU: f32 = 0.11;
pub const MUSIC_CALM_ARP_GAIN: f32 = 0.10;
pub const MUSIC_ACTION_ARP_GAIN: f32 = 0.17;

/// Slow filter sweep applied to the base voices, in cycles per second.
pub const MUSIC_CALM_SWEEP_RATE: f32 = 0.05;
pub const MUSIC_ACTION_SWEEP_RATE: f32 = 0.12;

// --- Disc wars --------------------------------------------------------------

/// Source arena byte cap, mirroring the document cap. Bigger files are read up
/// to this and then truncated with a status-line note.
pub const SOURCE_MAX_BYTES: usize = 256 * 1024;
/// Lines the cheap tokenizer ever looks at. The rest of the file only feeds the
/// fingerprint hash, so a huge generated source cannot stall a frame.
pub const SOURCE_MAX_LINES: usize = 4_000;

/// Smallest and largest ring radius, in cells. A stub file gets a tiny practice
/// ring; a long one gets a full coliseum.
pub const DISC_RADIUS_MIN: i32 = 8;
pub const DISC_RADIUS_MAX: i32 = 18;
/// How far the close gate corridor runs outward from the ring wall.
pub const DISC_GATE_DEPTH: i32 = 3;
pub const DISC_MAX_GALLERIES: usize = 12;
pub const DISC_MAX_HAZARDS: usize = 24;
pub const DISC_MAX_SAFE_PADS: usize = 24;
pub const DISC_MAX_PICKUPS: usize = 6;

/// Rounds won needed to take the match (best of three).
pub const DISC_WIN_SCORE: u8 = 2;
/// Pause between rounds before both fighters respawn.
pub const DISC_ROUND_DELAY: f32 = 1.6;
/// Cells a disc flies before it turns back.
pub const DISC_RANGE: i32 = 8;
/// Extra range granted by Split / Widen.
pub const DISC_RANGE_BONUS: i32 = 2;
/// Disc flight speed, in cells per second.
pub const DISC_SPEED: f32 = 9.0;
/// Opponent ground speed, in cells per second. Slower than the bike so the
/// Recognizer can be lined up and hit, but still a moving target.
pub const DISC_OPPONENT_SPEED: f32 = 1.7;
/// The opponent only bothers dodging a disc this close (Chebyshev cells). A
/// distant disc leaves it advancing instead of sliding off its own firing line.
pub const DISC_OPPONENT_DODGE_RANGE: i32 = 4;
/// Cells of slack around the opponent's body that still count as a hit for the
/// player's disc. The Recognizer moves in grid steps, so an exact-cell rule
/// makes a moving target nearly impossible to land on. Its own disc keeps an
/// exact rule, so dodging its shots still matters.
pub const DISC_PLAYER_HIT_SLACK: i32 = 1;
/// Seconds between opponent throws while it has line of sight.
pub const DISC_OPPONENT_THROW_COOLDOWN: f32 = 1.05;
/// Seconds the Recognizer winds up after acquiring line of sight, before it
/// fires. Its dais swells while charging, so the shot is telegraphed.
pub const DISC_OPPONENT_WINDUP: f32 = 0.45;
/// Quiet time at the start of a round before the opponent may line up a shot,
/// so a fresh spawn is not immediately punished.
pub const DISC_SPAWN_GRACE: f32 = 1.2;
/// Bullet time: while Shift is held in a ring, everything steps at this
/// fraction of real time, giving the rider room to line up a turn or a throw.
pub const DISC_BULLET_TIME_SCALE: f32 = 0.4;
/// Hazard cells a rider may cross in a row before the fuse burns out. Stepping
/// off the hazard resets it, so one or two are survivable.
pub const DISC_HAZARD_FUSE: i32 = 3;
/// How long a Phase pickup leaves the rider untouchable.
pub const DISC_PHASE_SECONDS: f32 = 1.2;
pub const DISC_SHIELD_MAX: u8 = 2;
/// Heavy disc speed multiplier.
pub const DISC_HEAVY_SPEED_SCALE: f32 = 0.6;

pub const DISC_FLOOR_COLOR: Color = Color::srgb(0.02, 0.03, 0.05);
/// Explorer/lightcycle tower body for a rideable source file. Amber keeps it
/// distinct from the green directory and cyan plain-file towers.
pub const SOURCE_TOWER_COLOR: Color = Color::srgb(1.0, 0.5, 0.05);
pub const DISC_RING_COLOR: Color = Color::srgb(0.05, 0.09, 0.14);
pub const DISC_HAZARD_COLOR: Color = Color::srgb(1.0, 0.18, 0.12);
pub const DISC_PICKUP_COLOR: Color = Color::srgb(0.95, 0.9, 0.3);
pub const DISC_OPPONENT_COLOR: Color = Color::srgb(1.0, 0.55, 0.08);
pub const DISC_PLAYER_DISC_COLOR: Color = Color::srgb(0.6, 0.98, 1.0);
pub const DISC_SAFE_PAD_COLOR: Color = Color::srgb(0.1, 0.5, 0.3);
/// Flat cylinder used for a thrown disc.
pub const DISC_MESH_RADIUS: f32 = 0.55;
pub const DISC_MESH_THICKNESS: f32 = 0.14;
/// Stubby cylinder body for the Recognizer opponent.
pub const RECOGNIZER_RADIUS: f32 = 0.6;
pub const RECOGNIZER_HEIGHT: f32 = 1.5;

// --- Asteroids (Python source files) ---------------------------------------
//
// A second source-file game: the cycle is parked in the middle of a small ring
// and only pivots, shooting beams at drifting rocks. See `crate::asteroids`.

/// Ring radius, in cells, for an asteroid field. Big enough that rocks have to
/// cross some ground before they reach the parked cycle, and still small enough
/// that the camera can frame the whole playfield from above.
pub const ASTEROIDS_RADIUS_CELLS: i32 = 9;
/// Pivot speed of the parked cycle, in radians per second.
pub const ASTEROIDS_TURN_RATE: f32 = 2.6;
/// Seconds between shots.
pub const ASTEROIDS_FIRE_COOLDOWN: f32 = 0.22;
/// Beam speed, in world units per second.
pub const ASTEROIDS_BEAM_SPEED: f32 = 26.0;
/// How long a beam lives before it fizzles.
pub const ASTEROIDS_BEAM_LIFE: f32 = 1.3;
/// Rendered beam length and collision thickness.
pub const ASTEROIDS_BEAM_LENGTH: f32 = 1.1;
pub const ASTEROIDS_BEAM_RADIUS: f32 = 0.18;
/// Collision radius of the parked cycle.
pub const ASTEROIDS_BIKE_RADIUS: f32 = 1.0;
/// Lives before the field is lost.
pub const ASTEROIDS_LIVES: u8 = 3;
/// Mercy window after losing a life.
pub const ASTEROIDS_INVULN: f32 = 1.8;
/// Rocks within this radius are derezzed by the respawn shockwave.
pub const ASTEROIDS_SHOCKWAVE: f32 = 7.5;
/// Rock radius per size tier, in world units. Kept well under the cycle's own
/// radius: a rock you can see around is a rock you can shoot.
pub const ASTEROIDS_ROCK_LARGE: f32 = 1.5;
pub const ASTEROIDS_ROCK_MEDIUM: f32 = 0.95;
pub const ASTEROIDS_ROCK_SMALL: f32 = 0.5;
/// Drift speed range for a fresh rock. Slow enough to line up a shot across the
/// field before it arrives.
pub const ASTEROIDS_ROCK_SPEED_MIN: f32 = 1.3;
pub const ASTEROIDS_ROCK_SPEED_MAX: f32 = 2.8;
/// Half-angle a split sends its two children away from the parent's path.
pub const ASTEROIDS_SPLIT_SPREAD: f32 = 0.7;
/// Large rocks in the opening wave.
pub const ASTEROIDS_WAVE_SIZE: usize = 4;
/// Entity pool caps; the sim never grows past these by design.
pub const ASTEROIDS_MAX_ROCKS: usize = 32;
pub const ASTEROIDS_MAX_BEAMS: usize = 16;
/// Top-down camera height as a multiple of the ring radius.
pub const ASTEROIDS_CAMERA_FIT: f32 = 2.4;
/// How far back from straight-down the field camera leans, as a fraction of its
/// height. A little lean reads as 3D without distorting the aim.
pub const ASTEROIDS_CAMERA_LEAN: f32 = 0.5;

pub const ASTEROIDS_ROCK_COLOR: Color = Color::srgb(0.44, 0.5, 0.6);
pub const ASTEROIDS_ROCK_CORE_COLOR: Color = Color::srgb(1.0, 0.66, 0.26);
pub const ASTEROIDS_BEAM_COLOR: Color = Color::srgb(0.6, 1.0, 1.0);

/// One ring identity per source language, mirroring the plan's table.
pub const DISC_RUST_ACCENT: Color = Color::srgb(0.0, 0.9, 1.0);
pub const DISC_C_ACCENT: Color = Color::srgb(1.0, 0.56, 0.07);
pub const DISC_CPP_ACCENT: Color = Color::srgb(1.0, 0.03, 0.72);
pub const DISC_PYTHON_ACCENT: Color = Color::srgb(0.2, 0.85, 0.25);

/// Per-language arpeggiator tint while a ring is open: Rust arpeggiates harder,
/// Python pumps slower. Multipliers on the folder theme, which stays the seed.
pub const MUSIC_DISC_RUST_ARP_RATE: f32 = 1.25;
pub const MUSIC_DISC_C_ARP_RATE: f32 = 1.0;
pub const MUSIC_DISC_CPP_ARP_RATE: f32 = 1.1;
pub const MUSIC_DISC_PYTHON_ARP_RATE: f32 = 0.75;
pub const MUSIC_DISC_RUST_ARP_GAIN: f32 = 1.2;
pub const MUSIC_DISC_C_ARP_GAIN: f32 = 1.0;
pub const MUSIC_DISC_CPP_ARP_GAIN: f32 = 1.1;
pub const MUSIC_DISC_PYTHON_ARP_GAIN: f32 = 0.85;

pub const LIGHTCYCLE_CAMERA_DISTANCE: f32 = 14.0;
pub const LIGHTCYCLE_CAMERA_HEIGHT: f32 = 8.0;
pub const LIGHTCYCLE_CAMERA_LOOKAHEAD: f32 = 4.0;
/// Time constant for the chase camera easing onto a new heading, in seconds.
/// Without this lag a corner looks like the world rotating around a still bike.
pub const LIGHTCYCLE_CAMERA_TURN_LAG: f32 = 0.28;
/// Pitch limits for right-drag free look, in radians. The floor keeps the
/// camera above the arena floor and the ceiling stops short of straight down.
pub const LIGHTCYCLE_CAMERA_MIN_PITCH: f32 = 0.08;
pub const LIGHTCYCLE_CAMERA_MAX_PITCH: f32 = 1.45;
/// Time constant for free look easing back behind the cycle once the right
/// mouse button is released, in seconds. Slower than the turn lag so letting go
/// reads as the camera settling rather than snapping.
pub const LIGHTCYCLE_CAMERA_LOOK_RECENTER: f32 = 0.45;

// --- Mode transition --------------------------------------------------------

/// Length of the flight between the explorer's orbit rig and the lightcycle's
/// chase rig, in seconds. It covers a climb to a satellite view of the city and
/// a zoom back down into one road, so it is longer than a plain cut.
pub const MODE_TRANSITION_DURATION: f32 = 1.6;
/// Point in the flight where the old world is torn down and the new one is
/// built, as a fraction of the duration. This is the top of the climb, and both
/// worlds are flattened into the ground plane by the time it arrives.
pub const MODE_TRANSITION_SWAP_AT: f32 = 0.45;
/// Height of the satellite view above the road it is aimed at, in world units.
pub const MODE_TRANSITION_GODS_EYE_HEIGHT: f32 = 58.0;
/// How far back down the road the satellite view sits, in world units. Tipping
/// the shot well off vertical keeps a skyline in frame, so the city sinking and
/// the next one rising read as the ground remaking itself rather than as a cut
/// between two overhead maps.
pub const MODE_TRANSITION_GODS_EYE_BACKOFF: f32 = 30.0;
/// Fraction of the flight the outgoing world spends sinking into the ground
/// before the swap, and the incoming one spends growing back out of it after.
pub const MODE_TRANSITION_DEREZ_WINDOW: f32 = 0.26;
pub const MODE_TRANSITION_REZ_WINDOW: f32 = 0.32;
/// How much the flight's altitude lags its ground track on the way down, as an
/// exponent. Above 1 the camera runs out over the road before dropping onto it,
/// and the climb applies the reciprocal, so it gains height before it travels.
pub const MODE_TRANSITION_ALTITUDE_BIAS: f32 = 1.8;
/// Fraction the field of view widens by during the dive, which exaggerates the
/// speed of the descent.
pub const MODE_TRANSITION_FOV_KICK: f32 = 0.18;
pub const MODE_TRANSITION_LIGHTCYCLE_TINT: Color = Color::srgb(0.55, 0.95, 1.0);
pub const MODE_TRANSITION_EXPLORER_TINT: Color = Color::srgb(0.25, 1.0, 0.45);
/// Height the rez wave climbs to as the new world grows, in world units. Tall
/// enough to clear the skyline, low enough to stay under the diving camera.
pub const MODE_TRANSITION_REZ_HEIGHT: f32 = 18.0;
/// Side length of the rez wave sheet, in world units. It has to cover the whole
/// frame at the swap, where it sits on the ground and carries the redraw.
pub const MODE_TRANSITION_REZ_SPAN: f32 = 140.0;
pub const MODE_TRANSITION_REZ_ALPHA: f32 = 0.38;

/// Convert a grid coordinate to a point on the ground plane.
pub fn ground_position(x: i32, z: i32) -> Vec3 {
    Vec3::new(x as f32 * GRID_SPACING, 0.0, z as f32 * GRID_SPACING)
}

/// Convert a grid coordinate to world coordinates at the block center.
pub fn world_position(x: i32, z: i32, height: f32) -> Vec3 {
    let mut position = ground_position(x, z);
    position.y = height / 2.0;
    position
}

/// World position just above a block, used for labels.
pub fn block_top_position(x: i32, z: i32, height: f32) -> Vec3 {
    let mut position = ground_position(x, z);
    position.y = height + 0.5;
    position
}
