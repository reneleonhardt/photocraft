"""Inspect native samples and serialize a PSD/PSB."""
import sys
from pathlib import Path
from photocraft_psd import Psd

if len(sys.argv) != 3:
    raise SystemExit("usage: inspect.py <input.psd> <output.psd>")
doc = Psd(Path(sys.argv[1]).read_bytes(), lazy=True)
print(doc.header_info(), doc.resolution(), doc.layer_tree())
print(f"merged: {len(doc.decode_merged())} bytes; ICC: {len(doc.icc_profile())} bytes")
print(doc.thumbnail_info(), [doc.resource_info(i) for i in range(doc.resource_count())])
slices = doc.slices()
if slices:
    print(slices.info(), slices.group_name(), [slices.slice_info(i) for i in range(slices.info()["count"])])
for i in range(doc.layer_count):
    print(doc.layer_metadata(i))
    vector = doc.vector_mask(i)
    if vector:
        print("vector mask:", vector.info())
    for j in range(doc.layer_channel_count(i)):
        channel = doc.channel_id(i, j)
        print(f"  channel {channel}: {len(doc.decode_channel(i, channel))} bytes "
              f"{doc.channel_bounds(i, channel)}")
Path(sys.argv[2]).write_bytes(doc.to_bytes())
