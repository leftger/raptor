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

# See all options
cargo run --release -- --help
```

## Dependencies

RAPTOR is built using **Bevy 0.19**, a modern Rust game engine that works on Linux, macOS and Windows.

Bevy repo (installation notes & troubleshooting):
https://github.com/bevyengine/bevy

The filesystem, CLI, and OS-integration modules remain plain Rust so they can be unit-tested without a GPU.

Directory scans run in the background. RAPTOR displays at most 30,000 entries from one
directory and caps eager child counts at 500 to keep unusually large trees responsive.
Capped counts are marked with `+`. Symbolic links are shown and can be opened, but linked
directories are not traversed just to calculate block height.

## Future Ideas (aka InGen Phase 2)

* File previews
* Drag-and-drop
* Jurassic-Park-style *“Access Denied”* red windows
* Terminal overlay
* Dinosaur noises when selecting something
* VR version?? (life… finds a way)
