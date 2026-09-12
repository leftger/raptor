# **RAPTOR**

### **Realtime Abstracted Path Tree Observer**

*A dangerously cool 3D filesystem explorer inspired by Jurassic Park’s iconic FNS scene.*

![Demo](./assets/demo.gif)

[It's a Unix system](https://youtu.be/dFUlAQZB9Ng?si=Xwjish_MCRif8j7A)

---

## What Is This?

**RAPTOR** is a 3D filesystem navigator built with **Rust + Bevy**, designed to make browsing your folders feel like hacking the mainframe *inside* Jurassic Park.

Remember that overly dramatic scene where a kid exclaims

> “It’s a UNIX system… I know this!”

and then flies through a cheesy 3D filesystem?

Yeah.
That.
But modern. Smooth. Glowy, because of course.

RAPTOR turns your directories and files into a neon-green cyber-grid of chunky blocks you can rotate around, scan, click on, and explore as if your PC security was being compromised by velociraptors.

## Things

* **3D Blocks** for every file and directory
* **Orbiting Camera** (right-click drag + scroll zoom)
* **Navigation**:
  `h j k l` to move, `o/Enter` to open/enter, `u/-` to go up
* **Open files** with your system's default application
* **Reveal in file manager** (`F`, works on macOS, Windows, and Linux)
* **Automatic Directory Grid Layout**
* **Clickable breadcrumbs** for jumping back through parent folders
* **Reload current directory** (`r`)
* **Selection, Hover, and Glow Effects**
* **Raycast Block Picking**
* **Dynamic Block Heights** (file size / children count)
* **Scanning Intro Line** (because retro sci-fi vibes)
* **Breadcrumb Navigation**
* **Labels Toggle** (Tab)
* **Interactive UI Panels** with live stats
* **Smooth camera tweening**
* **Cool glowing wireframes because aesthetics**
* **TRON-style Lightcycle Mode** (press `M` in the 3D view), entered by a
  satellite-style zoom out over the city and back down into the road you start on
* **Disc Wars** — ride into a `.rs` / `.cpp` file to fight a Recognizer in a ring
  generated from that file, best of three (`Space` / click to throw, `Q` to
  recall)
* **Asteroid Field** — ride into a `.c` / `.h` file to pivot a parked cycle in a
  ring and blast the drifting rocks (`A`/`D` to aim, `Space` to fire)
* **Snake** — ride into a `.py` file to chase power-ups around a ring; every one
  you eat lengthens your tail, and the exit only opens once you have them all
* **Platformer** — ride into a `.slint` file to run and jump a procedural level
  to the door at the far end
* **Brick Breaker** — ride into a `.lua` file and *be* the paddle: the bike
  slides along the bottom and rebounds the ball at the wall of bricks
* **Stealth** — ride into a `.sh` file to sneak the Tron runner past patrolling
  guards and their vision cones
* **River Surfer** — ride into a `.toml` file to hover-bike down a procedural
  river, threading boost gates to the finish without beaching on a bank or a rock
* **Galaga** — ride into a `.json` file to slide the cycle along the bottom of
  a fixed screen and shoot down the diving formation before it marches onto you
* **Pac-Man** — ride into a `.go` file to eat every dot in a maze while three
  garbage collectors chase you
* **Columns** — ride into a `.rb` file to drop and rotate gem stacks and clear
  three-in-a-row matches in a vertical well
* **Tetris** — ride into a `.yaml` / `.yml` file and clear ten lines of
  indentation blocks
* **Frogger** — ride into a `.js` file and hop the cycle across five lanes of
  moving code
* **Q*bert** — ride into a `.zig` file and hop diagonally across a cube pyramid
  to light every cube
* **Bomberman** — ride into a `.php` file, blast the crates, and reach the exit
* **Plinko** — ride into an `.r` file and drop balls through seeded pins to
  beat the score target
* **Procedural music** seeded by your folders (`N` to toggle)

## Navigation

### **Camera**

* **Right-mouse drag** → Rotate
* **Scroll wheel** → Zoom

### **Navigation**

* `h` → Left
* `j` → Down
* `k` → Up
* `l` → Right
* `o` or `Enter` → Open directory / open file with default app
* `r` → Reload current directory
* `f` → Reveal selected item in file manager
* `u` or `-` → Go to parent
* `/` → Go to root
* `Home` → Go to home directory
* `Backspace/Esc` → Back in history

### **Mouse**

* Hover → Highlight block
* Click → Select
* Click selected block again → Enter directory / open file
* Click breadcrumb path segment → Jump to that directory

### **UI**
* `.`   → Toggle hidden files
* `Tab` → Toggle labels

### **Music**
* `N`   → Toggle procedural music
* `[`   → Volume down
* `]`   → Volume up

## Lightcycle Mode (TRON-style)

Press **`M`** to toggle between the classic explorer and a lightcycle run over
the same directory grid.

* `A` / `Left`  → Queue a left turn (applied at the next cell boundary)
* `D` / `Right` → Queue a right turn
* **Right-mouse drag** → Hold to look around the cycle (springs back on release)
* `R` → Restart the run in the current directory
* `M` → Back to Explorer
* `u` / `-` / breadcrumbs → Directory jumps (the run resets when the new folder loads)
* Folders → Enter them and load the directory
* Markdown files → Enter a readable page arena
* Source files (`.rs`, `.c`, `.h`, `.cpp`) → Enter a **disc wars ring**
* Python files (`.py`, `.pyi`) → Enter an **asteroid field**: `A`/`D` pivot, `Space` fire
* In a ring: `Space` / left click → Throw your disc; `Q` → Recall it
* Other files, your own trail, and the arena walls → Crash
* Pulsing gold gate → Go to the parent directory (inactive at `/`)
* Folio gate (inside a document) → Close the file and restore the folder city

The gate is a real opening in the wall, framed by two posts and a lintel that
pulse while light bars sweep up through the gap, so it is easy to spot from
across the arena.

Every folder's gate is cut into a different wall at a different offset, so you
have to go looking for it, but it is derived from the folder's path rather than
drawn at random: the same folder always keeps the same door.

The cycle moves continuously between cell centers; turns are queued and execute
at boundaries. Explorer mouse picking, orbit camera, and labels are disabled
while riding, and are restored when you toggle back.

`M` does not cut between the two modes, it zooms between them, the way you
zoom into a satellite image. The camera pulls back off whichever rig it is on
until the whole city is below it, then zooms back in on a single road and lands
exactly on the rig the destination mode was going to use, so nothing pops when
it hands the camera over. Entering, that road is the one the cycle is about to
ride, framed running up the screen with the bike at the bottom; leaving, the
shot is centered on the directory the explorer is about to show you.

There is no flash covering the swap. The world being left behind sinks into the
ground as the camera pulls back, and the new one grows out of it as the camera
comes down, so the two are only ever exchanged while both are flat — a cyan rez
wave lies over the ground for that moment and then rises with the city coming up
under it. Detail resolves as you close in, and the lens widens through the zoom.

The run is held on its spawn cell until the camera lands, so you always start
riding from the shot you were given rather than partway down the first street.

The chase camera follows the cycle's heading, but hold the right mouse button
and drag to swing it around the bike and tilt it between a near-ground view and
a near-overhead one — useful for finding the parent gate without riding the
whole perimeter. Free look is a hold: let go and the camera eases back to the
default over-the-shoulder shot, so you cannot ride on blind. Spinning it several
turns still unwinds the short way round.

In Lightcycle mode the directory entries are re-laid out as widely spaced
"downtown" towers inside a path-seeded TRON district. Directory towers load the
next arena; file towers are solid and crash the cycle. The cycle rounds
intersections smoothly and leaves a liquid-glass wall that streams off its
tail, thickening to full height a fraction of a cell behind the bike.

Riding into a directory tower stops the cycle inside a cyan transport column.
Recognizer-style halos sweep upward around it while the bike is lifted into the
beam, and the next directory begins loading near the animation's bright apex.
The short delay is intentional: even an instant filesystem scan cannot replace
the arena before the transport is visible.

Arenas are always square and never smaller than a fixed minimum, so a folder
holding one or two entries still gives you room to turn around rather than a
shallow corridor.

Every run starts with clear road ahead. The spawn search picks a cell and a
heading with at least six empty cells in a straight line — over a second and a
half of runway — so dropping into an unfamiliar folder gives you time to read the
streets instead of reacting to whatever is in the next cell. It still starts you
near the middle of the city, and in a folder too cramped to offer that much it
faces you down the longest run it can find.

The city generator lays connected arterial roads, tower plazas, perimeter
boulevards, and alternate-route loops before it places any architecture. The
remaining blocks become a skyline of low barriers, translucent glass fins, tall
pylons, emissive caps, and animated beacons. Cyan, magenta, violet, and amber
district themes are derived from the directory path, so the same folder keeps
the same roads and identity across visits and Rust releases.

Every filesystem tower retains a clear 3×3 approach plaza, and the spawn area
and full parent gate are joined to the road network. Static geometry is merged
into material batches and architecture counts are capped, keeping large-folder
cities navigable and renderable.

Markdown files (`.md` / `.markdown`) are a fourth collision type: ride into
their cream-colored tower to pause the city run and load a **page arena**. The
navigator stays on the containing folder, so closing the document rebuilds that
already-loaded city instead of scanning the disk again. Ordinary files still
crash the cycle. Explorer still opens files with the system default app.

A document arena is a connected reading spine, not a downtown skyline: warm
page floor, ruled baselines, dark ink paragraph walls, heading arches with
bitmap-glyph landmarks, and a folio-shaped close gate instead of the gold
parent portal. Approach a heading or paragraph to highlight it and open a
folio panel with the full Unicode text. Unsupported glyphs stay readable in
that panel even when the 3D letters use a placeholder. Ride the folio gate or
press `U` / `-` to close the file and return to the directory city. `R`
restarts the page; `M` leaves Lightcycle entirely.

Large files are capped (about 256 KiB / 256 blocks) so a huge markdown dump
cannot stall a frame or spawn an unbounded mesh. Truncation is shown in the
status line.

## Disc Wars

Source files are a fifth collision type. Ride into a `.rs`, `.c`, `.h`, `.cpp`,
or `.py` tower and the city gives way to a fighting ring generated from that
file's own bytes — the same path-seeded fingerprint the cities and the music
use, so the same file always produces the same ring.

* The ring's radius follows the file's size and function count. Functions raise
  gallery alcoves, `unsafe` / `TODO` / panic sites seed red hazard tiles, tests
  become recharging safe pads, and constructs map to pickups: a `TODO` gives a
  wall-phasing glitch disc, `async` a longer throw, `match` a disc that forks at
  a wall, generics a heavy disc, `pub` extra range, `panic!` a spike, doc
  comments a shield, and `cfg` a short phase floor.
* A Recognizer opponent glides through the ring, throws along line of sight, and
  only flinches at a disc that is about to reach it. A dense file fields an
  aggressive fighter; a comment-heavy one hangs back.
* Your disc grazes: a body within one cell of its path still counts as a hit, so
  a target that steps a whole cell at a time is beatable. The Recognizer's own
  disc keeps the exact-cell rule, so dodging its shots still matters.
* `Space` / left click throws your disc. The throw auto-aims at the Recognizer,
  so you do not have to be facing it; `Q` recalls the disc. Only an outbound disc
  (or a returning one carrying Spike) derezzes the opponent.
* Hold `Shift` for **bullet time**: the whole ring slows to 40% while you line up
  a turn or a shot. The status line shows `LOCK` whenever a throw would have a
  clear shot.
* The Recognizer telegraphs. It holds still and swells for a beat before firing,
  and each round opens with a short grace period so a fresh spawn is not
  immediately punished.
* Best of three: first to two rounds wins, the gate lights up, a rising fanfare
  plays, and the status line reads `DISC: WIN`. Losing still leaves the gate open
  so you can ride out; `R` rematches.
* The folder's music keeps playing, tinted per language — Rust arpeggiates
  harder, Python pumps slower.
* The ring is capped like a document (about 256 KiB), and the tokenizer only
  walks a few thousand lines, so a generated source dump cannot stall a frame.

As with markdown pages, the navigator stays on the containing folder, so
closing the ring rebuilds the city that was already loaded. Explorer still opens
files with the system default app.

## Asteroid Field

C files are not a disc ring. A `.c` or `.h` file opens an **asteroid field**: the
cycle is parked at the middle of a small ring and never moves, but it pivots on
the spot and fires beams. Same ring geometry, close gate and restore path as
disc wars — only the game inside changes.

* `A` / `D` (or the arrows) pivot the parked cycle; `Space` / left click fires a
  beam along the facing, on a short cooldown.
* Rocks drift, bounce off the ring wall, and split when hit: large → two medium →
  two small. The smallest just derezzes. Score climbs with the tier.
* You have three lives. A rock that reaches the cycle costs one, then a
  shockwave clears the rocks around you and a mercy window opens. Lose all three
  and the field is lost.
* Clear every rock to win; a fanfare plays and the status line reads
  `ASTEROIDS: WIN`.
* Hold `Shift` for bullet time, exactly as in a disc ring — it slows the rocks
  and your pivot together.
* The camera pulls up to a single top-down shot while the rocks are live, so
  aiming never swings the view around. When the field ends it settles back down
  behind the cycle.
* The field is capped at a small ring (about nine cells) however long the file
  is, and its wave is seeded from the file's fingerprint: the same file always
  fields the same opening rocks.

Once the field is **won or lost the cycle is handed back** and drives normally,
keeping the facing you were aiming, so you can ride out through the gate to the
parent directory. `U` / `-` leaves immediately from anywhere, `M` returns to
Explorer, and `R` restarts the field.

## Snake

Python files open a **snake** run: the ordinary lightcycle grid, except the trail
is a tail of finite length and the ring's exit is sealed until you have eaten
every power-up.

* Ride with `A` / `D` (or the arrows) exactly as in a normal run — no new
  controls.
* Six power-ups are scattered over the ring, seeded from the file, and none of
  them land on top of your spawn.
* Each one you eat adds four cells to your tail. The tail is lethal, so the
  longer it gets the less room you have to turn.
* The status line tracks `SNAKE <eaten> | LEFT <rest> | TAIL <length>` and
  `EXIT: LOCKED`. Until the exit opens, the gate is a solid red bar: riding into
  it wrecks the cycle like any other wall.
* Once the last power-up is eaten the bar disappears, the status flips to
  `EXIT: OPEN`, and the gate becomes a normal way out.
* One crash ends the run — your own tail, a gallery wall or the ring wall all
  count. `R` restarts and re-scatters the same power-ups.

## Platformer

`.slint` files open a **side-scrolling platformer**: the camera swings around to
the side and the Tron runner has to reach the door at the end of the level.

* `A` / `D` (or the arrows) run; `Space` jumps. Nothing else changes.
* The level is laid out from the file's fingerprint: a run of platforms with
  gaps and steps that always stay inside what the jump can clear, so a level is
  never impossible. Longer file, longer level.
* Fall into the gap below the level and the run ends like any other crash —
  `R` rebuilds the very same level.
* Reaching the door leaves the run and returns you to the directory.

## Brick Breaker

`.lua` files open a **brick breaker**, and the bike is the paddle. The court is
upright in front of you: the wall of bricks is above, the bike is on the rail
along the bottom.

* `A` / `D` (or the arrows) slide the bike; `Space` serves the ball.
* The ball bounces off the side and ceiling walls, and where it lands on the
  bike sets its return angle, so the edges of the paddle are for aiming.
* The wall you have to worry about is the one **below** the bike: let the ball
  past and the run is over.
* Clear every brick and the run is cleared; `R` re-serves the same wall.

## Stealth

`.sh` files open a **stealth run**: a room of cover seen from just behind the
character, patrolled by guards who each sweep a cone of vision across the floor.
The camera rides high over the runner's shoulder — close enough to see the walk,
high enough to read the patrols ahead — and holds a fixed bearing, so turning a
corner never whips the view around.

* `WASD` (or the arrows) walks: hold a key and the runner keeps walking that
  way, let go and it stops where it stands. Standing still to let a patrol pass
  is therefore just not pressing anything.
* The runner turns to face the way it is walking, walks a cell at a time, and is
  eased between cells on screen so the grid underneath does not show.
* The runner is a rigged, skinned character: 19 joints, and a walk cycle
  authored as a real animation clip. Playback speed is matched to how fast it is
  actually moving, so the feet do not skate, and it rewinds to the start when
  the runner stops rather than freezing mid-stride.
* The guards share the same rig and walk on the same clip, paced by the patrol
  clock rather than by the player, so they keep walking while you stand still
  and wait.
* Press into a wall and hold it: the character turns to face the wall and
  stops, and the camera swings round to look along it, past the corner, from
  whichever side has floor — so you can see what is coming before you step out.
  Let go and it swings back. This is a view only; it does not change what the
  guards can see.
* The cones are drawn on the floor, and a translucent one is a guard looking
  the other way. Anything solid between you and a guard breaks its line of
  sight, so cover is cover.
* Standing in a cone fills the detection meter in the status line. Fill it and
  you are caught; reach the door at the far side and you are out.

## River Surfer

`.toml` files open a **river surfer** — the lightcycle becomes a hoverbike and
the arena becomes a river that runs the length of the file, swaying to a path
seeded from its bytes.

* The throttle is always open. `A` / `D` (or the arrows) steer; hold `Space` /
  `W` / `Up` to boost.
* The river banks are the only walls. Drift past one and the bike beaches;
  hit one of the rocks sticking out of the water and the run is over. `R`
  re-runs the same river.
* Glowing gates float over the water. Ride through the middle of one for a
  burst of speed.
* The finish gate spans the river at the far end. Cross it to clear the run
  and return to the directory, exactly like reaching the door in a platformer.
* The camera drops low and close to the water, Jet-Moto style, so the gates
  read as a course instead of a flyover.

## Galaga

`.json` files open a **Galaga field** — the classic fixed-screen formation
shooter, played with the lightcycle parked on the bottom edge of the ring.

* `A` / `D` (or the arrows) slide the cycle along the bottom; `Space` or left
  click fires upward.
* A grid of bugs holds formation overhead, swaying side to side and stepping
  lower. Bugs peel off one at a time and dive at you; a diver that misses
  loops back into its slot.
* Top rows are worth more points. Clear the whole formation to win the field
  and return to the directory.
* You have three lives. A bug reaching the cycle costs one (with a short mercy
  window), and the formation marching all the way down ends the run.
* `R` re-deals the same formation from the file's seed.

## The Arcade Block

Seven more file types open seven fixed-screen arcade games. Each one is seeded
by the file it belongs to, played with the shared lightcycle, and `R` always
re-deals the same board.

### Pac-Man (`.go`)

* Hold `A` / `D` / `W` / `S` to ride the corridors. The cycle turns at cell
  centers, so hold the next direction a little early.
* Eat every dot in the maze. Three garbage-collector ghosts chase you; touching
  one costs a life (with a short mercy window) and losing all three ends the
  run.

### Columns (`.rb`)

* `A` / `D` slide the falling gem stack, `W` rotates its colors, `Space` or
  left click hard-drops it.
* Three or more matching gems in a row (any direction) clear; everything above
  falls. Empty the well to win, or lose if a stack buries the rim.

### Tetris (`.yaml` / `.yml`)

* `A` / `D` slide, `W` rotates, `S` soft-drops, `Space` or left click
  hard-drops.
* Clear `TETRIS_TARGET_LINES` lines to win. Locking a piece above the top row
  ends the run.

### Frogger (`.js`)

* One keypress hops one cell: `W` / `S` forward and back, `A` / `D` side to
  side.
* Cross all five lanes of moving code to reach the far side. Obstacles wrap
  around the row; touching one spends a life and returns you to the start.

### Q*bert (`.zig`)

* Four keys hop the pyramid diagonally: `A` down-left, `D` down-right, `W`
  up-left, `S` up-right.
* Land on every cube to light it. Falling off the edge is ignored (the cycle
  stays put); the two enemies hopping between cubes cost a life on contact.
  Light the whole pyramid to win.

### Bomberman (`.php`)

* Hold `W` / `A` / `S` / `D` to walk the room. `Space` or left click plants a
  bomb under the cycle (two at a time).
* Blasts clear crates in a cross pattern — and clear the cycle too if it is
  still in the way. Destroy every crate to unlock the green exit, then reach
  it to win.

### Plinko (`.r`)

* Hold `A` / `D` to slide the cycle along the top rail, `Space` or left click
  drops a ball.
* Balls bounce through seeded pins into eight scored buckets. After the rack
  is spent, beat the seeded target to clear the board; fall short and the run
  crashes out.

## Inside the Machine

Riding a directory is meant to feel like travelling a motherboard, and the
world keeps up a running machine metaphor:

* **PCB trace trail** — the trail is a lit copper trace rather than a plain
  glass sheet.
* **Hex-dump highway** — the roads carry a sampled byte stream as emissive data
  plates, seeded from the directory's path so a folder always lays out the same.
* **Call stack** — one disc per path level, stacked in a small tower on the road
  at the arena's edge, so `/home/you/project` is visibly three frames deep. It
  stands beside the track rather than across the view.
* **Disk seek** — every directory hop plays a mechanical head-seek cue.
* **Cache hit** — revisiting a directory you have already opened grants a short
  ground-speed surge.
* **Garbage collector** — every so often the collector runs and the world
  stalls for a beat while the sweep passes.
* **Memory flood** — after a short delay a wall of spilled memory rises across
  the arena. Fall behind it and the run ends in a buffer overflow.
* **Quarantine vault** — directories named for heavy build or hidden folders
  (`node_modules`, `.git`, `target`, `vendor`, `__pycache__`, `.venv`,
  `.cache`, `build`) open as a quarantined vault: warning pylons at the corners
  and a flood that starts sooner.
* **Header telemetry** — a syscall trace ticks past the header alongside `PC`,
  `SP`, clock speed and die temperature, scaled by how much of the directory is
  loaded.
* **Scheduler race** — three rival threads start across the directory and run
  for the same gate you are. Touch one and the run ends in a race condition;
  reach the gate first for a time slice (a speed surge).
* **Git time machine** — the directories you ride form a commit log. `Z`
  rewinds to the previous commit (and its arena), `Y` fast-forwards the redo
  branch back. The status line shows `HISTORY n`, with `↻` when a redo is
  waiting.

The pause menu (`P` / `Esc`) lists every game so you can warp straight into any
of them without hunting for a file.

## Procedural Music

RAPTOR synthesizes its own soundtrack with [Glicol](https://glicol.org), driven
by the filesystem:

* Every folder's path seeds a deterministic theme — key, scale, tempo, and
  timbre family — so revisiting a folder plays the same piece.
* Each file and directory is a voice placed on the grid. As the camera (or the
  lightcycle) approaches a block, its voice swells in and opens up, then fades
  as you move away.
* Explorer mode plays a calm, ambient interpretation. Lightcycle mode reworks
  the same theme into a faster, driving arrangement.

Music starts on and can be toggled with `N`; adjust the volume with `[` and `]`.
Press `M` while music is playing to hear the calm track gear up into action.

## Why Jurassic Park?

Because *FNS* from Jurassic Park is legendary.
It’s campy. It’s 90s CGI nonsense. It’s a cultural artifact.

And I thought:

> “What if I recreate that…
> but actually make it FUN and SMOOTH and more 60 FPS…
> and with keybinds?”

And boom — **RAPTOR** hatched.

No dinosaurs were harmed in the making of this filesystem explorer.
(Except maybe your GPU when opening a directory with 30k files.)

## Running

For the fastest edit/build cycle, use Bevy's dynamic library:

```bash
cargo run --features dev
```

For normal optimized use:

```bash
cargo run --release
```

RAPTOR will open in all its neon glory.

Distribution builds keep the slower size-focused LTO settings:

```bash
cargo build --profile dist
```

### Command-line options

```bash
# Start in a specific directory
cargo run --release -- /path/to/folder

# Show hidden files, hide labels, and hide the FPS counter
cargo run --release -- --hidden --no-labels --no-fps

# Start with music off, or at half volume
cargo run --release -- --no-music
cargo run --release -- --music-volume 0.5

# See all options
cargo run --release -- --help
```

## Performance

The frame is GPU-bound, so the settings that matter are raster settings. All of
them are runtime flags, and `--bench` measures them on your own machine.

```bash
# Print frame-time statistics for 12 seconds, then exit
cargo run --release -- --bench

# A settled view, for comparing two settings fairly
cargo run --release -- --bench --bench-static

# The preset for weak integrated graphics
cargo run --release -- --fast

# Individual knobs
cargo run --release -- --msaa 1 --no-bloom --window 960x540
```

Measured on an Intel Iris 6100 at 1280x720, riding a 3,000-entry directory:

| preset | frame time | frame rate |
| --- | --- | --- |
| Bevy defaults (4x MSAA + bloom) | 59.0 ms | 17 fps |
| **current defaults** (no MSAA + bloom) | **36.1 ms** | **28 fps** |
| `--fast` (no MSAA, no bloom) | 23.1 ms | 43 fps |

What each knob is worth, measured the same way:

* **Multisampling** — 4x (Bevy's default) costs about 20 ms a frame; 2x measured
  no cheaper than 4x, so the choice is really on or off. Now off by default.
* **Bloom** — about 15 ms, because it forces the whole HDR pipeline. Trimming
  the blur chain did not help, so the choice is on or off; on by default for the
  glow, off in `--fast`.
* **Scanlines** — about 1 ms, so they stay.
* **Window size** — fill-rate bound: 960x540 is worth about 13 ms over 1280x720.

Alongside those, the per-frame work that scaled with directory size was cut: the
label pass used to project every entry in the directory (up to 30,000) and sort
them all to place 512 labels, and it now culls and pools the nearest blocks
first. Visibility and HUD writes are change-guarded, so idle frames stop marking
the whole scene dirty.

## Dependencies

RAPTOR is built using **Bevy 0.19**, a modern Rust game engine that works on Linux, macOS and Windows.

Bevy repo (installation notes & troubleshooting):
https://github.com/bevyengine/bevy

The filesystem, CLI, and OS-integration modules remain plain Rust so they can be unit-tested without a GPU.

Procedural music is synthesized by [Glicol](https://glicol.org) and played
through `cpal`, so RAPTOR does not enable Bevy's audio backend. On Linux you need
the ALSA development headers to build the audio output:

```bash
sudo apt install libasound2-dev pkg-config
```

If no output device is available, RAPTOR runs silently instead of failing.

Directory scans run in the background. RAPTOR displays at most 30,000 entries from one
directory and caps eager child counts at 500 to keep unusually large trees responsive.
Capped counts are marked with `+`. Symbolic links are shown and can be opened, but linked
directories are not traversed just to calculate block height.

## Credits

The lightcycle model in `assets/models/light_cycle` is
["Light Cycle - Tron (1982)"](https://sketchfab.com/3d-models/light-cycle-tron-1982-54fedda920094ef09d87a17d42b282af)
by [arabinowitz](https://sketchfab.com/arabinowitz), used under the
[Sketchfab Standard license](https://sketchfab.com/licenses).

The runner in `assets/models/tron_character` is
["Tron Male Character"](https://sketchfab.com/3d-models/tron-male-character-b7b2dd24bf6e495e9a729d7d271c52db)
by [dehariyalokesh1998](https://sketchfab.com/dehariyalokesh1998), used under
[CC-BY-4.0](http://creativecommons.org/licenses/by/4.0/). It has been modified:
rigged with a 19-joint skeleton, skinned, and given a walk cycle, all scripted in
Blender. Those modifications are released under the same licence.

## Future Ideas (aka InGen Phase 2)

* File previews
* Drag-and-drop
* Jurassic-Park-style *“Access Denied”* red windows
* Terminal overlay
* Dinosaur noises when selecting something
* VR version?? (life… finds a way)
