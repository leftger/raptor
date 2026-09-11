"""Probe the rig objectively: which local axis swings which joint, and how much
the skin tears when a joint is driven.

Nothing here relies on looking at a render, so the answers are trustworthy.

Run: blender --background --factory-startup --python-use-system-env --python probe_rig.py
"""

import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rig_lib import build_rig, edge_stretch  # noqa: E402

mesh_obj, rig = build_rig()
print("BUILT bones", len(rig.data.bones), "groups", len(mesh_obj.vertex_groups))
print("REST", edge_stretch(mesh_obj))
print()

bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="POSE")

REST_TAILS = {b.name: b.tail.copy() for b in rig.pose.bones}


def clear_pose():
    for bone in rig.pose.bones:
        bone.rotation_euler = (0.0, 0.0, 0.0)
        bone.location = (0.0, 0.0, 0.0)
    bpy.context.view_layer.update()


def probe(name, angle=math.radians(30)):
    """Rotate one bone 30 degrees about each local axis; report where the tip goes."""
    rest = REST_TAILS[name]
    out = []
    for axis in range(3):
        clear_pose()
        rotation = [0.0, 0.0, 0.0]
        rotation[axis] = angle
        rig.pose.bones[name].rotation_euler = rotation
        bpy.context.view_layer.update()
        moved = rig.pose.bones[name].tail - rest
        out.append((axis, moved))
    clear_pose()
    return out


AXIS = "XYZ"
# Which local axis moves which joint, in world terms: +y is the way the figure
# faces, +z is up.
for name in ("Thigh.R", "Shin.R", "Foot.R", "UpperArm.R", "Forearm.R", "Hips", "Chest", "Head"):
    print(f"{name}: 30 deg per local axis -> tip moves")
    for axis, moved in probe(name):
        note = []
        if abs(moved.y) > 0.4:
            note.append("FORWARD" if moved.y > 0 else "BACKWARD")
        if abs(moved.z) > 0.4:
            note.append("UP" if moved.z > 0 else "DOWN")
        if abs(moved.x) > 0.4:
            note.append("RIGHT" if moved.x > 0 else "LEFT")
        print(
            f"   local {AXIS[axis]}: d=({moved.x:6.2f},{moved.y:6.2f},{moved.z:6.2f})"
            f"  {' '.join(note) or 'small'}"
        )
    print()

# How badly does the skin tear at common joint angles?
print("TEARING CHECK (edge stretch vs rest)")
for label, poses in [
    ("thigh.R fwd 30", [("Thigh.R", 0, math.radians(30))]),
    ("thigh.R fwd 45", [("Thigh.R", 0, math.radians(45))]),
    ("knee.R bend 45", [("Shin.R", 0, math.radians(45))]),
    ("knee.R bend 75", [("Shin.R", 0, math.radians(75))]),
    ("arm.R down 60", [("UpperArm.R", 0, math.radians(60))]),
    ("hips twist 20", [("Hips", 2, math.radians(20))]),
]:
    clear_pose()
    for bone, axis, angle in poses:
        rotation = [0.0, 0.0, 0.0]
        rotation[axis] = angle
        rig.pose.bones[bone].rotation_euler = rotation
    bpy.context.view_layer.update()
    print(f"  {label:18} {edge_stretch(mesh_obj)}")

clear_pose()
bpy.ops.object.mode_set(mode="OBJECT")
print("DONE")
