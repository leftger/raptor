"""Shared rig construction for the Tron mesh.

Bone positions come from trace_limbs.py (centroids per height band).
Blender is Z-up; the glTF export converts back to Y-up.
"""

import os
import bpy
import tempfile

SOURCE = os.environ.get("TRON_SOURCE", os.path.join(tempfile.gettempdir(), "tron_src", "scene.gltf"))

SPINE = {
    "Hips": ((0.0, 0.0, 10.30), (0.0, 0.0, 12.90)),
    "Spine": ((0.0, 0.0, 12.90), (0.0, 0.0, 15.60)),
    "Chest": ((0.0, 0.0, 15.60), (0.0, 0.0, 18.20)),
    "Neck": ((0.0, 0.0, 18.20), (0.0, 0.0, 19.90)),
    "Head": ((0.0, 0.0, 19.90), (0.0, 0.0, 23.40)),
}
LIMBS = {
    "Shoulder": ((0.70, 0.10, 17.60), (2.90, 0.60, 18.00)),
    "UpperArm": ((2.90, 0.60, 18.00), (5.60, -0.10, 15.90)),
    "Forearm": ((5.60, -0.10, 15.90), (7.60, -0.80, 13.90)),
    "Hand": ((7.60, -0.80, 13.90), (8.30, -1.10, 12.90)),
    "Thigh": ((1.55, 0.05, 10.30), (1.70, 0.60, 5.60)),
    "Shin": ((1.70, 0.60, 5.60), (1.72, 0.35, 1.10)),
    "Foot": ((1.72, 0.35, 1.10), (1.74, 1.30, 0.20)),
}


def mirror(point):
    return (-point[0], point[1], point[2])


def build_rig():
    """Imports the model, flattens it, builds and skins the armature."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=SOURCE)
    # The densest mesh: an imported scene can also hold stray helper objects,
    # and picking the wrong one rigs and exports the wrong thing.
    mesh_obj = max(
        (o for o in bpy.data.objects if o.type == "MESH"),
        key=lambda o: len(o.data.vertices),
    )

    # Bake the import hierarchy's 0.0254 scale into the mesh.
    bpy.ops.object.select_all(action="DESELECT")
    mesh_obj.select_set(True)
    bpy.context.view_layer.objects.active = mesh_obj
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    for empty in [o for o in bpy.data.objects if o.type == "EMPTY"]:
        bpy.data.objects.remove(empty, do_unlink=True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

    armature = bpy.data.armatures.new("TronArmature")
    rig = bpy.data.objects.new("TronRig", armature)
    bpy.context.scene.collection.objects.link(rig)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")

    bones = {}
    for name, (head, tail) in SPINE.items():
        bone = armature.edit_bones.new(name)
        bone.head, bone.tail = head, tail
        bones[name] = bone
    for name, (head, tail) in LIMBS.items():
        for side, flip in (("R", False), ("L", True)):
            bone = armature.edit_bones.new(f"{name}.{side}")
            bone.head = mirror(head) if flip else head
            bone.tail = mirror(tail) if flip else tail
            bones[f"{name}.{side}"] = bone

    bones["Spine"].parent = bones["Hips"]
    bones["Chest"].parent = bones["Spine"]
    bones["Neck"].parent = bones["Chest"]
    bones["Head"].parent = bones["Neck"]
    for side in ("L", "R"):
        bones[f"Shoulder.{side}"].parent = bones["Chest"]
        bones[f"UpperArm.{side}"].parent = bones[f"Shoulder.{side}"]
        bones[f"Forearm.{side}"].parent = bones[f"UpperArm.{side}"]
        bones[f"Hand.{side}"].parent = bones[f"Forearm.{side}"]
        bones[f"Thigh.{side}"].parent = bones["Hips"]
        bones[f"Shin.{side}"].parent = bones[f"Thigh.{side}"]
        bones[f"Foot.{side}"].parent = bones[f"Shin.{side}"]
    for name in ("Spine", "Chest", "Neck", "Head"):
        bones[name].use_connect = True
    for side in ("L", "R"):
        for name in ("Forearm", "Hand", "Shin", "Foot"):
            bones[f"{name}.{side}"].use_connect = True

    bpy.ops.object.mode_set(mode="OBJECT")

    bpy.ops.object.select_all(action="DESELECT")
    mesh_obj.select_set(True)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.parent_set(type="ARMATURE_AUTO")

    for bone in rig.pose.bones:
        bone.rotation_mode = "XYZ"
    return mesh_obj, rig


def edge_stretch(mesh_obj):
    """Longest posed/rest edge ratio, and where it happened.

    A torn or badly weighted seam shows up here as an edge stretched far beyond
    its resting length.
    """
    import numpy as np

    rest = mesh_obj.data
    rest_count = len(rest.vertices)
    rest_co = np.empty(rest_count * 3, dtype=np.float64)
    rest.vertices.foreach_get("co", rest_co)
    rest_co = rest_co.reshape(rest_count, 3)
    edges = np.empty(len(rest.edges) * 2, dtype=np.int64)
    rest.edges.foreach_get("vertices", edges)
    edges = edges.reshape(-1, 2)

    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = mesh_obj.evaluated_get(depsgraph)
    posed_mesh = evaluated.to_mesh()
    posed_count = len(posed_mesh.vertices)
    posed_co = np.empty(posed_count * 3, dtype=np.float64)
    if posed_count != rest_count:
        evaluated.to_mesh_clear()
        return None
    posed_mesh.vertices.foreach_get("co", posed_co)
    posed_co = posed_co.reshape(posed_count, 3)
    evaluated.to_mesh_clear()

    rest_len = np.linalg.norm(rest_co[edges[:, 0]] - rest_co[edges[:, 1]], axis=1)
    posed_len = np.linalg.norm(posed_co[edges[:, 0]] - posed_co[edges[:, 1]], axis=1)
    keep = rest_len > 1e-6
    ratio = posed_len[keep] / rest_len[keep]
    worst = int(np.argmax(ratio))
    worst_edge = edges[keep][worst]
    return {
        "max_ratio": float(ratio.max()),
        "over_1_5": int((ratio > 1.5).sum()),
        "over_3": int((ratio > 3.0).sum()),
        "worst_mid": (rest_co[worst_edge].mean(axis=0)).round(2).tolist(),
    }
