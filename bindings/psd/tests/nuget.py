"""Check the managed assembly and exact native runtime assets in a NuGet package."""
import sys
from zipfile import ZipFile

with ZipFile(sys.argv[1]) as package:
    names = set(package.namelist())
    assert "lib/net10.0/PhotoCraft.Psd.dll" in names
    assert {"README.md", "LICENSE-MIT", "LICENSE-APACHE"} <= names
    expected = set()
    for rid in sys.argv[2:]:
        library = ("photocraft_psd_bindings.dll" if rid.startswith("win-") else
                   "libphotocraft_psd_bindings.dylib" if rid.startswith("osx-") else
                   "libphotocraft_psd_bindings.so")
        expected.add(f"runtimes/{rid}/native/{library}")
    assert {name for name in names if name.startswith("runtimes/")} == expected
    assert not any(name.endswith((".psd", ".cs", ".cpp", ".rs", ".pdb")) for name in names)
    assert all(package.getinfo(name).file_size > 0 for name in expected)
print("NuGet runtime assets checked.")
