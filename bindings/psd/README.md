# PhotoCraft PSD bindings

Rust adapters expose `photocraft-psd` to C, Python and JavaScript. Thin C++, C#, Go and Dart wrappers use the C ABI. The core PSD crate keeps `forbid(unsafe_code)`.

The API reads PSD/PSB headers, layers, masks, groups, paths, slices, resources, thumbnails and pixels. It creates layered documents, edits existing layers and serializes them. Unknown blocks and compression follow the core reader/writer's fidelity rules. Serialization may normalize padding; byte-for-byte equality is not guaranteed.

Run commands from `bindings/psd` unless noted. Use stable Rust, C11/C++17 compilers, CMake 3.20+, .NET 10, Go 1.27+, Dart 3.13+, Python 3.11+, Node.js 22+, npm, maturin, wasm-pack and Binaryen's `wasm-opt` for the respective bindings. SDK checks also use pkg-config on Unix and Swift on macOS. Rust/npm lockfiles fix dependencies; wasm-pack selects the matching wasm-bindgen CLI.

## Data

Native decoding preserves the colour model and depth: 1-bit MSB-first rows padded to bytes, 8-bit bytes, 16-bit big-endian integers, or 32-bit big-endian IEEE floats. The composite is planar; transparent RGB composites retain PSD's white matte, while layer samples are straight. Stored CMYK samples are inverted ink values. Raw decoded bytes are not interleaved builder inputs. RGBA8 previews remove the composite matte before colour conversion. Layers are bottom-to-top; bounds are `[top, left, bottom, right]`. Mask channels `-2` and `-3` use their own bounds. Foreign-language buffers own their data independently of the input and document.

Composites and layer channels support raw, PackBits RLE, ZIP and ZIP with prediction. Numeric colour-mode, section-type and compression values retain unknown PSD codes.

Parsing validates decodable saved composites by default. Composites above decoder limits remain stored without inflation; decoding them returns a size-limit error. For metadata or immediate pixel decoding, use `pc_psd_parse_lazy`, Python `Psd(bytes, lazy=True)`, or JavaScript `new Psd(bytes, true)` to defer that work. Pixel decoding checks sizes and ZIP checksums in one pass; lazy parsing can retain corrupt composites until decoding.

| Python / JavaScript | Result |
| --- | --- |
| `header_info()` / `headerInfo()` | Header, compression, merged alpha, real-composite flag |
| `layer_metadata(i)` / `layerMetadata(i)` | Name, bounds, channels, blend key, opacity/fill opacity (0–255), clipping, visibility, ID, section type, mask |
| `layer_tree()` / `layerTree()` | Preorder nodes with layer index, parent, divider, group flag |
| `resolution()` | Horizontal/vertical resolution and units |
| `resource_path(i)` / `resourcePath(i)` | Owned saved-path geometry for resource 1025 or 2000–2997; other resources return no path |
| `vector_mask(i)` / `vectorMask(i)` | Owned layer vector-mask geometry, version and flags |
| `slices()` | Owned slice collection: group name, rectangles, layer IDs and text fields |
| `color_mode_data()` / `colorModeData()` | Raw colour data, including indexed palettes |
| `resource(id)` | Resource bytes, e.g. XMP 1060, EXIF 1058, thumbnail 1036 |
| `resource_count()` / `resourceCount()` | Number of resources in file order |
| `resource_info(i)` / `resourceInfo(i)` | ID, signature, name/payload lengths; `resource_name(i)` / `resourceName(i)` and `resource_data(i)` / `resourceData(i)` return raw bytes |
| `layer_block_count(i)` / `layerBlockCount(i)` | Layer block count; `layer_block_info(i, j)` / `layerBlockInfo(i, j)` and `layer_block_data(i, j)` / `layerBlockData(i, j)` read each block |
| `global_block_count()` / `globalBlockCount()` | Global block count; `global_block_info(i)` / `globalBlockInfo(i)` and `global_block_data(i)` / `globalBlockData(i)` read each block |
| `thumbnail_info()` / `thumbnailInfo()` | Format (0: padded RGB24, 1: JPEG), dimensions, row bytes, legacy BGR flag; `thumbnail_data()` / `thumbnailData()` returns the image payload |
| `preview(max_edge)` / `preview(maxEdge)` | Sized sRGB preview: Python `(width, height, bytes)`; JavaScript `Preview` with `width`, `height`, `data`, and `free()` |
| `layer_preview(i, max_edge)` / `layerPreview(i, maxEdge)` | Sized layer thumbnail with the same result type; empty layers return 0 × 0 |
| `merged_rgba8()` / `mergedRgba8()` | Saved composite as interleaved RGBA8 |
| `layer_rgba8(i)` / `layerRgba8(i)` | RGBA8 layer at its own bounds |
| `merged_pixels_f32()` / `mergedPixelsF32()` | Source-space float image: width, height, colour mode, channels and owned data |
| `layer_pixels_f32(i)` / `layerPixelsF32(i)` | Float layer pixels at its own bounds |
| `edit_layer(i, name=..., visible=...)` / `editLayer(i, name?, visible?)` | Independent document with a renamed layer or changed visibility |
| `replace_channel(i, id, bytes)` / `replaceChannel(i, id, bytes)` | Independent document with an existing channel replaced at unchanged bounds |

Python metadata uses dictionaries with snake_case keys; JavaScript uses typed objects with camelCase keys. Missing metadata is `None` / `null`; geometry handles use `None` / `undefined`; JavaScript resource, thumbnail-payload and channel-compression results use `undefined`. Missing ICC profiles return empty bytes. Blend keys retain their raw four bytes alongside a UTF-8 string with replacement characters. Masks include bounds, flags, density/feather parameters, and real-mask fields.

Paths expose `info()`, `subpath_info(i)` / `subpathInfo(i)` and `knot(subpath, i)`. Knot coordinates are x/y fractions of canvas width/height; flags and subpath operations retain PSD codes. Typed paths reject partial records and allocations above 256 MiB; raw bytes remain available.

Slices expose `info()`, `group_name()` / `groupName()`, `slice_info(i)` / `sliceInfo(i)` and `text(i, field)`. Text fields are `name`, `url`, `target`, `message`, `alt` and `cell_text`. Slice rectangles are `[left, top, right, bottom]`; collection bounds/outsets use `[top, left, bottom, right]`. Colours are `[alpha, red, green, blue]`.

Tree parents/dividers are layer-record indices. Nesting beyond 256 levels returns an error. C structures, presence flags, and sentinels are defined in the header.

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

`sh package-c.sh` builds a relocatable SDK in `target/native-sdk` with headers, shared/static libraries, licenses, CMake targets and pkg-config metadata. Set `CMAKE_PREFIX_PATH` to the SDK and use:

```cmake
find_package(PhotoCraftPsd CONFIG REQUIRED)
target_link_libraries(your_target PRIVATE PhotoCraftPsd::Psd)
```

Use `PhotoCraftPsd::PsdStatic` for static linking; it includes Rust's native linker requirements. Deploy shared libraries beside your application or configure their runtime search path. Windows DLLs live in `bin/`, import/static libraries in `lib/`. On Unix, set `PKG_CONFIG_PATH` to the SDK's `lib/pkgconfig` and use `pkg-config --cflags --libs photocraft-psd`. The included Clang module map allows Swift to `import PhotoCraftPsd` with the SDK include/library paths.

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

## C++

`include/photocraft_psd.hpp` is a header-only C++17 adapter. Link the C library using the SDK targets above.

`photocraft::psd::parse`, `builder`, `build`, `edit_layer` and `replace_channel` return move-only owners. `bytes(function, args...)` owns byte outputs; `call<T>(function, args...)` returns plain C values. `check(status, error)` throws `std::runtime_error` on failure. All C functions remain available. Use `Buffer`, `Tree`, `Preview` and `FloatImage` with `put()` for output storage and `get()` for access; `put()` releases the previous allocation. `Path` and `Slices` own their handles. The C pointer and concurrency contracts still apply.

See `examples/inspect.cpp` and `examples/create.cpp` for ownership and creation.

## C#

`PhotoCraft.Psd` targets .NET 10 with no external managed dependencies. CI packs a NuGet artifact with native libraries for `linux-x64`, `linux-arm64`, `win-x64`, `win-arm64` and `osx-arm64`. NuGet selects the native library automatically. Linux libraries use the build runner's glibc; the Python wheels' `manylinux_2_28` baseline does not apply to them.

```csharp
using PhotoCraft.Psd;
using var doc = Document.Parse(File.ReadAllBytes("input.psd"), lazy: true);
var header = doc.Header;
var pixels = doc.DecodeMerged();
var preview = doc.Preview(256);
File.WriteAllBytes("output.psd", doc.ToBytes());
```

`Document` exposes headers, layer names/bounds, channel IDs/bounds, native pixels, colour-mode data, ICC profiles, raw resources, RGBA8 previews and serialization. `Info` reports compression, merged alpha (0/1) and real-composite availability (-1 unknown, 0 false, 1 true). `Builder` supports layers, masks, groups, composites, resources, ICC profiles and DPI with the creation rules above. `Bounds` uses top/left/bottom/right. Outputs are independent managed arrays; keep input arrays unchanged during calls. Use `using` for documents and builders. Safe handles protect native lifetimes, document reads can run concurrently, and builder calls are serialized. Native failures raise `ArgumentException` or `InvalidDataException`; disposed objects raise `ObjectDisposedException`.

See `examples/csharp/` for inspect/create consumers. `sh check-dotnet.sh <RID>` packs and tests the NuGet artifact for your host RID.

`MergedPixelsF32` / `LayerPixelsF32` return `FloatImage` with owned `float[]` data. `EditLayer` / `ReplaceChannel` return independent documents. `Resolution` and `ThumbnailInfo` are nullable; `ThumbnailData()` returns the payload. `ChannelCompression` is nullable for an empty channel without a tag. Null edit fields keep their values; an empty name clears it.

`LayerProperties`, `LayerMask` and `LayerTree` expose visibility, blend keys, opacity/fill opacity, clipping, IDs, raster masks and group relationships. `LayerMask` returns `null` when absent; mask `Parameters` uses the C presence bits (1: user density, 2: user feather, 4: vector density, 8: vector feather). `HasReal` marks real-mask fields. Tree parent/divider indices are nullable.

## Graphics frameworks

Previews provide packed sRGB RGBA8, straight alpha and `width * 4` bytes per row. Copy pixels into framework-owned storage, or retain the C/C++ buffer owner until the framework releases it. Never let a framework free Rust storage with its allocator. Reject empty previews before constructing textures/surfaces. Preserve sRGB colour metadata where supported.

| Consumer | Pixel format and ownership |
| --- | --- |
| [Qt 5/6](https://doc.qt.io/qt-6/qimage.html) | `QImage::Format_RGBA8888`; construct from preview bytes and call `copy()` for independent ownership. Set `QColorSpace::SRgb` (Qt 5.14+). |
| [GTK 4](https://docs.gtk.org/gdk4/ctor.MemoryTexture.new.html) | `GdkMemoryTexture`, `GDK_MEMORY_R8G8B8A8`, stride `width * 4`; `g_bytes_new` copies bytes, then the texture retains the `GBytes`. |
| [Skia](https://api.skia.org/namespaceSkImages.html) | `SkImageInfo::Make(width, height, kRGBA_8888_SkColorType, kUnpremul_SkAlphaType, SkColorSpace::MakeSRGB())`; `SkImages::RasterFromPixmapCopy` takes an independent copy. |
| [SDL3](https://wiki.libsdl.org/SDL3/SDL_CreateSurfaceFrom) | `SDL_PIXELFORMAT_RGBA32`; `SDL_CreateSurfaceFrom` borrows bytes. Keep their owner alive through surface destruction or copy into an owned surface. |
| [Cairo](https://cairographics.org/manual/cairo-Image-Surfaces.html) | `CAIRO_FORMAT_ARGB32` requires native-endian, premultiplied ARGB. Convert RGBA before wrapping; retain the converted buffer through surface destruction and validate Cairo's stride/status. |

For Cairo, widen components to `uint32_t`; each destination word is `(a << 24) | (premultiply(r) << 16) | (premultiply(g) << 8) | premultiply(b)`, with `premultiply(c) = (c * a + 127) / 255`; store the word in native byte order. Transparent pixels become zero. Native-depth editing still requires decoded samples and the ICC profile; these display recipes quantize to RGBA8.

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

## JavaScript and TypeScript

JavaScript `FloatImage.data` returns an owned `Float32Array` that survives its document/image and WASM memory growth. Release the image with `free()`. Encoding, supported modes and limits match the C float API above.

ES2022 is the baseline for Node.js 22+ and modern browsers with WebAssembly SIMD128, BigInt, and `TextEncoder`/`TextDecoder`. The browser loader needs `fetch` and `URL`. `--target web` selects an ESM loader, not an ES version.

```sh
rustup target add wasm32-unknown-unknown
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="-C target-feature=+simd128"
wasm-pack build --release --target nodejs --out-dir pkg-node --out-name photocraft_psd -- --features javascript --locked
node examples/inspect.mjs input.psd output.psd
wasm-pack build --release --target web --out-dir pkg --out-name photocraft_psd -- --features javascript --locked
```

See `examples/inspect.ts` for browser initialization and typed metadata. Include `ESNext.Disposable` in TypeScript's `lib` for generated declarations; the runtime disposal hook is optional. Call `free()` once when finished, then discard the object. Returned arrays stay valid. Bounds are `Int32Array`; byte data is `Uint8Array`; failures throw `Error`. Indices must be unsigned 32-bit integers; channel IDs must be signed 16-bit integers.

Decoding is synchronous. For large files, use `examples/worker.ts` with module workers. Compile to `worker/worker.js`, keep the generated web package in sibling `pkg/`, and serve both over HTTP. Send `{ bytes }` with `[bytes]` as the transfer list; the sender's buffer detaches. The worker transfers the preview back and permits retry after initialization failure.

