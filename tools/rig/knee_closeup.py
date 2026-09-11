"""Close-up render of the knee at maximum flexion, to see which way it folds.

Run: blender --background --factory-startup --python-use-system-env --python knee_closeup.py
"""

import math
import os
import tempfile
import sys

import bpy

OUT = os.environ.get("RIG_OUT", tempfile.gettempdir())

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rig_lib import build_rig  # noqa: E402

mesh_obj, rig = build_rig()
bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="POSE")

KNEE_BEND = math.radians(42)
THIGH_SWING = math.radians(16)

for bone in rig.pose.bones:
    bone.rotation_euler = (0, 0, 0)
# Phase 0: the right leg is mid-swing, which is where the knee folds most.
rig.pose.bones["Thigh.R"].rotation_euler = (0.0, 0.0, 0.0)
rig.pose.bones["Shin.R"].rotation_euler = (-KNEE_BEND, 0.0, 0.0)
bpy.context.view_layer.update()

hip = rig.pose.bones["Thigh.R"].head
joint = rig.pose.bones["Thigh.R"].tail
ankle = rig.pose.bones["Shin.R"].tail
print(f"HIP   {tuple(round(v, 2) for v in hip)}")
print(f"KNEE  {tuple(round(v, 2) for v in joint)}")
print(f"ANKLE {tuple(round(v, 2) for v in ankle)}")
print("forward is +Y; the ankle should be BEHIND the knee (smaller y):", round(ankle.y - joint.y, 2))

scene = bpy.context.scene
scene.render.engine = "BLENDER_WORKBENCH"
scene.render.resolution_x = 640
scene.render.resolution_y = 640

camera_data = bpy.data.cameras.new("Preview")
camera_data.type = "ORTHO"
camera_data.ortho_scale = 12.0
camera = bpy.data.objects.new("Preview", camera_data)
scene.collection.objects.link(camera)
scene.camera = camera

# Straight down the X axis, looking at the right thigh, so the fold is侧-on.
camera.location = (40.0, 0.0, 6.0)
camera.rotation_euler = (math.radians(90), 0.0, math.radians(90))

scene.render.filepath = os.path.join(OUT, "knee_closeup.png")
bpy.ops.render.render(write_still=True)
print(f"RENDERED {OUT}/knee_closeup.png")
