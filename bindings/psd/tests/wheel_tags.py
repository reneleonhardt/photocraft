"""Require the three wheel families per platform; no redundant version-specific GIL builds."""
import sys
from pathlib import Path

wheels = list(Path(sys.argv[1]).glob("*.whl"))
expected = {"cp311-abi3", "cp314-cp314t", "cp315-abi3.abi3t"}
actual = {"-".join(wheel.name.split("-")[-3:-1]) for wheel in wheels}
assert actual == expected and len(wheels) == 3, (actual, [p.name for p in wheels])
print("Release wheel families:", ", ".join(sorted(actual)))
