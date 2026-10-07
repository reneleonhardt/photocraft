"""Create a layered PSD with a saved composite."""
import sys
from pathlib import Path
from photocraft_psd import PsdBuilder

if len(sys.argv) != 2:
    raise SystemExit("usage: create.py <output.psd>")
pixels = bytes([255, 0, 0, 128])
builder = PsdBuilder(1, 1)
builder.add_layer("Red", [0, 0, 1, 1], pixels)
builder.composite(pixels)
builder.resolution(300)
Path(sys.argv[1]).write_bytes(builder.build().to_bytes())
