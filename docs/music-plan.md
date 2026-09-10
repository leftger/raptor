# RAPTOR — Procedural, proximity-reactive music plan

Status: M0–M6 implemented, M7 pending
Owner: —
Last updated: 2026-09-10

## 1. Concept

Live, procedurally generated music driven by the filesystem, rendered in real
time by [Glicol](../../music-stuff/glicol). Two layers:

- **Base score (deterministic, per directory).** The current folder's path feeds
  the existing `stable_path_seed` (`src/lightcycle/logic.rs`) to pick root note,
  scale, tempo, timbre family, and reverb size. The same seed already drives the
  neon city, so a folder's sound and its district stay linked.
- **Proximity voices (spatial, per entry).** Each file/directory gets a motif
  derived from its own path, size, and type. As the listener approaches that
  block, its voice fades up (gain, pan, filter cutoff). The listener is the
  orbit-camera target in Explorer mode and `run.sim.cell` in Lightcycle mode.

The filesystem is therefore both the score's seed and the spatial arrangement of
the instruments.

## 2. Mode-responsive intensity (calm <-> action)

A third axis, selected from the `InteractionMode` resource:

| Axis | Input | Controls |
| --- | --- | --- |
| Path seed | `stable_path_seed(current_path)` | Root, scale, timbre family (stable per folder in both modes) |
| Proximity | `orbit.target` / `sim.cell` | Which per-entry voices are audible |
| Mode intensity | `InteractionMode` | Tempo, texture, density, proximity aggressiveness |

Root, scale, and family stay tied to the folder, so a directory is recognizable
in both modes; it simply goes from ambient-drone to full chase-scene.

### ModeProfile

| | Calm (Explorer) | Action (Lightcycle) |
| --- | --- | --- |
| Tempo | theme base (60–80 BPM), often half-time | base × 1.6 (~110–160 BPM) |
| Rhythm | none / soft pulse | kick `bd`, snare `sn`, hat `hh` pattern |
| Pad/lead | sustained pads, slow attack, wide reverb | driving bass + arpeggio, short/punchy reverb |
| Voice budget K | 4 | 8 |
| Proximity radius `R` | small | ~2× larger |
| Gain ceiling | ~0.25 | ~0.6 |
| Smoothing tau | ~150 ms (lazy) | ~40 ms (snappy) |
| Register | low/mid | wider, brighter |
| Accents | none | pluck/impact when entering a node's radius |

The base graph is structurally different per mode (the rhythm track only exists
in Action). Mode switches reuse the safe transition planned for directory
changes: fade master to 0, `update_with_code`, fade back (~150–250 ms). Because
both modes share the seed, the transition reads as the same piece gearing up.

## 3. How the `music-stuff` crates fit

| Crate | Role | Why |
| --- | --- | --- |
| **glicol** (`rs/main`, 0.14.0-dev) | The synthesis engine. `Engine<N>::update_with_code` compiles the base graph; `send_msg("chain,pos,param,value;")` modulates params every frame without recompiling; `next_block` renders a block. | Chosen: live, block-based, string-addressable params. |
| **fundsp** | Not used initially. Fallback for a hand-built voice if Glicol's node set limits us. | Overkill for v1. |
| **tunes** | Not used initially. Only relevant if we later want theory, instruments, or WAV/MIDI export. | Large dependency tree. |

Glicol 0.14.0-dev is unpublished. Use `path = "../music-stuff/glicol/rs/main"`
during development, then pin to a git rev (local checkout is `0317db2`) once
stable. Its internal `glicol_parser` / `glicol_synth` path deps resolve
automatically.

## 4. Architecture

New module `src/music/` is audio-free and unit-testable, mirroring how
`filesystem/`, `config.rs`, and `lightcycle/logic.rs` are testable without a GPU.
A Bevy plugin wires it in, and the audio bridge is isolated in one file.

```text
src/music/
  mod.rs        # public types: MusicTheme, ModeProfile, MusicState
  theme.rs      # path -> MusicTheme; scale/root/tempo/family; ModeProfile selection
  score.rs      # MusicTheme + nodes -> Glicol code strings (base + voice chains)
  proximity.rs  # listener + nodes -> Vec<VoiceTarget> (weights, slot assignment)
  engine.rs     # audio thread: owns Engine<N>, cpal output, ring buffers
src/plugins/music.rs   # Bevy systems wiring the above
```

Data flow:

```text
PathBuf (NavigatorResource.current_path)
   +-> stable_path_seed -> MusicTheme -> base code --------+
                                                           +-> engine.rs (control msgs)
DirectoryLoaded.nodes -> proximity(listener, profile) -----+
                                    (orbit.target / sim.cell)
                                                           |
                                                     audio thread: Engine<N>
                                                           |  next_block
                                                     SPSC ring -> cpal callback -> speakers
```

- **Control/audio separation.** A dedicated thread owns `Engine` and renders
  continuously into a lock-free ring buffer; the cpal callback only drains that
  ring. This sidesteps any `Engine: !Send` concern and keeps the realtime
  callback away from the graph. Parameters flow to the audio thread through a
  second SPSC ring of small numeric snapshots.
- **Struct changes are rare.** The base graph recompiles only on
  directory/theme/mode change, behind a master-gain fade. Everything continuous
  uses `send_msg`.
- **Device-absent is graceful.** If cpal finds no output device, log a
  `UiNotice` and continue silently rather than failing.

## 5. Music model

### Base graph (per directory and mode, compiled on change)

```text
Engine::set_bpm(bpm); Engine::set_seed(seed as usize);
p0: saw 110 >> lpf 800 >> mul 0.12 >> pan 0.35
p1: saw 165 >> lpf 900 >> mul 0.10 >> pan 0.65
d0: bd 1 >> mul 0.35                  // Action profile only
o:  p0 >> add p1 >> add d0 >> reverb <size> ...
```

Exact node arities confirmed in the M0 spike (`glicol.pest`).

### Voice chains (fixed budget K, never recompiled)

```text
v0: saw <freq> >> lpf <cutoff> >> mul <gain> >> pan <pan>
v1: ...
```

Per frame, at ~30 Hz, the control side emits:

```text
v0,0,0,220.0;v0,1,0,900.0;v0,2,0,0.37;v0,3,0,0.20;
```

Positions are chain slots (`saw`=0, `lpf`=1, `mul`=2, `pan`=3).

### Per-node voice derivation

- `node_seed = stable_path_seed(&node.path)`, precomputed per load, not per frame.
- Pitch: `node_seed` mapped into the theme scale; octave from higher bits.
- Directories -> sustained/drone voices; files -> plucked/bell voices.
- Extension groups -> waveform families (source/text -> saw, images/media ->
  sine bell, `.md` -> triangle), so approaching a file is recognizable.
- Level scales with `size` / `children_count`, bounded, so big things are louder.

### Proximity kernel

- Listener: Explorer -> `OrbitCameraResource.target` (already chases the selected
  block, `selection.rs`); Lightcycle -> `config::ground_position(sim.cell)`.
- `d` = XZ distance from listener to `config::ground_position(node.grid_pos)`.
- `w = R^2 / (R^2 + d^2)`, hard cutoff past `3R`.
- **Slot hysteresis:** stable `node_index -> voice_slot` map; reassign only past a
  high/low-water mark. Prevents thrash and clicks.
- **One-pole smoothing** on gain/pan/cutoff (~80 ms base, per-profile tau) before
  `send_msg`.

## 6. RAPTOR integration points

| File | Change |
| --- | --- |
| `src/music/*` | New pure logic + audio bridge |
| `src/plugins/music.rs` | New `MusicPlugin`: init engine, react to `DirectoryLoaded` and `InteractionMode`, run proximity + param push, handle keys |
| `src/plugins/mod.rs` | Register `music` module and `MusicPlugin` |
| `src/state.rs` | Add `MusicState` / `MusicSettings` (enabled, volume, theme, profile, active slots, device status) |
| `src/config.rs` | `MUSIC_*` constants incl. `MUSIC_CALM_*` and `MUSIC_ACTION_*` |
| `src/cli.rs` | `--no-music`, `--music-volume <0..1>` (music defaults on) |
| `src/main.rs` | Insert music resource, pass CLI options |
| `src/plugins/ui.rs` | Status suffix, e.g. `MUSIC: ON 72bpm CALM | near: 2` |
| `README.md` | Document keys/flags and the mechanic |

### Keys

`M` is taken by Lightcycle (`lightcycle.rs`). Music uses:

- `N` — toggle music (handled by `MusicPlugin` in both modes)
- `[` / `]` — volume down / up

All three are currently unbound. A small `read_music_keys` system in
`MusicPlugin` runs in both modes (not gated by `in_explorer_mode`, unlike the
existing `Command` handler). `Command` may be extended later for consistency.

## 7. Dependencies and platform

```toml
[dependencies]
glicol = { git = "https://github.com/chaosprint/glicol.git", rev = "0317db2e3f157b911bfc28c72ad5db90db963f24" }
cpal = "0.18"
crossbeam-channel = "0.5"   # commands to the audio thread + rendered sample queue
```

- Do **not** enable `bevy_audio`; driving cpal directly keeps the Bevy feature
  set at `["3d", "ui"]`.
- Linux needs ALSA headers (`libasound2-dev`) and `pkg-config`; documented in the
  README. (Verified present on this machine.)
- Glicol's `use-samples` / `use-meta` features arrive via its internal manifest;
  the synth-only graph needs no sample assets.

### Glicol pin

Glicol 0.14.0-dev is unpublished, so the dependency is pinned to the exact
revision `0317db2e3f157b911bfc28c72ad5db90db963f24`. This resolves without the
local `music-stuff` checkout, so CI and raptor-only clones build normally.
`Cargo.lock` records the git source for `glicol`, `glicol_parser`, and
`glicol_synth`.

The voice bank relies on `Engine::send_msg` keying reference chains by their
leading `~` (e.g. `~v0,2,0,0.4`). Re-verify that when bumping the pin: compile a
graph and confirm `~v0` addresses resolve. Vendoring the three glicol crates is
the alternative if a git dependency is ever undesirable.


## 8. Milestones

Status: M0–M6 implemented; M7 pending.

1. **M0 — Audio spike (no Bevy wiring).** DONE. Glicol renders stereo blocks and
   the graph compiles for both profiles.
2. **M1 — Pure music logic.** DONE. `theme.rs`, `score.rs`, `proximity.rs` with
   unit tests covering determinism, profile selection, falloff, hysteresis, and
   Glicol compilation.
3. **M2 — Engine plumbing.** DONE. Dedicated audio thread owns `Engine` + the cpal
   stream, drains commands, fades graph swaps, and reports a graceful
   no-device status.
4. **M3 — Path-seeded base track.** DONE. `DirectoryLoaded` rebuilds the theme and
   entry list; the audio thread fades the swap. (Audio-verified as far as this
   headless machine allows; needs a listen on real hardware.)
5. **M4 — Explorer proximity (Calm).** DONE. `orbit.target` drives voice gains,
   pan, and cutoff through the smoothed mixer.
6. **M5 — Lightcycle proximity (Action).** DONE. The listener follows
   `run.sim.cell`, and `InteractionMode` changes rebuild the action profile.
7. **M6 — UX.** DONE. Default-on, `N` toggle, `[`/`]` volume, status line, CLI
   flags, README.
8. **M7 — Polish.** PENDING. Action flourishes (speed -> filter/arp rate, turn
   accents), crash/entry/portal one-shots, per-frame allocation and perf guards.

## 9. Risks and mitigations

- **`Engine` thread-safety / `next_block` channel mapping** — resolved in M0
  before any Bevy wiring.
- **Realtime allocation in `send_msg` / `update_with_code`** — send param
  snapshots from the control thread; recompile only during a master-gain fade;
  accept small allocs for v1, revisit if audible.
- **Proximity thrash / zipper noise** — high/low-water hysteresis + one-pole
  smoothing.
- **Proximity cost on huge folders (30k entries)** — evaluate at 30 Hz with cheap
  math, keep only top-K.
- **Glicol API churn (0.14.0-dev)** — pin a rev and isolate all Glicol calls in
  `music/engine.rs`.
- **Default-on audio UX** — default on, `--no-music` escape hatch, persistent
  on-screen indicator.

## 10. Testing

- Unit (no GPU/audio): seed determinism, scale/pitch mapping, score-string
  snapshots, proximity falloff monotonicity, slot-hysteresis stability, smoothing
  convergence, `ModeProfile` selection.
- Integration: no-audio-device path leaves the app running.
- Manual matrix: several folders in both modes, toggle, volume, empty folder,
  huge folder, symlinks.
- `cargo test` and `cargo clippy` in CI, as usual.

## 11. Non-goals (for now)

- Exporting WAV/MIDI, sample-file playback, and `tunes`-style theory-driven
  composition.
- Per-pixel audio occlusion/raycast; proximity is a distance kernel on grid
  positions.
