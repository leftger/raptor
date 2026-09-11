"""Trace the limb axes of the Tron mesh: centroid per height band, per region.

This is what the rig's bones are placed from, so the numbers are printed for
inspection before anything is built.
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
p = raw @ matrix[:3, :3].T + matrix[:3, 3]

top = p[:, 2].max()
bottom = p[:, 2].min()


def chain(label, mask, bands=14):
    print(f"{label} (band z centre | centroid x, y | n | z extent)")
    selected = p[mask]
    if not len(selected):
        print("  (no vertices)")
        return
    z0, z1 = selected[:, 2].min(), selected[:, 2].max()
    for index in range(bands):
        a = z0 + (z1 - z0) * index / bands
        b = z0 + (z1 - z0) * (index + 1) / bands
        in_band = selected[(selected[:, 2] >= a) & (selected[:, 2] < b)]
        if len(in_band) < 8:
            continue
        print(
            f"  z {(a + b) / 2:7.2f} | x {in_band[:, 0].mean():6.2f} y {in_band[:, 1].mean():6.2f}"
            f" | n {len(in_band):5d} | z {in_band[:, 2].min():6.2f}..{in_band[:, 2].max():6.2f}"
        )
    print()


# Right arm: everything well clear of the body.
chain("RIGHT ARM  (x > 3.4)", p[:, 0] > 3.4)
# Right leg: right of centre, below the crotch.
chain("RIGHT LEG  (0.35 < x, z < 10.6)", (p[:, 0] > 0.35) & (p[:, 2] < 10.6))
# Spine: the middle column, above the legs and below the neck.
chain("TORSO      (|x| < 2.4)", (np.abs(p[:, 0]) < 2.4) & (p[:, 2] > 10.4) & (p[:, 2] < 19.9))
# Head.
chain("HEAD       (z > 19.9)", p[:, 2] > 19.9, bands=8)

print("LANDMARKS")
print("  feet z      ", round(bottom, 2))
print("  head top z  ", round(top, 2))
print("  height      ", round(top - bottom, 2))
for z in (0.0, 1.5, 3.0, 5.5, 8.0, 10.0, 10.6, 12.9, 15.5, 17.6, 19.9, 23.4):
    band = p[np.abs(p[:, 2] - z) < 0.4]
    if len(band):
        print(
            f"  z {z:5.1f}: x {band[:, 0].min():6.2f}..{band[:, 0].max():6.2f}"
            f"  y {band[:, 1].min():6.2f}..{band[:, 1].max():6.2f}  n {len(band)}"
        )
