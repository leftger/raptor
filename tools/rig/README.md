# Rigging and animating the Tron character

The pipeline that turned the unrigged Sketchfab model into the rigged, skinned,
animated character the game uses. It is Blender scripting, not art: every bone
position, weight and keyframe comes from a number this repo measured.

Everything here is meant to be re-runnable, because the alternative is
re-deriving all of it from scratch.

## Requirements

- Blender 5.x on `PATH`.
- `numpy` visible to Blender's Python. Blender uses the system interpreter, so:

  ```bash
  python3 -m pip install --user --break-system-packages numpy
  blender --background --python-expr "import numpy; print(numpy.__version__)"
  ```

- **`--python-use-system-env` on every invocation.** Without it, Blender does not
  see user-installed packages and the glTF importer fails with
  `ModuleNotFoundError: No module named 'numpy'`.

## Getting a source mesh

`rig_lib.py` rigs an **unrigged** glTF, by default `<tmp>/tron_src/scene.gltf`.
The unrigged original is in git history, from just before the rig landed:

```bash
SRC=$(mktemp -d)/tron_src && mkdir -p "$SRC/textures"
git show c710917^:assets/models/tron_character/scene.gltf > "$SRC/scene.gltf"
git show c710917^:assets/models/tron_character/scene.bin  > "$SRC/scene.bin"
cp assets/models/tron_character/textures/*.png "$SRC/textures/"
export TRON_SOURCE="$SRC/scene.gltf"
```

Or point `TRON_SOURCE` at any other unrigged humanoid in a T/A-pose.

## Running it

Each step is its own script. Run them from anywhere; paths are resolved relative
to the script, and `RIG_OUT` (default: the temp dir) collects renders and the
export.

```bash
B="blender --background --factory-startup --python-use-system-env --python"

$B tools/rig/measure_mesh.py   # what the mesh actually looks like
$B tools/rig/trace_limbs.py    # limb axes, for bone placement
$B tools/rig/probe_rig.py      # joint axes + skin tearing
$B tools/rig/author_walk.py    # author the cycle, verify, render, export
$B tools/rig/check_knee.py     # which way the knee folds
$B tools/rig/knee_closeup.py   # close-up render of the flexed knee
python3 tools/rig/install_asset.py   # copy the export into assets/
```

`measure_mesh.py` prints an ASCII silhouette of the mesh, front and side, plus
the vertex extent per height band. `trace_limbs.py` prints the limb centroids
that the bone table in `rig_lib.py` was written from — if you move bones, move
them to match that output rather than by eye.

## What "good" looks like

`author_walk.py` and `probe_rig.py` print the numbers to check. Healthy is:

| Measure | Expected |
| --- | --- |
| Rest-pose edge stretch | `max_ratio 1.0000`, `over_1_5: 0` |
| Stretch at the extremes | `max_ratio` under about 1.6, `over_3: 0` |
| Foot contact | the lowest moment of the cycle sits at the rest foot level |
| Mirror check | error `0.00` between left and right |
| Knee check | the ankle sits at a **larger y** than the knee (folds back) |

## Traps

Each of these cost a debugging round, so they are written down.

- **Never rig the rigged asset.** Point `TRON_SOURCE` at an unrigged mesh. Once
  the export replaced the original, rigging it re-imported our own output, which
  appends stray objects, and the script rigged the wrong mesh. `rig_lib.py` now
  takes the densest mesh as a guard, but the source still has to be unrigged.
  A corrupted run is recognisable by its ground-clamp numbers reading a constant
  `-1.000` lowest vertex.
- **The model faces `+Z` in glTF, which is `-Y` in Blender.** Swing the legs
  toward `-Y` and fold the knees toward `+Y`. With this backwards the character
  duck-walks: the knees bend forward, which is not a thing human knees do. The
  toe is the long overhang from the ankle, and that is the quickest way to
  check.
- **The original glTF is authored in inches.** Its node matrices scale by 0.0254,
  so the mesh spans 929 units but arrives 23.607. The export bakes the node
  transforms into the vertices, which keeps the figure the same height and the
  game's scale constants valid.
- **Blender writes flat texture filenames** (`03_-_Default_baseColor.png`) and
  its own buffer name. `install_asset.py` repoints them to `textures/` and
  `scene.bin` rather than duplicating 2 MB of files.
- **No IK.** The cycle is keyframes, and the feet are planted by an automatic
  ground clamp, so the lowest moment of the cycle touches the floor but a foot
  can float up to about 0.4 units at other phases. Foot IK is the real fix.
- On the game side, Bevy's `RepeatAnimation` defaults to `Never`, so a clip
  played through `AnimationPlayer` runs once and parks on its last frame unless
  it is explicitly looped.
