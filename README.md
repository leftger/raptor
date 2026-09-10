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
* **TRON-style Lightcycle Mode** (press `M` in the 3D view)
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

## Future Ideas (aka InGen Phase 2)

* File previews
* Drag-and-drop
* Jurassic-Park-style *“Access Denied”* red windows
* Terminal overlay
* Dinosaur noises when selecting something
* VR version?? (life… finds a way)
