"""Author a walk cycle on the rig, verify it numerically, render it, export it.

Axes come from probe_rig.py; per-side signs are probed here rather than assumed,
because Blender does not give mirrored bones mirrored local axes.

Run: blender --background --factory-startup --python-use-system-env --python walk_tron.py
"""

import math
import os
import tempfile
import sys

import bpy
from mathutils import Matrix

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rig_lib import build_rig, edge_stretch  # noqa: E402

OUT = os.environ.get("RIG_OUT", tempfile.gettempdir())
FPS = 24
CYCLE = 24  # frames per stride

mesh_obj, rig = build_rig()
bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="POSE")

# Amplitudes. The legs are 9.2 units long on a 23.6 unit figure, so a walk-sized
# stride is a modest swing; 26 degrees threw the feet 6 units apart.
THIGH_SWING = math.radians(16)
KNEE_BEND = math.radians(42)
FOOT_ROLL = math.radians(9)
ARM_LOWER = math.radians(48)
ARM_SWING = math.radians(13)
ARM_ELBOW = math.radians(16)
HIP_TWIST = math.radians(5)
HIP_BOB = 0.45
SPINE_LEAN = math.radians(3)

REST_TAILS = {b.name: b.tail.copy() for b in rig.pose.bones}
REST_MIN_Z = min(v.co.z for v in mesh_obj.data.vertices)


def world_rot(bone_name, lower=0.0, swing=0.0):
    """A frontal-plane drop (about world Y) composed with a forward swing (about
    world X), converted into the bone's own local space.

    The arm bones' local axes are not aligned with the frontal plane, so a local
    rotation sweeps the arm forward instead of dropping it to the side.
    """
    rest = rig.data.bones[bone_name].matrix_local.to_3x3()
    rotation = Matrix.Rotation(swing, 3, "X") @ Matrix.Rotation(lower, 3, "Y")
    return (rest.inverted() @ rotation @ rest).to_euler("XYZ")


def clear_pose():
    for bone in rig.pose.bones:
        bone.rotation_euler = (0.0, 0.0, 0.0)
        bone.location = (0.0, 0.0, 0.0)
    bpy.context.view_layer.update()


def probe_sign(family, side, axis):
    """+20 degrees about `axis`: does the tip go forward (legs) / down (arms)?"""
    clear_pose()
    rotation = [0.0, 0.0, 0.0]
    rotation[axis] = math.radians(20)
    rig.pose.bones[f"{family}.{side}"].rotation_euler = rotation
    bpy.context.view_layer.update()
    moved = rig.pose.bones[f"{family}.{side}"].tail - REST_TAILS[f"{family}.{side}"]
    clear_pose()
    if family in ("UpperArm", "Forearm"):
        return 1.0 if moved.z < 0.0 else -1.0
    # The model faces -Y: the toes are the long overhang from the ankle, and
    # swing toward +Y is the model's backward, which folds the knees forward.
    return 1.0 if moved.y < 0.0 else -1.0


print("PROBED SIGNS (+1 means the positive angle walks forward / lowers the arm)")
SIGNS = {}
for family, axis in (("Thigh", 0), ("Shin", 0), ("Foot", 0), ("UpperArm", 2), ("Forearm", 2)):
    row = []
    for side in ("L", "R"):
        SIGNS[(family, side)] = probe_sign(family, side, axis)
        row.append(f"{side}{SIGNS[(family, side)]:+.0f}")
    print(f"  {family:9} {' '.join(row)}")


def pose_for(phase, hip_offset=0.0):
    """Phase 0 is the double-support contact pose."""
    clear_pose()
    swing = math.sin(phase)
    for side, offset in (("R", 0.0), ("L", math.pi)):
        leg_phase = phase + offset
        thigh = SIGNS[("Thigh", side)] * THIGH_SWING * math.sin(leg_phase)
        # The knee folds only while that leg is off the ground. The thigh peaks
        # forward at pi/2, so the swing is centred on phase 0 and the knee must
        # bend there, not half a cycle later.
        knee = -SIGNS[("Shin", side)] * KNEE_BEND * max(0.0, math.cos(leg_phase))
        foot = SIGNS[("Foot", side)] * FOOT_ROLL * math.cos(leg_phase)
        rig.pose.bones[f"Thigh.{side}"].rotation_euler = (thigh, 0.0, 0.0)
        rig.pose.bones[f"Shin.{side}"].rotation_euler = (knee, 0.0, 0.0)
        rig.pose.bones[f"Foot.{side}"].rotation_euler = (foot, 0.0, 0.0)
        # Arms hang down from the A-pose and counter-swing the legs.
        drop = ARM_LOWER if side == "R" else -ARM_LOWER
        swing_arm = -ARM_SWING * math.sin(leg_phase + math.pi)
        rig.pose.bones[f"UpperArm.{side}"].rotation_euler = world_rot(
            f"UpperArm.{side}", lower=drop, swing=swing_arm
        )
        rig.pose.bones[f"Forearm.{side}"].rotation_euler = world_rot(
            f"Forearm.{side}", swing=ARM_ELBOW
        )
    # Hips: twist with the stride, lowest at contact, highest at passing.
    rig.pose.bones["Hips"].rotation_euler = (0.0, -HIP_TWIST * swing, 0.0)
    bob = -HIP_BOB * (1.0 + math.cos(2 * phase)) * 0.5
    rig.pose.bones["Hips"].location = (0.0, 0.0, bob + hip_offset)
    rig.pose.bones["Chest"].rotation_euler = (0.0, HIP_TWIST * swing, 0.0)
    rig.pose.bones["Spine"].rotation_euler = (-SPINE_LEAN, 0.0, 0.0)
    rig.pose.bones["Head"].rotation_euler = (0.0, -HIP_TWIST * swing * 0.5, 0.0)
    bpy.context.view_layer.update()


def lowest_vertex():
    """Lowest point of the deformed mesh, the way the floor sees it."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = mesh_obj.evaluated_get(depsgraph)
    posed = evaluated.to_mesh()
    low = min(v.co.z for v in posed.vertices)
    evaluated.to_mesh_clear()
    return low


# ---------------------------------------------------------------------------
# Both sides should now be mirror images under the same pose.
# ---------------------------------------------------------------------------
print()
print("MIRROR CHECK (same pose both sides; left should mirror right)")
clear_pose()
for side in ("L", "R"):
    rig.pose.bones[f"Thigh.{side}"].rotation_euler = (SIGNS[("Thigh", side)] * THIGH_SWING, 0, 0)
    rig.pose.bones[f"UpperArm.{side}"].rotation_euler = (
        0,
        0,
        SIGNS[("UpperArm", side)] * ARM_LOWER,
    )
bpy.context.view_layer.update()
for name in ("Thigh", "UpperArm", "Hand"):
    right = rig.pose.bones[f"{name}.R"].tail
    left = rig.pose.bones[f"{name}.L"].tail
    error = math.dist((-left.x, left.y, left.z), (right.x, right.y, right.z))
    print(
        f"  {name:9} R ({right.x:6.2f},{right.y:6.2f},{right.z:6.2f})"
        f"  L ({left.x:6.2f},{left.y:6.2f},{left.z:6.2f})  error {error:5.2f}"
    )

# ---------------------------------------------------------------------------
# Ground clamp: the lowest point of the whole cycle should just touch the
# resting foot level, so the walk neither floats nor sinks through the floor.
# ---------------------------------------------------------------------------
print()
print("FOOT CONTACT THROUGH THE CYCLE")
lows = []
for step in range(CYCLE):
    pose_for(math.tau * step / CYCLE)
    lows.append(lowest_vertex())
print(f"  rest lowest {REST_MIN_Z:.3f}  cycle lowest {min(lows):.3f}  highest {max(lows):.3f}")
clamp = REST_MIN_Z - min(lows)
print(f"  CLAMP hips by {clamp:+.3f} so the lowest moment lands on the rest contact")
print("  every other frame:", " ".join(f"{low + clamp:.2f}" for low in lows[::2]))

print()
print("LIMB POSITIONS THROUGH THE CYCLE (hands should hang at the sides: small |x|, y near 0)")
for step in range(0, CYCLE, 3):
    phase = math.tau * step / CYCLE
    pose_for(phase, clamp)
    hand = rig.pose.bones["Hand.R"].tail
    foot_r = rig.pose.bones["Foot.R"].tail
    foot_l = rig.pose.bones["Foot.L"].tail
    print(
        f"  frame {step + 1:2}  handR ({hand.x:5.2f},{hand.y:5.2f},{hand.z:5.2f})"
        f"  footR y {foot_r.y:5.2f} z {foot_r.z:5.2f} | footL y {foot_l.y:5.2f} z {foot_l.z:5.2f}"
    )

print()
print("KNEE CHECK (model faces -Y, so a folded knee puts the ankle at a LARGER y than the knee)")
for label, phase in (("right mid-swing", 0.0), ("left mid-swing", math.pi)):
    pose_for(phase, clamp)
    for side in ("L", "R"):
        knee = rig.pose.bones[f"Thigh.{side}"].tail
        ankle = rig.pose.bones[f"Shin.{side}"].tail
        folds = "FOLDS BACK" if ankle.y > knee.y + 0.05 else "straight"
        print(f"  {label:16} {side}: knee y {knee.y:+.2f} ankle y {ankle.y:+.2f}  {folds}")

print()
print("STRETCH AT THE EXTREMES")
for label, phase in (("contact", 0.0), ("passing", math.pi / 2), ("lift-off", math.pi)):
    pose_for(phase, clamp)
    print(f"  {label:9} {edge_stretch(mesh_obj)}")

# ---------------------------------------------------------------------------
# Key the cycle and export.
# ---------------------------------------------------------------------------
scene = bpy.context.scene
scene.render.fps = FPS
scene.frame_start = 1
scene.frame_end = CYCLE

for frame in range(1, CYCLE + 2):
    pose_for(math.tau * (frame - 1) / CYCLE, clamp)
    for bone in rig.pose.bones:
        bone.keyframe_insert("rotation_euler", frame=frame)
        bone.keyframe_insert("location", frame=frame)
print("KEYED frames 1..", CYCLE + 1)
rig.animation_data.action.name = "Walk"

scene.render.engine = "BLENDER_WORKBENCH"
scene.render.resolution_x = 420
scene.render.resolution_y = 640
camera_data = bpy.data.cameras.new("Preview")
camera_data.type = "ORTHO"
camera_data.ortho_scale = 26.0
camera = bpy.data.objects.new("Preview", camera_data)
scene.collection.objects.link(camera)
scene.camera = camera
SIDE = ((60.0, 0.0, 12.0), (math.radians(85), 0.0, math.radians(90)))

for frame, label in ((1, "contact"), (7, "passing"), (13, "contact2")):
    scene.frame_set(frame)
    camera.location, camera.rotation_euler = SIDE
    scene.render.filepath = f"{OUT}/walk_{label}_side.png"
    bpy.ops.render.render(write_still=True)
    print("RENDERED", label)

bpy.ops.object.mode_set(mode="OBJECT")
scene.frame_set(1)
bpy.ops.export_scene.gltf(
    filepath=f"{OUT}/tron_rigged.gltf",
    export_format="GLTF_SEPARATE",
    export_animations=True,
    export_skins=True,
    # The source mesh had tangents; without them the normal map loses fidelity.
    export_tangents=True,
)
print(f"EXPORTED {OUT}/tron_rigged.gltf")
print("DONE")
