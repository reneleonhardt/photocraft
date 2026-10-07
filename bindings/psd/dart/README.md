# PhotoCraft PSD for Dart

A thin Dart FFI package for PSD/PSB reading, creation and serialization. Requires Dart 3.13+ and the matching PhotoCraft PSD native SDK; its only Dart dependency is `ffi`.

```dart
import 'dart:io';
import 'package:photocraft_psd/photocraft_psd.dart';

final api = PsdLibrary.open('/path/to/libphotocraft_psd_bindings.dylib');
final doc = api.parse(File('input.psd').readAsBytesSync(), lazy: true);
try {
  print(doc.header);
  final image = doc.preview(256);
  File('output.psd').writeAsBytesSync(doc.toBytes());
} finally {
  doc.close();
}
```

Build the SDK with `sh package-c.sh` from `bindings/psd`, or deploy the corresponding CI artifact. Pass the shared library's path: `lib/libphotocraft_psd_bindings.so` on Linux, `lib/libphotocraft_psd_bindings.dylib` on macOS, or `bin/photocraft_psd_bindings.dll` on Windows. CI supplies Linux/Windows x64 and ARM64, and macOS ARM64. `PsdLibrary(dynamicLibrary)` also accepts an existing `dart:ffi` library. Mobile apps must build and link the C library separately; static consumers can use `DynamicLibrary.process()` with the C ABI symbols retained and exported. Web consumers need JavaScript interop with the WASM binding.

`Psd` exposes headers, layer names/bounds/channel IDs, native-depth decoding, colour-mode data, ICC/resources, serialization and sized document/layer previews. Returned bytes own their storage. Parsing validates the saved composite unless `lazy: true` defers validation until decoding. Serialization can normalize padding.

Native samples retain their colour model: planar, 1-bit MSB-first rows, 8-bit bytes, or big-endian 16-bit integers/32-bit floats. Previews return packed sRGB RGBA8 with straight alpha. They convert embedded ICC profiles, clip extended-range values, and quantize depth. CMYK previews require an ICC profile. Layer previews omit masks, effects, opacity and blending; document previews use the saved composite.

`layerProperties`, `layerMask` and `layerTree` expose visibility, blend keys, opacity/fill opacity, clipping, IDs, raster-mask fields and group relationships. Missing masks, mask parameters, real-mask fields and tree parent/divider indices are nullable. Returned records and lists own their data.

For Flutter, reject empty previews and pass `image.premultipliedRgba8()` to [`decodeImageFromPixels`](https://api.flutter.dev/flutter/dart-ui/decodeImageFromPixels.html) with `PixelFormat.rgba8888` and `rowBytes: image.width * 4`. This conversion returns a copy and requires no Flutter dependency in the package.

`api.builder(width, height, depth: 8, colorMode: rgb, version: 1, compression: rle)` creates RGB/Gray/CMYK documents at 8/16 bits with raw, RLE, ZIP or ZIP prediction. `addLayer` takes interleaved colour-plus-alpha bytes, named `Bounds`, optional `LayerOptions` and an 8-bit mask. Sixteen-bit input is big-endian; CMYK input is ink amounts. Set `maskBounds` when supplying a mask. Groups, composites, ICC/resources and DPI are supported; close groups before `build()`. Built documents own independent handles; building does not render layers. See `example/create.dart` and `example/inspect.dart`.

Calls are synchronous. Open a library and parse/create inside a worker isolate for large files; send owned bytes back, never handles. Close documents/builders promptly; automatic native cleanup is a fallback. `close()` is idempotent; use after closing throws `StateError`. Invalid argument ranges raise `RangeError`, native argument failures raise `ArgumentError`, and PSD/encoding failures raise `FormatException`. Loader failures propagate from `DynamicLibrary.open`.
