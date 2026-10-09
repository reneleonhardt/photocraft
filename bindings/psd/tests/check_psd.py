"""Check generated PSDs against literal expectations using independent readers."""

import argparse
import re
import subprocess
from pathlib import Path

import numpy as np
from psd_tools import PSDImage
from psd_tools.constants import Resource
from psd_tools.psd.vector import Path as PsdPath, Subpath


def check(path, magick=None):
    matrix = re.fullmatch(r"v([12])-d(8|16|32)-c([0-3])", path.stem)
    created = path.stem.startswith("created-")
    edited = path.stem.startswith("edited-")
    version, depth, compression = map(int, matrix.groups()) if matrix else (1, 8 if created else 16, 1)
    width = 1 if created else 2
    psd = PSDImage.open(path)
    assert (psd.version, psd.size, psd.depth, psd.color_mode, psd.channels) == (version, (width, 1), depth, 3, 4)
    assert psd._record.image_data.compression == compression
    resolution = psd.image_resources.get_data(Resource.RESOLUTION_INFO)
    assert (resolution.horizontal, resolution.vertical) == (300 * 65536, 300 * 65536)
    layers = list(psd.descendants())
    assert len(layers) == (2 if matrix else 1)
    layer = layers[-1]
    assert layer.kind == "pixel" and layer.name == ("Black" if edited else "Red" if created else "Red ☺")
    assert layer.bbox == ((0, 0, 1, 1) if created else (-1, 2, 1, 3))
    assert (layer.opacity, layer.fill_opacity, layer.visible, layer.clipping) == (
        (127, 128, False, True) if matrix else (255, 255, not edited, False)
    )
    color = np.array([[[1, 0, 0]] * width], dtype=np.float32)
    alpha = {8: 128 / 255, 16: 32768 / 65535, 32: 0.5}[depth]
    shape = np.array([[[alpha]] if created else [[1], [alpha]]], dtype=np.float32)
    np.testing.assert_array_equal(layer.numpy("color"), np.zeros_like(color) if edited else color)
    np.testing.assert_array_equal(layer.numpy("shape"), shape)
    # Allow only floating-point unmatting roundoff, below one 16-bit sample step.
    np.testing.assert_allclose(psd.numpy("color"), color, rtol=0, atol=1e-6)
    np.testing.assert_array_equal(psd.numpy("shape"), shape)
    assert layer.has_mask() == bool(matrix)
    if matrix:
        assert layers[0].is_group() and layers[0].name == "Folder" and layer.parent is layers[0]
        assert (layer.mask.bbox, layer.mask.disabled, layer.mask.background_color) == ((0, 2, 1, 3), True, 255)
        mask = 0.501953125 if depth == 32 else 128 / 255
        np.testing.assert_array_equal(layer.numpy("mask"), np.array([[[mask]]], dtype=np.float32))
    assert layer.has_vector_mask() == (not created)
    if not created:
        assert layer.vector_mask.disabled
        assert [[k.anchor for k in subpath] for subpath in layer.vector_mask.paths] == [[(0, 0), (0, 1), (1, 1)]]
        saved = PsdPath.frombytes(psd.image_resources.get_data(2000))
        assert [[k.anchor for k in subpath] for subpath in saved if isinstance(subpath, Subpath)] == [[(0, 0), (0, 1), (1, 1)]]
        profile = psd.image_resources.get_data(Resource.ICC_PROFILE)
        assert profile[16:20] == b"RGB " and profile[36:40] == b"acsp"
        assert psd.image_resources.get_data(Resource.XMP_METADATA) == b"<xmp/>"
        # psd-tools decodes raw thumbnails as RGBX; this resource contains padded RGB24.
        thumbnail = psd.image_resources.get_data(Resource.THUMBNAIL_RESOURCE)
        if edited:
            assert thumbnail is None and not psd.image_resources.get_data(Resource.VERSION_INFO).has_composite
        else:
            assert (thumbnail.fmt, thumbnail.width, thumbnail.height, thumbnail.row, thumbnail.bits, thumbnail.planes) == (0, 2, 1, 8, 24, 1)
            assert thumbnail.data == b"\xff\0\0\xff\0\0\0\0"
        slices = psd.image_resources.get_data(Resource.SLICES)
        assert (slices.version, slices.data.name, slices.data.bbox, len(slices.data.items)) == (6, "assets", [0, 0, 1, 2], 1)
        item = slices.data.items[0]
        assert (item.slice_id, item.associated_id, item.name, item.bbox, item.alt_tag) == (7, 2 if matrix else 1, "red", [0, 0, 2, 1], "Red")
    if magick and not edited:
        rgba = b"\xff\0\0\x80" if created else b"\xff\0\0\xff\xff\0\0\x80"
        layer_rgba = b"\xff\0\0\x7f\xff\0\0\x40" if matrix else rgba
        # ImageMagick supports ZIP layer channels, but only RAW/RLE composites.
        for scene, expected in [(1, layer_rgba)] + ([(0, rgba)] if compression < 2 else []):
            result = subprocess.run(
                [magick, "-regard-warnings", f"{path}[{scene}]", "-depth", "8", "rgba:-"],
                capture_output=True, check=True, timeout=30,
            )
            assert not result.stderr, result.stderr.decode(errors="replace")
            assert result.stdout == expected, f"ImageMagick scene {scene}: {result.stdout.hex()}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--imagemagick", metavar="EXECUTABLE", help="also check decoded RGBA8 with ImageMagick")
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()
    for path in args.paths:
        try:
            check(path, args.imagemagick)
        except Exception as error:
            raise AssertionError(f"{path}: {error}") from error
    readers = "psd-tools + ImageMagick" if args.imagemagick else "psd-tools"
    print(f"Independent generated PSD checks ({readers}): {len(args.paths)} passed")


if __name__ == "__main__":
    main()
