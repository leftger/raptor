"""Install the corrected rigged export into the repo.

Blender writes flat filenames next to the export; the repo keeps its textures in
textures/ and its buffer as scene.bin, so the URIs are repointed rather than the
assets duplicated.
"""

import json
import os
import tempfile
import pathlib
import shutil

REPO = pathlib.Path(__file__).resolve().parents[2]
src = pathlib.Path(os.environ.get("RIG_OUT", tempfile.gettempdir()))
dst = REPO / "assets/models/tron_character"

g = json.loads((src / "tron_rigged.gltf").read_text())
for buffer in g["buffers"]:
    if buffer.get("uri") == "tron_rigged.bin":
        buffer["uri"] = "scene.bin"
for image in g.get("images", []):
    uri = image.get("uri", "")
    if "/" not in uri and uri.endswith(".png"):
        image["uri"] = f"textures/{uri}"

(dst / "scene.gltf").write_text(json.dumps(g, indent=1) + "\n")
shutil.copyfile(src / "tron_rigged.bin", dst / "scene.bin")

prim = g["meshes"][0]["primitives"][0]
pos = g["accessors"][prim["attributes"]["POSITION"]]
size = [round(pos["max"][i] - pos["min"][i], 3) for i in range(3)]
print("installed:")
for name in ("scene.gltf", "scene.bin"):
    print(f"  {name}  {(dst / name).stat().st_size} bytes")
print("meshes", len(g["meshes"]), "| skins", len(g["skins"]), "| joints", len(g["skins"][0]["joints"]))
print("skinned:", "JOINTS_0" in prim["attributes"], "| tangents:", "TANGENT" in prim["attributes"])
print("animations", [a.get("name") for a in g["animations"]])
print("mesh size x, y(height), z:", size)
print("buffer", g["buffers"][0]["uri"])
print("images", sorted(i["uri"] for i in g["images"]))
