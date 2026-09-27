"""Vaulter logo: a cel-shaded vault door with a loot gem, rendered in
the Borderlands style (flat toon bands, heavy ink outlines).

blender -b --python logo.py -- <out.png> <size> <line_px> <bolts 0/1>

The shipped art: `logo.png` is this at 1024 scaled to 256; `logo-small.png`
is `NOHATCH=1 OUTLINE=0.075` at 512 scaled to 64.
"""
import math
import os
import sys

import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT = argv[0] if len(argv) > 0 else "logo.png"
SIZE = int(argv[1]) if len(argv) > 1 else 1024
LINE = float(argv[2]) if len(argv) > 2 else 7.0
BOLTS = (argv[3] == "1") if len(argv) > 3 else False

# ---- scene ----------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
for engine in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
    try:
        scene.render.engine = engine
        break
    except TypeError:
        pass
scene.render.resolution_x = SIZE
scene.render.resolution_y = SIZE
scene.render.film_transparent = True
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.view_settings.view_transform = "Standard"
scene.view_settings.look = "None"
scene.render.use_freestyle = False
scene.render.line_thickness_mode = "ABSOLUTE"
scene.render.line_thickness = LINE
try:
    scene.eevee.taa_render_samples = 32
except AttributeError:
    pass

world = bpy.data.worlds.new("World")
scene.world = world
world.use_nodes = True
world.node_tree.nodes["Background"].inputs[1].default_value = 0.0


def hex_rgb(h):
    h = h.lstrip("#")
    srgb = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    lin = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in srgb]
    return (*lin, 1.0)


def toon(name, shadow, mid, light, split=(0.28, 0.62), hatch=None):
    """Three flat bands (shadow, mid, highlight) from diffuse light, with
    optional ink hatching over the shadow band."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    diffuse = nt.nodes.new("ShaderNodeBsdfDiffuse")
    to_rgb = nt.nodes.new("ShaderNodeShaderToRGB")
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    emit = nt.nodes.new("ShaderNodeEmission")
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    ramp.color_ramp.interpolation = "CONSTANT"
    els = ramp.color_ramp.elements
    els[0].position = 0.0
    els[0].color = hex_rgb(shadow)
    els[1].position = split[0]
    els[1].color = hex_rgb(mid)
    e = els.new(split[1])
    e.color = hex_rgb(light)
    nt.links.new(diffuse.outputs[0], to_rgb.inputs[0])
    nt.links.new(to_rgb.outputs[0], ramp.inputs[0])
    color = ramp.outputs[0]
    if hatch:
        # Diagonal ink strokes, only where the surface is in shadow.
        coords = nt.nodes.new("ShaderNodeTexCoord")
        wave = nt.nodes.new("ShaderNodeTexWave")
        wave.wave_type = "BANDS"
        wave.bands_direction = "DIAGONAL"
        wave.inputs["Scale"].default_value = hatch
        wave.inputs["Distortion"].default_value = 0.0
        strokes = nt.nodes.new("ShaderNodeValToRGB")
        strokes.color_ramp.interpolation = "CONSTANT"
        strokes.color_ramp.elements[0].color = (0, 0, 0, 1)
        strokes.color_ramp.elements[1].position = 0.62
        strokes.color_ramp.elements[1].color = (1, 1, 1, 1)
        in_shadow = nt.nodes.new("ShaderNodeMath")
        in_shadow.operation = "LESS_THAN"
        in_shadow.inputs[1].default_value = split[0]
        both = nt.nodes.new("ShaderNodeMath")
        both.operation = "MULTIPLY"
        mix = nt.nodes.new("ShaderNodeMixRGB")
        mix.inputs[2].default_value = hex_rgb("#140B08")
        nt.links.new(coords.outputs["Object"], wave.inputs["Vector"])
        nt.links.new(wave.outputs["Color"], strokes.inputs[0])
        nt.links.new(to_rgb.outputs[0], in_shadow.inputs[0])
        nt.links.new(strokes.outputs[0], both.inputs[0])
        nt.links.new(in_shadow.outputs[0], both.inputs[1])
        nt.links.new(both.outputs[0], mix.inputs[0])
        nt.links.new(ramp.outputs[0], mix.inputs[1])
        color = mix.outputs[0]
    nt.links.new(color, emit.inputs[0])
    nt.links.new(emit.outputs[0], out.inputs[0])
    return mat


INK = bpy.data.materials.new("ink")
INK.use_nodes = True
INK.use_backface_culling = True
_ink_emit = INK.node_tree.nodes.new("ShaderNodeEmission")
_ink_emit.inputs[0].default_value = hex_rgb("#140B08")
INK.node_tree.links.new(_ink_emit.outputs[0], INK.node_tree.nodes["Material Output"].inputs[0])
for _n in list(INK.node_tree.nodes):
    if _n.type == "BSDF_PRINCIPLED":
        INK.node_tree.nodes.remove(_n)

DOOR = toon("door", "#C2183F", "#FF6A13", "#FFB27A", split=(float(os.environ.get("S0", "0.5")), float(os.environ.get("S1", "0.92"))), hatch=None if os.environ.get("NOHATCH") else 11)
RIM = toon("rim", "#7F0E2A", "#E11D48", "#FF6A13")
DIAL = toon("dial", "#121214", "#26262C", "#3A3A42")
GEM = toon("gem", "#FF6A13", "#FFC49A", "#FFFFFF", split=(0.55, 0.88))
BOLT = toon("bolt", "#2A2A30", "#55555F", "#8A8A96")


def shade_flat(obj):
    for p in obj.data.polygons:
        p.use_smooth = False


def bevel(obj, width, segments=2):
    m = obj.modifiers.new("bevel", "BEVEL")
    m.width = width
    m.segments = segments
    m.limit_method = "ANGLE"


root = bpy.data.objects.new("logo", None)
scene.collection.objects.link(root)


OUTLINE = float(os.environ.get("OUTLINE", "0.035"))


def add(obj, mat):
    obj.data.materials.append(mat)
    obj.data.materials.append(INK)
    obj.parent = root
    # Inverted hull: a slightly larger, inside-out copy drawn in ink.
    m = obj.modifiers.new("outline", "SOLIDIFY")
    m.thickness = OUTLINE
    m.offset = 1.0
    m.use_flip_normals = True
    m.use_rim = False
    m.material_offset = 1
    return obj


# Door: a thick disc with a chamfered edge.
bpy.ops.mesh.primitive_cylinder_add(vertices=160, radius=1.0, depth=0.3, location=(0, 0, 0))
door = add(bpy.context.object, DOOR)
bevel(door, 0.12, 6)
bpy.ops.object.shade_smooth()

# Recessed dial.
bpy.ops.mesh.primitive_cylinder_add(vertices=160, radius=0.6, depth=0.1, location=(0, 0, 0.13))
dial = add(bpy.context.object, DIAL)
bevel(dial, 0.035, 4)
bpy.ops.object.shade_smooth()

# Loot gem: a hexagonal bipyramid standing upright (long lower point), its
# girdle tilted toward the viewer so the facets catch the light.
import bmesh
mesh = bpy.data.meshes.new("gem")
bm = bmesh.new()
zc = 0.3
ring = [bm.verts.new((0.34 * math.cos(t), 0.06 * math.sin(t), zc + 0.2 * math.sin(t))) for t in (math.radians(30 + 60 * k) for k in range(6))]
top_v = bm.verts.new((0, 0.46, zc + 0.05))
bot_v = bm.verts.new((0, -0.6, zc + 0.05))
for k in range(6):
    a, b = ring[k], ring[(k + 1) % 6]
    bm.faces.new((a, b, top_v))
    bm.faces.new((b, a, bot_v))
bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
bm.to_mesh(mesh)
bm.free()
gem = bpy.data.objects.new("gem", mesh)
scene.collection.objects.link(gem)
add(gem, GEM)
shade_flat(gem)

# Four locking bolts on the diagonals.
if BOLTS:
    for i in range(4):
        a = math.radians(45 + 90 * i)
        bpy.ops.mesh.primitive_cube_add(size=1, location=(math.cos(a) * 1.02, math.sin(a) * 1.02, 0))
        b = add(bpy.context.object, BOLT)
        b.scale = (0.22, 0.34, 0.26)
        b.rotation_euler = (0, 0, a - math.pi / 2)
        bevel(b, 0.04, 2)

# A 3/4 tilt so the thickness and bevels read as 3D.
root.rotation_euler = (math.radians(-14), math.radians(16), 0)

# ---- light and camera -------------------------------------------------------------
sun_data = bpy.data.lights.new("sun", "SUN")
sun_data.energy = float(os.environ.get("SUN", "6.5"))
sun_data.use_shadow = False
sun = bpy.data.objects.new("sun", sun_data)
scene.collection.objects.link(sun)
sun.rotation_euler = (math.radians(float(os.environ.get("SUNX", "26"))), math.radians(float(os.environ.get("SUNY", "-34"))), 0)

cam_data = bpy.data.cameras.new("cam")
cam_data.type = "ORTHO"
cam_data.ortho_scale = 2.52 if BOLTS else 2.2
cam = bpy.data.objects.new("cam", cam_data)
scene.collection.objects.link(cam)
cam.location = (0, 0, 10)
cam.rotation_euler = (0, 0, 0)
scene.camera = cam

# ---- ink lines (Freestyle) --------------------------------------------------------
fs = scene.view_layers[0].freestyle_settings
fs.crease_angle = math.radians(140)
ls = fs.linesets[0] if fs.linesets else fs.linesets.new("ink")
ls.select_by_visibility = True
ls.select_silhouette = True
ls.select_border = True
ls.select_crease = False
ls.select_material_boundary = True
if ls.linestyle is None:
    ls.linestyle = bpy.data.linestyles.new("ink")
style = ls.linestyle
style.color = hex_rgb("#140B08")[:3]
style.thickness = LINE
style.caps = "ROUND"

scene.render.filepath = OUT
bpy.ops.render.render(write_still=True)
print("rendered", OUT)
