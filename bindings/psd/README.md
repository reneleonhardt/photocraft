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

