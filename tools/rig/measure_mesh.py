"""Measure the Tron mesh so the rig is built from data rather than guesswork.

Run with:
    blender --background --factory-startup --python-use-system-env --python measure_mesh.py
"""

import bpy
import numpy as np

GLTF = "/home/usuario/Projects/open-source-repos/raptor/assets/models/tron_character/scene.gltf"

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=GLTF)

obj = next(o for o in bpy.data.objects if o.type == "MESH")
mesh = obj.data
count = len(mesh.vertices)
raw = np.empty(count * 3, dtype=np.float64)
mesh.vertices.foreach_get("co", raw)
raw = raw.reshape(count, 3)

matrix = np.array(obj.matrix_world)
points = raw @ matrix[:3, :3].T + matrix[:3, 3]

low = points.min(axis=0)
high = points.max(axis=0)
print("BOUNDS min", np.round(low, 3), "max", np.round(high, 3))
print("SIZE   x(width)", round(high[0] - low[0], 3), "y(depth)", round(high[1] - low[1], 3), "z(height)", round(high[2] - low[2], 3))
print("FEET   z =", round(low[2], 3), " HEAD TOP z =", round(high[2], 3))
print()


def silhouette(a_values, b_values, a_low, a_high, b_low, b_high, cols, rows, label, a_name, b_name):
    print(f"{label}: {a_name} across {cols} cols, {b_name} down {rows} rows")
    grid = np.zeros((rows, cols), dtype=int)
    ai = np.clip(((a_values - a_low) / (a_high - a_low) * cols).astype(int), 0, cols - 1)
    bi = np.clip(((b_values - b_low) / (b_high - b_low) * rows).astype(int), 0, rows - 1)
    np.add.at(grid, (bi, ai), 1)
    for row in range(rows - 1, -1, -1):
        b_mid = b_low + (row + 0.5) * (b_high - b_low) / rows
        cells = "".join("#" if v else "." for v in grid[row])
        print(f"  {b_mid:7.2f} |{cells}|")
    print(f"  {'':7} +{'-' * cols}+")
    print(f"  {a_name} from {a_low:.2f} (left) to {a_high:.2f} (right)")
    print()


# Front view: width across, height down. This is the view the game camera sees.
pad = 0.6
silhouette(
    points[:, 0], points[:, 2],
    low[0] - pad, high[0] + pad, low[2] - pad, high[2] + pad,
    48, 24, "FRONT VIEW (silhouette)", "x", "z",
)
# Side view: depth across, height down. Shows which way the figure faces.
silhouette(
    points[:, 1], points[:, 2],
    low[1] - pad, high[1] + pad, low[2] - pad, high[2] + pad,
    24, 24, "SIDE VIEW (silhouette)", "y", "z",
)

# Numbers per height band: how wide, how deep, and how the legs split.
print("HEIGHT BANDS (z centre | x range | y range | vertices | x-clusters over 0.35)")
bands = 20
for index in range(bands - 1, -1, -1):
    z0 = low[2] + (high[2] - low[2]) * index / bands
    z1 = low[2] + (high[2] - low[2]) * (index + 1) / bands
    mask = (points[:, 2] >= z0) & (points[:, 2] < z1)
    if not mask.any():
        continue
    band = points[mask]
    xs = np.sort(band[:, 0])
    # Count clusters: gaps wider than 10% of the width mean separate limbs.
    gaps = np.diff(xs)
    wide = int((gaps > (high[0] - low[0]) * 0.05).sum())
    print(
        f"  z {z0:7.2f}..{z1:7.2f} | x {band[:, 0].min():7.2f}..{band[:, 0].max():7.2f}"
        f" | y {band[:, 1].min():6.2f}..{band[:, 1].max():6.2f}"
        f" | n {len(band):5d} | gaps {wide}"
    )
