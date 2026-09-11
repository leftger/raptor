"""Measure which way the authored knee bends.

A human knee folds backward, so when flexed the ankle sits BEHIND the knee. The
model faces -Y in Blender, so "behind" means a LARGER y: the shin's y must exceed
the thigh's. If it is smaller, the knee is hyperextending (the duck-walk) and the
whole cycle is authored facing the wrong way.

Run: blender --background --factory-startup --python-use-system-env --python check_knee.py
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rig_lib import build_rig  # noqa: E402

mesh_obj, rig = build_rig()
bpy_context = rig
import bpy  # noqa: E402

bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="POSE")

KNEE_BEND = math.radians(42)
THIGH_SWING = math.radians(16)
SIGNS = {("Thigh", s): 1.0 for s in ("L", "R")}
for side in ("L", "R"):
    # Probe, exactly as the walk script does.
    rig.pose.bones[f"Shin.{side}"].rotation_euler = (math.radians(20), 0, 0)
    bpy.context.view_layer.update()
    rest = rig.data.bones[f"Shin.{side}"].tail_local
    moved = rig.pose.bones[f"Shin.{side}"].tail - rest
    SIGNS[("Shin", side)] = 1.0 if moved.y > 0.0 else -1.0
    rig.pose.bones[f"Shin.{side}"].rotation_euler = (0, 0, 0)
    bpy.context.view_layer.update()
print("SIGNS", SIGNS)

for phase_label, phase in (("mid-swing (max bend)", 0.0), ("contact", math.pi / 2), ("mid-stance", math.pi)):
    for side in ("L", "R"):
        for bone in rig.pose.bones:
            bone.rotation_euler = (0, 0, 0)
        leg_phase = phase + (0.0 if side == "R" else math.pi)
        thigh = SIGNS[("Thigh", side)] * THIGH_SWING * math.sin(leg_phase)
        knee = -SIGNS[("Shin", side)] * KNEE_BEND * max(0.0, math.cos(leg_phase))
        rig.pose.bones[f"Thigh.{side}"].rotation_euler = (thigh, 0, 0)
        rig.pose.bones[f"Shin.{side}"].rotation_euler = (knee, 0, 0)
        bpy.context.view_layer.update()

        hip = rig.pose.bones[f"Thigh.{side}"].head
        joint = rig.pose.bones[f"Thigh.{side}"].tail
        ankle = rig.pose.bones[f"Shin.{side}"].tail
        thigh_dir = (joint - hip).normalized()
        shin_dir = (ankle - joint).normalized()
        verdict = (
            "FOLDS BACK (ok)"
            if shin_dir.y > thigh_dir.y
            else "HYPEREXTENDS: the cycle is authored facing the wrong way"
        )
        print(
            f"  {phase_label:20} {side}: thigh.y {thigh_dir.y:+.2f} shin.y {shin_dir.y:+.2f}"
            f"  knee angle {math.degrees(thigh_dir.angle(shin_dir)):5.1f} deg  -> {verdict}"
        )
print("DONE")
