"""Consumer checks for Python type conversion and error translation."""
import sys
from pathlib import Path
from photocraft_psd import Psd, PsdBuilder
from concurrent.futures import ThreadPoolExecutor
from threading import Barrier
import sysconfig

doc = Psd(Path(sys.argv[1]).read_bytes())
assert (doc.width, doc.height, doc.version, doc.channels, doc.depth, doc.color_mode, doc.layer_count) == (2, 1, 1, 4, 16, 3, 1)
assert doc.layer_name(0) == "Red ☺"
assert list(doc.layer_bounds(0)) == [2, -1, 3, 1]
assert doc.layer_channel_count(0) == 4
assert [doc.channel_id(0, i) for i in range(4)] == [-1, 0, 1, 2]
assert list(doc.channel_bounds(0, 0)) == [2, -1, 3, 1]
assert doc.decode_channel(0, 0) == bytes.fromhex("ffff ffff")
assert doc.decode_merged() == bytes.fromhex("ffff ffff 0000 7fff 0000 7fff ffff 8000")
assert doc.icc_profile()[36:40] == b"acsp"
assert doc.header_info()["compression"] == 1
assert doc.header_info()["has_real_merged_data"] is True
assert doc.layer_metadata(0)["mask"] is None
assert doc.resolution()["horizontal"] == 300.0
assert doc.resource(1060) == b"<xmp/>" and doc.resource(65535) is None
assert doc.color_mode_data() == b""
assert doc.merged_rgba8() == doc.layer_rgba8(0) == bytes.fromhex("ff0000ff ff000080")

for i in range(doc.resource_count()):
    info = doc.resource_info(i)
    assert info["data_len"] == len(doc.resource_data(i))
    assert info["name_len"] == len(doc.resource_name(i))
    if info["id"] == 1060: assert doc.resource_name(i) == b"XMP"
for i in range(doc.layer_block_count(0)):
    assert doc.layer_block_info(0, i)["data_len"] == len(doc.layer_block_data(0, i))
assert doc.global_block_count() == 1
assert bytes(doc.global_block_info(0)["key"]) == b"TEST" and doc.global_block_data(0) == b"global"
assert doc.thumbnail_info() == dict(resource_id=1036, format=0, width=2, height=1, row_bytes=8, bgr=False)
assert doc.thumbnail_data() == bytes.fromhex("ff0000ff00000000")
assert doc.preview(1) == (1, 1, bytes([255, 0, 0, 192]))
assert doc.layer_preview(0, 1) == doc.preview(1)
edited = doc.edit_layer(0, name="Black", visible=False).replace_channel(0, 0, bytes(4))
Path(sys.argv[1]).with_name("edited-python.psd").write_bytes(edited.to_bytes())

path_index = next(i for i in range(doc.resource_count()) if doc.resource_info(i)["id"] == 2000)
owner = Psd(doc.to_bytes())
outline = owner.resource_path(path_index)
vector = owner.vector_mask(0)
slices = owner.slices()
del owner
assert outline.info() == dict(subpath_count=1, version=0, flags=0, initial_fill=False, is_vector_mask=False)
assert outline.subpath_info(0) == dict(knot_count=3, operation=1, closed=True)
assert outline.knot(0, 1) == dict(pre=[1.0, 0.0], anchor=[1.0, 0.0], post=[1.0, 0.0], linked=True)
assert vector.info()["is_vector_mask"] and vector.info()["flags"] == 4
assert slices.info() == dict(bounds=[0, 0, 1, 2], count=1, version=6)
assert slices.group_name() == "assets" and slices.text(0, "name") == "red" and slices.text(0, "alt") == "Red"
assert slices.slice_info(0)["bounds"] == [0, 0, 2, 1] and slices.slice_info(0)["has_layer_id"]
assert doc.resource_path(0) is None

builder = PsdBuilder(1, 1, depth=16, compression=3)
pixels = bytes.fromhex("ffff 0000 0000 8000")
builder.begin_group("Folder")
builder.add_layer("Red", [0, 0, 1, 1], pixels,
    dict(blend_mode=[110, 111, 114, 109], opacity=127, fill_opacity=128, visible=False, clipping=True,
         mask_bounds=[0, 0, 1, 1], mask_default=255, mask_disabled=True), b"\x80")
builder.end_group()
builder.composite(pixels)
builder.resolution(300)
builder.icc_profile(doc.icc_profile())
builder.resource(1060, b"<xmp/>", b"XMP")
created = Psd(builder.build().to_bytes())
assert created.decode_channel(1, 0) == b"\xff\xff" and created.decode_channel(1, -2) == b"\x80\x80"
assert created.layer_metadata(1)["fill_opacity"] == 128 and created.resolution()["horizontal"] == 300
assert created.resource(1060) == b"<xmp/>" and created.merged_rgba8() == bytes([255, 0, 0, 128])
assert created.slices() is None and created.vector_mask(1) is None
del builder, created
assert outline.knot(0, 2)["anchor"] == [1.0, 1.0]

fixtures = sorted(Path(sys.argv[1]).parent.joinpath("fixtures").glob("*.psd"))
assert len(fixtures) == 24, "generate the full codec/depth/version matrix first"
for path in fixtures:
    version, depth, compression = (int(part[1:]) for part in path.stem.split("-"))
    raw = path.read_bytes()
    matrix = Psd(raw)
    assert Psd(raw, lazy=True).decode_merged() == matrix.decode_merged()
    if compression in (2, 3):
        corrupt = raw[:-1] + bytes([raw[-1] ^ 1])
        for call in (lambda: Psd(corrupt), lambda: Psd(corrupt, lazy=True).decode_merged()):
            try:
                call()
            except ValueError:
                pass
            else:
                raise AssertionError("corrupt ZIP checksum accepted")
    assert (matrix.version, matrix.depth, matrix.header_info()["compression"]) == (version, depth, compression)
    assert matrix.to_bytes() == raw
    layer = matrix.layer_metadata(1)
    assert (layer["name"], layer["opacity"], layer["fill_opacity"], layer["clipping"], layer["visible"], layer["blend_mode"]) == ("Red ☺", 127, 128, 1, False, "norm")
    assert bytes(layer["blend_mode_key"]) == b"norm"
    assert layer["id"] is not None and layer["mask"]["flags"] & 2
    assert layer["mask"]["bounds"] == dict(top=2, left=0, bottom=3, right=1)
    assert matrix.layer_tree() == [dict(index=2, parent=None, divider=0, is_group=True), dict(index=1, parent=2, divider=None, is_group=False)]
    for id in [-2, -1, 0, 1, 2]:
        assert matrix.channel_compression(1, id) == compression
    red = {8: "ffff", 16: "ffffffff", 32: "3f8000003f800000"}[depth]
    assert matrix.decode_channel(1, 0) == bytes.fromhex(red)
    assert matrix.decode_merged() == bytes.fromhex({
        8: "ffff007f007fff80", 16: "ffffffff00007fff00007fffffff8000",
        32: "3f8000003f800000000000003f000000000000003f0000003f8000003f000000",
    }[depth])
    assert matrix.layer_rgba8(1) == matrix.merged_rgba8() == bytes.fromhex("ff0000ff ff000080")
    image = matrix.layer_pixels_f32(1)
    floats = image.data
    assert (image.width, image.height, image.color_mode, image.channels) == (2, 1, 3, 4)
    assert floats.format == "f" and floats.itemsize == 4 and floats.readonly and floats.contiguous
    assert list(floats)[:7] == [1, 0, 0, 1, 1, 0, 0] and abs(floats[7] - 0.5) < 0.002
    assert matrix.merged_pixels_f32().data[0] == 1
    renamed = matrix.edit_layer(1, name="Ink 🎨")
    assert Psd(renamed.to_bytes()).layer_name(1) == "Ink 🎨" and renamed.header_info()["has_real_merged_data"] is True
    changed = renamed.replace_channel(1, 0, bytes.fromhex(red) if depth != 32 else bytes.fromhex("4000000040000000"))
    hidden = changed.edit_layer(1, visible=True)
    reopened = Psd(hidden.to_bytes())
    assert reopened.layer_metadata(1)["visible"] and reopened.thumbnail_info() is None
    assert reopened.header_info()["has_real_merged_data"] is False and matrix.layer_name(1) == "Red ☺"
    assert reopened.layer_pixels_f32(1).data[0] == (2 if depth == 32 else 1)
    del image, matrix
    assert floats[0] == 1

# Concurrent first-use ICC initialization, decoding and serialization.
shared = Psd(doc.to_bytes(), lazy=True)
barrier = Barrier(4)
def read_shared(_):
    barrier.wait(timeout=10)
    return shared.merged_rgba8(), shared.decode_merged(), shared.to_bytes(), tuple(shared.merged_pixels_f32().data), shared.edit_layer(0, name="Copy").layer_name(0)
expected = doc.merged_rgba8(), doc.decode_merged(), doc.to_bytes(), tuple(doc.merged_pixels_f32().data), "Copy"
with ThreadPoolExecutor(max_workers=4) as executor:
    assert all(value == expected for value in executor.map(read_shared, range(16)))
if sysconfig.get_config_var("Py_GIL_DISABLED"):
    assert not sys._is_gil_enabled(), "import unexpectedly re-enabled the GIL"
for call, expected in [
    (lambda: doc.edit_layer(999), ValueError),
    (lambda: doc.replace_channel(0, 123, b""), ValueError),
    (lambda: doc.replace_channel(0, 0, b""), ValueError),
    (lambda: doc.layer_pixels_f32(999), ValueError),
    (lambda: Psd(b"invalid"), ValueError),
    (lambda: Psd("invalid"), TypeError),
    (lambda: doc.layer_name(999), ValueError),
    (lambda: doc.layer_bounds(999), ValueError),
    (lambda: doc.layer_channel_count(999), ValueError),
    (lambda: doc.channel_id(0, 999), ValueError),
    (lambda: doc.channel_bounds(0, 99), ValueError),
    (lambda: doc.decode_channel(0, 99), ValueError),
    (lambda: doc.layer_name(-1), OverflowError),
    (lambda: doc.layer_metadata(999), ValueError),
    (lambda: doc.channel_compression(0, 99), ValueError),
    (lambda: doc.layer_rgba8(999), ValueError),
    (lambda: doc.resource(65536), OverflowError),
    (lambda: doc.resource_info(999), ValueError),
    (lambda: doc.resource_name(999), ValueError),
    (lambda: doc.resource_data(999), ValueError),
    (lambda: doc.layer_block_count(999), ValueError),
    (lambda: doc.layer_block_info(0, 999), ValueError),
    (lambda: doc.layer_block_data(0, 999), ValueError),
    (lambda: doc.global_block_info(999), ValueError),
    (lambda: doc.global_block_data(999), ValueError),
    (lambda: doc.preview(0), ValueError),
    (lambda: doc.layer_preview(0, 0), ValueError),
    (lambda: doc.layer_preview(999, 1), ValueError),
    (lambda: doc.resource_path(999), ValueError),
    (lambda: doc.vector_mask(999), ValueError),
    (lambda: outline.subpath_info(999), ValueError),
    (lambda: outline.knot(0, 999), ValueError),
    (lambda: slices.slice_info(999), ValueError),
    (lambda: slices.text(0, "invalid"), ValueError),
    (lambda: PsdBuilder(0, 1), ValueError),
    (lambda: PsdBuilder(1, 1).add_layer("", [0, 0, 1, 1], bytes(4), dict(opacitty=1)), ValueError),
    (lambda: PsdBuilder(1, 1).add_layer("", [0, 0, 1, 1], b""), ValueError),
    (lambda: PsdBuilder(1, 1).add_layer("", [0, 0, 1, 1], bytes(4), dict(opacity=256)), OverflowError),
]:
    try:
        call()
    except expected:
        pass
    else:
        raise AssertionError(f"expected {expected.__name__}")
print("Python consumer checks passed")
