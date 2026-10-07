# PhotoCraft PSD bindings

Rust adapters expose `photocraft-psd` to C, Python and JavaScript. Thin C++, C#, Go and Dart wrappers use the C ABI. The core PSD crate keeps `forbid(unsafe_code)`.

The API reads PSD/PSB headers, layers, masks, groups, paths, slices, resources, thumbnails and pixels. It creates layered documents, edits existing layers and serializes them. Unknown blocks and compression follow the core reader/writer's fidelity rules. Serialization may normalize padding; byte-for-byte equality is not guaranteed.

Run commands from `bindings/psd` unless noted. Use stable Rust, C11/C++17 compilers, CMake 3.20+, .NET 10, Go 1.27+, Dart 3.13+, Python 3.11+, Node.js 22+, npm, maturin, wasm-pack and Binaryen's `wasm-opt` for the respective bindings. SDK checks also use pkg-config on Unix and Swift on macOS. Rust/npm lockfiles fix dependencies; wasm-pack selects the matching wasm-bindgen CLI.

## Data

Native decoding preserves the colour model and depth: 1-bit MSB-first rows padded to bytes, 8-bit bytes, 16-bit big-endian integers, or 32-bit big-endian IEEE floats. The composite is planar; transparent RGB composites retain PSD's white matte, while layer samples are straight. Stored CMYK samples are inverted ink values. Raw decoded bytes are not interleaved builder inputs. RGBA8 previews remove the composite matte before colour conversion. Layers are bottom-to-top; bounds are `[top, left, bottom, right]`. Mask channels `-2` and `-3` use their own bounds. Foreign-language buffers own their data independently of the input and document.

Composites and layer channels support raw, PackBits RLE, ZIP and ZIP with prediction. Numeric colour-mode, section-type and compression values retain unknown PSD codes.

Parsing validates decodable saved composites by default. Composites above decoder limits remain stored without inflation; decoding them returns a size-limit error. For metadata or immediate pixel decoding, use `pc_psd_parse_lazy`, Python `Psd(bytes, lazy=True)`, or JavaScript `new Psd(bytes, true)` to defer that work. Pixel decoding checks sizes and ZIP checksums in one pass; lazy parsing can retain corrupt composites until decoding.

RGBA8 previews convert embedded ICC profiles to sRGB before reducing precision. RGB, grayscale, bitmap, indexed and Lab are supported; CMYK requires an embedded profile. Indexed previews and float pixels honor the transparent palette index. Invalid, mismatched or unsupported profiles return errors. Without a profile, integer RGB/Gray assume sRGB/sGray, float RGB/Gray assume linear sRGB, and Lab assumes D50. Duotone and multichannel previews are unsupported.

Use RGBA8 for thumbnails and SDR display. Editing, HDR, wide-gamut work and high-quality export need native-depth samples and ICC data. Preview output is clipped to 0–1 after colour conversion; there is no HDR tone mapping. Layer previews include transparency, but do not apply masks, effects, opacity, fill opacity or blending; they extract pixels rather than render layer appearance. A document marked as having no real saved composite returns a composite preview error; individual layers remain available.

`preview` and `layer_preview` fit within a positive maximum edge without upscaling, using alpha-aware bilinear filtering in sRGB. They decode the full composite or layer first; full-resolution RGBA output is capped at 256 MiB, not total memory. Embedded-thumbnail payloads are returned without decoding or colour conversion. Enumeration preserves duplicate resource IDs/block keys and raw Pascal name bytes.

## Editing

`edit_layer` and `replace_channel` work on opened files at 8/16/32-bit depths. They copy the document and preserve unknown blocks and untouched channels. Channel inputs retain the native decoded encoding described above; they are raster samples, not rendered text, effects or smart objects. Visibility changes and channel replacement remove embedded thumbnails and mark the saved composite unavailable. Raw composite bytes remain stored; composite previews and float extraction return errors until a renderer saves a new composite. Renaming preserves the composite. Edited layer padding is regenerated. Names are limited to 1 MiB and channel inputs to 1 GiB, excluding copy/encoding memory. C uses `pc_psd_edit_layer` and `pc_psd_replace_channel`.

## Creation

Use `PsdBuilder(width, height, *, depth=8, color_mode=3, version=1, compression=1)` in Python or `new PsdBuilder(width, height, depth?, colorMode?, version?, compression?)` in JavaScript. C uses `pc_psd_builder_new` with `PC_PSD_BUILDER_CONFIG(width, height)`. The builder supports RGB, grayscale and CMYK at 8/16 bits, PSD/PSB and all four compressions. RLE is the default for compatibility with readers such as ImageMagick that cannot decode ZIP composites.

`add_layer` / `addLayer` takes a name, bounds, interleaved colour-plus-alpha bytes, optional layer options and an optional raster mask. Sixteen-bit samples use big-endian bytes. CMYK inputs are ink amounts; PSD stores their inverse. Raster-mask inputs are 8-bit and scale to the document depth. Options default to normal blending, full opacity/fill, visible, unclipped and an enabled mask with white outside its bounds. Python keys use snake_case; JavaScript keys use camelCase. Unknown option keys are rejected. Use `mask_bounds` / `maskBounds` when supplying a mask.

Add groups with `begin_group` / `beginGroup` and `end_group` / `endGroup`. Set the saved composite with `composite(pixels)`, supplying straight samples; the builder adds RGB's white matte, with quantization at the document depth. Add ICC bytes with `icc_profile` / `iccProfile`, DPI with `resolution(dpi)` and raw resources with `resource(id, data, name?)`. ICC/DPI helpers append resources; use each once per builder. `build()` returns an independent immutable document; it does not render layers. Groups must be closed before building.

Builders use a 1 GiB input budget with per-item overhead, at most 32,767 layer records and 256 group levels. This does not cap build/encode working memory. PSD canvases are limited to 30,000 pixels per axis; PSB allows 300,000, subject to the input limit. Creation at 32-bit depth is unsupported by the core builder; reading and serialization preserve it.

Creation examples cover C, C++, C#, Go, Python and JavaScript under `examples/`, and Dart under `dart/example/`.

## C

```sh
cargo build --release --features c --locked --target-dir target/c
cc -std=c11 -Wall -Wextra -Werror -Iinclude examples/inspect.c \
  -Ltarget/c/release -lphotocraft_psd_bindings -Wl,-rpath,"$PWD/target/c/release" -o target/inspect-c
target/inspect-c input.psd output.psd
```

Ship `include/photocraft_psd.h` and the platform's `libphotocraft_psd_bindings.so`, `.dylib`, or `photocraft_psd_bindings.dll` with its Windows import library. The command above targets Linux/macOS. The separate target directory prevents Python builds from overwriting the C library.

On Windows, use a Visual Studio developer shell for the target architecture:

```bat
cargo build --release --features c --locked --target-dir target/c
cl /nologo /std:c11 /D_CRT_SECURE_NO_WARNINGS /Iinclude examples/inspect.c target/c/release/photocraft_psd_bindings.dll.lib /Fetarget/inspect-c.exe /Fotarget/inspect-c.obj
copy target\c\release\photocraft_psd_bindings.dll target\
target\inspect-c.exe input.psd output.psd
```

For static linking, obtain native flags with `cargo rustc --release --features c --locked --target-dir target/c -- --print native-static-libs`.

Follow the header's pointer contract. Fallible calls return a status, reset valid outputs on failure, and optionally fill a UTF-8 error buffer. Free documents with `pc_psd_free`, byte buffers with `pc_psd_buffer_free`, trees with `pc_psd_tree_free`, and builder/path/slice handles with their matching `*_free` function. Release storage before reusing an output; use these functions for Rust allocations. Read-only calls can share a live document with separate outputs.

Fallible calls catch Rust panics. Dangling pointers, insufficient storage, and allocation failure remain outside that guarantee.

`pc_psd_merged_pixels_f32` and `pc_psd_layer_pixels_f32` return packed source-space colour plus straight alpha. RGB composites are unmatted; CMYK becomes ink amounts; bitmap/indexed pixels expand to Gray/RGB. Values retain finite HDR range without ICC conversion, clipping or tone mapping. Lab uses normalized PSD encoding as documented in the header. Integer composite matte quantization cannot be recovered. Extra composite channels are omitted; native decoding retains them. No layer opacity, fill opacity, masks, effects or blending is applied. Duotone/multichannel are unsupported. Nonfinite colours and invalid alpha return errors. Output is capped at 1 GiB, excluding decoding memory.

```c
pc_psd_float_image pixels = {0};
if (pc_psd_merged_pixels_f32(doc, &pixels, &error) == PC_PSD_OK) {
    /* Rows contain width * channels floats; the last channel is alpha. */
    process_pixels(pixels.data.data, pixels.width, pixels.height, pixels.channels, pixels.color_mode);
}
pc_psd_float_buffer_free(&pixels.data);
```

The float buffer owns its storage independently of the document; `len` counts floats. Use the embedded ICC profile with the source colour mode when converting to a working space.

## Python

Python `FloatImage.data` is an owned, read-only native-endian `memoryview` with format `f`. Use `numpy.frombuffer(data, dtype=numpy.float32)` for a NumPy view; NumPy is optional. The buffer survives its document/image. Encoding, supported modes and limits match the C float API above.

```sh
python3 -m venv .venv
.venv/bin/python -m pip install maturin==1.15.0
.venv/bin/python -m maturin build --release --features python --locked \
  --interpreter "$PWD/.venv/bin/python" --out target/wheels
.venv/bin/python -m pip install --no-index --find-links target/wheels --no-deps photocraft-psd
.venv/bin/python examples/inspect.py input.psd output.psd
```

Import `Psd` from `photocraft_psd` and construct it with file bytes. Python manages its immutable lifetime; parsing and lookup failures raise `ValueError`. Long Rust operations detach from the interpreter. The module declares `gil_used = false` for free-threaded execution.

CI builds C libraries and Python wheels natively for Linux/Windows x86_64 and ARM64, and macOS ARM64. Linux wheels target `manylinux_2_28` (glibc 2.28+). Wheels use cibuildwheel, platform repairs, and strict ABI audits:

| Build | Wheel tags | Consumers |
| --- | --- | --- |
| Python 3.11 | `cp311-abi3` | Regular 3.11+ |
| Python 3.14t | `cp314-cp314t` | Free-threaded 3.14 |
| Python 3.15t | `cp315-abi3.abi3t` | Regular and free-threaded 3.15+ |

PyO3 selects the ABI from the interpreter; `abi3t` starts at 3.15. cibuildwheel selects free-threaded builds by their `t` suffix; no environment override is needed. Consumer jobs test stable regular 3.11–3.15 and free-threaded 3.14/3.15.

Run wheel builds from the **repository root** so Linux containers receive the PSD and CMS path dependencies:

```sh
python -m pip install 'cibuildwheel[uv]==4.3.1'
python -m cibuildwheel bindings/psd --output-dir bindings/psd/target/wheels-release
```

Linux requires Docker. macOS cibuildwheel uses python.org interpreters; direct maturin builds accept other compatible interpreters.

