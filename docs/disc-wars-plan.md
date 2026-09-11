# RAPTOR — Disc wars mini-game plan

Status: proposal
Owner: —
Last updated: 2026-09-10

A fighting disc-wars arena entered from source files (`.c` / `.h` / `.cpp` /
`.py` / `.rs`) during Lightcycle mode. Each file procedurally generates the ring,
hazards, and pickups from its own bytes.

## 1. Concept

Code files are still crash towers today. Markdown already has a second arena
that keeps the folder in memory and rebuilds the city on close — disc wars
should be that same pattern, not a new mode stacked on `M`.

Lightcycle is already Tron: trails, gates, identity. Markdown became a *page*
because reading is the point. Source is identity and conflict: you throw a
disc, it comes back, and the arena is the program you just rode into.

Each language is a ring type, not a different game:

| Language | Ring / identity | Why it reads |
| --- | --- | --- |
| `.rs` | Cyan identity disc, tight arena, ownership walls | Borrowed space you cannot occupy twice |
| `.c` / `.h` | Amber I/O disc, header as a second ring | Include graph as linked platforms |
| `.cpp` | Magenta overload disc, stacked rings | Classes as raised galleries |
| `.py` | Green indent disc, stepped terraces | Indentation as actual elevation |

Same controls as the cycle where possible (`A` / `D` to turn, `R` restart,
`U` / `-` leave). Combat adds **throw** (space / click) and **recall**. There
is no new WASD fighter.

## 2. How you enter it

Mirror markdown, do not invent a third interaction mode.

1. Extend `FileNode` with `is_source()` next to `is_markdown()`.
2. Directory collision: source towers become a fifth cell type
   (`CellContent::Source`) instead of `CrashReason::File`.
3. Ride in → same transport beam → load bytes with the existing document-style
   cap (256 KiB, truncation shown in the status line).
4. Navigator **stays on the folder**. Close gate / `U` restores the city from
   already-loaded entries, same as folio close.
5. Explorer still opens the file in the system editor. Disc wars is
   lightcycle-only.

That keeps `M` as explorer ↔ lightcycle, and file-type as *which inner arena*.

## 3. The fight, in one sentence

You and a **Recognizer-style opponent** (a disc-wielding NPC, not a second
bike) occupy a circular ring generated from the file. You throw your disc; it
rides a short trail, bounces, and either returns or locks as a wall. First to
land a hit wins the round. Best of three, then the close gate opens.

Keep it a *mini* game:

- No health bars with 12 stats.
- One disc in flight at a time per fighter.
- Hits are “you occupied the same cell as a live disc,” the same cell model
  the cycle already uses.
- Trails are **temporary ring segments**, not infinite lightcycle walls —
  otherwise a 200-line file becomes an instant maze.

**Win:** opponent derezzes, close gate lights up, status line shows `DISC: WIN`.

**Lose:** you derezz, brief crash FX (already exists), `R` rematches, gate
still lets you leave.

## 4. What the file actually generates

Do **not** compile or fully parse. Fingerprint the bytes the way documents
already do (`stable_path_seed(path) ^ content_hash`), then a cheap tokenizer per
language. Same file always produces the same ring, same opponent, same pickups
— that rule already exists for city themes and music.

### Ring geometry (the environment)

- **File size / line count** → ring radius (clamped, like
  `LIGHTCYCLE_MIN_ARENA_SPAN`).
- **Functions / `fn` / `def` / methods** → concentric inner rings or raised
  galleries. A 4-function file is a small coliseum with 4 alcoves.
- **Indent / braces** → terrace height. Python files literally step. C is
  flatter, with header-include bridges if `#include` local headers exist in the
  same folder.
- **Comments / TODOs / `FIXME`** → dark pits or low walls (dead code as
  hazards).
- **Tests (`#[test]`, `test_`, `_test.`)** → safe pads: no disc damage, your
  disc recharges faster.
- **Unsafe / `goto` / raw pointers / `eval`** → red hazard tiles. Standing on
  them ticks a short fuse; riding through is a gamble.
- **Imports / `use` / `#include`** → portals *inside the ring* that warp you
  to another alcove. Not directory jumps.
- **Empty / stub files** → tiny practice ring, opponent is slow, one pickup.
  Still playable.

### Powerups (thrown into the ring as discs or floor glyphs)

Map constructs to *verbs*, not loot tables:

| Source signal | Pickup | Effect |
| --- | --- | --- |
| `TODO` / `unimplemented!` | Glitch disc | Next throw phases through one wall |
| `async` / threads / `go` | Split disc | Throw leaves a delayed echo |
| `match` / `switch` | Fork | Disc splits left/right once |
| Generics / templates | Heavy disc | Slower throw, breaks a hazard tile |
| `pub` / exports | Widen | Your trail lasts one extra cell |
| Panics / `abort` / `raise` | Spike | Your disc is lethal on the return path too |
| Doc comments | Shield | One free absorb |
| `cfg` / `#ifdef` | Phase floor | A terrace drops for a beat |

Cap pickups (maybe 6) the way glyphs and city structures are capped. A 10k-line
`.rs` must not spawn an item per function.

### The opponent

Seeded from the same fingerprint:

- Aggressive if the file is dense (high token / line).
- Defensive if the file is mostly comments.
- “Compiler” themed: it throws from a dais labeled with the language (`rustc`,
  `cc`, `CPython`). Beating it is “the file compiled.” Losing is a crash dump
  in the status line (`error[E0308]`-style flavor text derived from language, not
  a real diagnostic).

## 5. Combat vs the current sim

Do **not** overload `LightcycleSim` with HP. Add a sibling `DiscSim` (or
`ArenaKind::Disc`) that still steps on the same fixed clock
(`LIGHTCYCLE_FIXED_STEP`).

**Shared:** cell grid, heading, queued turns, crash FX, chase camera, music
action profile.

**New:** disc projectile (cell + heading + remaining life), opponent AI (pick
a heading each cell, throw on line-of-sight), pickup occupancy.

`RunEnvironment` gains a third arm next to `Directory` and `Document`:

```text
Source { path, language, layout: DiscLayout, round, score }
```

Visibility: hide the directory scene the same way documents do. Close gate =
`restore_directory`, already implemented.

## 6. Presentation

- Floor: darker than the city, language-tinted (cyan rust, amber C, …).
- Ring wall: the file’s stem name as a circling bitmap glyph, same 8x8 font as
  document headings.
- Your cycle can stay the bike *or* stand you on a disc pad — keep the bike
  for one control scheme, and treat the disc as the weapon it throws, not a
  vehicle swap.
- Entry: reuse the satellite zoom, but the apex looks *down into the ring*
  instead of a spawn road. Same plugin, different landing pose.
- Music: action profile, plus a per-language drum. Rust slightly more arpeggiated,
  Python slower pump. Still the folder theme, so leaving the ring returns to the
  city arrangement.

HUD: `DISC 1–0 | RING: src/plugins/lightcycle.rs | rustc` in the existing status
line. Folio panel can show the focused function’s signature when you ride near
its alcove — reading still exists, fight is the foreground.

## 7. What this is not

- Not a real language server, compiler, or debugger.
- Not a second global mode. If you press `M` inside a ring, you leave
  lightcycle entirely (same as documents).
- Not for every file. Binaries, images, lockfiles still crash. Source
  extensions are an allowlist.
- Headers (`.h`) can either be their own small ring or a *linked gallery* if
  the matching `.c` sits in the same folder — fun, but phase 2. Phase 1: every
  listed extension is a self-contained ring.

## 8. Suggested build order

1. **Collision + stub arena** — source towers enter, load bytes, spawn an
   empty round ring with a close gate, no AI. Proves the load/restore path.
2. **Throw / recall / hit cells** — you vs a stationary dummy. Restart and
   close still work.
3. **Tokenizer → layout** — functions, hazards, pickups from the fingerprint.
   Determinism tests like city generation.
4. **Opponent AI** — line-of-sight throw, simple dodge. Best of three.
5. **Language palettes + HUD + music tint** — the thematic layer, after the
   loop is fun.

The first slice that would feel real is (1)+(2): riding into `main.rs` and
actually throwing a disc, even on a blank ring. Everything else is the file
talking back.
