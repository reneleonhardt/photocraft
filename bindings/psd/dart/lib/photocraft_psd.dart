import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

part 'src/native.dart';

typedef Bounds = ({int top, int left, int bottom, int right});
typedef PsdHeader = ({
  int width,
  int height,
  int version,
  int channels,
  int depth,
  int colorMode,
  int layerCount,
});
typedef DocumentInfo = ({
  int compression,
  bool mergedHasAlpha,
  bool? hasRealMergedData,
});
typedef ResolutionInfo = ({
  double horizontal,
  double vertical,
  int horizontalUnit,
  int verticalUnit,
  int widthUnit,
  int heightUnit,
});
typedef ThumbnailInfo = ({
  int format,
  int width,
  int height,
  int rowBytes,
  int resourceId,
  bool bgr,
});
typedef FloatImage = ({
  int width,
  int height,
  int colorMode,
  int channels,
  Float32List data,
});
typedef PsdLayer = ({String name, Bounds bounds, int channelCount});
typedef Preview = ({int width, int height, Uint8List data});
typedef LayerProperties = ({
  Uint8List blendMode,
  int sectionType,
  int? id,
  bool visible,
  int opacity,
  int fillOpacity,
  int clipping,
});
typedef LayerMask = ({
  Bounds bounds,
  int flags,
  int defaultColor,
  int? userDensity,
  double? userFeather,
  int? vectorDensity,
  double? vectorFeather,
  Bounds? realBounds,
  int? realFlags,
  int? realBackground,
});
typedef TreeNode = ({int index, int? parent, int? divider, bool isGroup});

extension PreviewPixels on Preview {
  /// Owned premultiplied RGBA8 for Flutter's PixelFormat.rgba8888.
  Uint8List premultipliedRgba8() {
    final out = Uint8List.fromList(data);
    if (out.length % 4 != 0)
      throw ArgumentError('Expected complete RGBA pixels');
    for (var i = 0; i < out.length; i += 4) {
      for (var c = 0; c < 3; c++) {
        out[i + c] = (out[i + c] * out[i + 3] + 127) ~/ 255;
      }
    }
    return out;
  }
}

const grayscale = 1, rgb = 3, cmyk = 4;
const raw = 0, rle = 1, zip = 2, zipPrediction = 3;

final class LayerOptions {
  final String blendMode;
  final Bounds maskBounds;
  final int opacity, fillOpacity, maskDefault;
  final bool visible, clipping, maskDisabled;
  const LayerOptions({
    this.blendMode = 'norm',
    this.maskBounds = (top: 0, left: 0, bottom: 0, right: 0),
    this.opacity = 255,
    this.fillOpacity = 255,
    this.maskDefault = 255,
    this.visible = true,
    this.clipping = false,
    this.maskDisabled = false,
  });
}

/// Loads a matching native SDK shared library; handles stay in their creating isolate.
final class PsdLibrary {
  final _Api _api;
  PsdLibrary.open(String path)
    : this(DynamicLibrary.open(File(path).absolute.path));
  PsdLibrary(DynamicLibrary library) : _api = _Api(library) {
    if (_api.abi() != 1) throw StateError('Unsupported PSD C ABI');
  }
  Psd parse(Uint8List data, {bool lazy = false}) => using((arena) {
    final input = _input(arena, data),
        out = arena<Pointer<Void>>(),
        error = arena<_Error>();
    _check(
      (lazy ? _api.parseLazy : _api.parse)(input.data, input.len, out, error),
      error,
    );
    return Psd._(_api, out.value);
  });
  PsdBuilder builder(
    int width,
    int height, {
    int depth = 8,
    int colorMode = rgb,
    int version = 1,
    int compression = rle,
  }) => using((arena) {
    final config = arena<_Config>();
    config.ref
      ..width = _range(width, 0, 0xffffffff)
      ..height = _range(height, 0, 0xffffffff)
      ..depth = _range(depth, 0, 65535)
      ..colorMode = _range(colorMode, 0, 65535)
      ..version = _range(version, 0, 65535)
      ..compression = _range(compression, 0, 65535);
    final out = arena<Pointer<Void>>(), error = arena<_Error>();
    _check(_api.builderNew(config.ref, out, error), error);
    return PsdBuilder._(_api, out.value);
  });
}

// Finalizable keeps this owner alive throughout native calls and blocks isolate transfer.
abstract base class _Handle implements Finalizable {
  final _Api _api;
  final NativeFinalizer _finalizer;
  final void Function(Pointer<Void>) _free;
  Pointer<Void> _pointer;
  _Handle(_Api api, this._pointer, bool builder)
    : _api = api,
      _finalizer = builder ? api.builderFinalizer : api.documentFinalizer,
      _free = builder ? api.builderFree : api.documentFree {
    _finalizer.attach(this, _pointer, detach: this);
  }
  Pointer<Void> get _live {
    if (_pointer == nullptr) throw StateError('PSD handle is closed');
    return _pointer;
  }

  /// Release promptly; repeated calls are harmless. Automatic cleanup is a fallback.
  void close() {
    if (_pointer != nullptr) {
      _finalizer.detach(this);
      _free(_pointer);
      _pointer = nullptr;
    }
  }
}

final class Psd extends _Handle {
  Psd._(_Api api, Pointer<Void> pointer) : super(api, pointer, false);
  PsdHeader get header => using((arena) {
    final out = arena<_Header>(), error = arena<_Error>();
    _check(_api.header(_live, out, error), error);
    final h = out.ref;
    return (
      width: h.width,
      height: h.height,
      version: h.version,
      channels: h.channels,
      depth: h.depth,
      colorMode: h.colorMode,
      layerCount: h.layerCount,
    );
  });
  DocumentInfo get info => using((arena) {
    final out = arena<_Info>(), error = arena<_Error>();
    _check(_api.info(_live, out, error), error);
    final o = out.ref;
    return (
      compression: o.compression,
      mergedHasAlpha: o.mergedHasAlpha != 0,
      hasRealMergedData: o.hasRealMergedData < 0
          ? null
          : o.hasRealMergedData != 0,
    );
  });
  Bounds channelBounds(int layer, int id) => using((arena) {
    final out = arena<Int32>(4), error = arena<_Error>();
    _check(
      _api.channelBounds(
        _live,
        _index(layer),
        _range(id, -32768, 32767),
        out,
        error,
      ),
      error,
    );
    return (top: out[0], left: out[1], bottom: out[2], right: out[3]);
  });
  int? channelCompression(int layer, int id) => using((arena) {
    final out = arena<Int32>(), error = arena<_Error>();
    _check(
      _api.channelCompression(
        _live,
        _index(layer),
        _range(id, -32768, 32767),
        out,
        error,
      ),
      error,
    );
    return out.value < 0 ? null : out.value;
  });
  ResolutionInfo? get resolution => using((arena) {
    final out = arena<_Resolution>(), error = arena<_Error>();
    _check(_api.getResolution(_live, out, error), error);
    final o = out.ref;
    return o.present == 0
        ? null
        : (
            horizontal: o.horizontal,
            vertical: o.vertical,
            horizontalUnit: o.horizontalUnit,
            verticalUnit: o.verticalUnit,
            widthUnit: o.widthUnit,
            heightUnit: o.heightUnit,
          );
  });
  ThumbnailInfo? get thumbnailInfo => using((arena) {
    final out = arena<_Thumbnail>(), error = arena<_Error>();
    _check(_api.thumbnailInfo(_live, out, error), error);
    final o = out.ref;
    return o.present == 0
        ? null
        : (
            format: o.format,
            width: o.width,
            height: o.height,
            rowBytes: o.rowBytes,
            resourceId: o.resourceId,
            bgr: o.bgr != 0,
          );
  });
  Uint8List thumbnailData() => _bytes(_api.thumbnailData);
  FloatImage _floats(int? layer) => using((arena) {
    final out = arena<_FloatImage>(), error = arena<_Error>();
    _check(
      layer == null
          ? _api.mergedPixelsF32(_live, out, error)
          : _api.layerPixelsF32(_live, _index(layer), out, error),
      error,
    );
    final o = out.ref;
    final buffer = arena<_Buffer>()..ref = o.data;
    try {
      return (
        width: o.width,
        height: o.height,
        colorMode: o.colorMode,
        channels: o.channels,
        data: o.data.len == 0
            ? Float32List(0)
            : Float32List.fromList(
                o.data.data.cast<Float>().asTypedList(o.data.len),
              ),
      );
    } finally {
      _api.floatBufferFree(buffer);
    }
  });
  FloatImage mergedPixelsF32() => _floats(null);
  FloatImage layerPixelsF32(int layer) => _floats(layer);
  Psd editLayer(int layer, {String? name, bool? visible}) => using((arena) {
    final input = _input(arena, name == null ? const [] : utf8.encode(name));
    if (name != null && input.len == 0) input.data = arena<Uint8>();
    final out = arena<Pointer<Void>>(), error = arena<_Error>();
    _check(
      _api.editLayer(
        _live,
        _index(layer),
        input,
        visible == null ? -1 : (visible ? 1 : 0),
        out,
        error,
      ),
      error,
    );
    return Psd._(_api, out.value);
  });
  Psd replaceChannel(int layer, int id, Uint8List data) => using((arena) {
    final input = _input(arena, data),
        out = arena<Pointer<Void>>(),
        error = arena<_Error>();
    _check(
      _api.replaceChannel(
        _live,
        _index(layer),
        _range(id, -32768, 32767),
        input,
        out,
        error,
      ),
      error,
    );
    return Psd._(_api, out.value);
  });
  Uint8List _bytes(
    int Function(Pointer<Void>, Pointer<_Buffer>, Pointer<_Error>) call,
  ) => using((arena) {
    final out = arena<_Buffer>(), error = arena<_Error>();
    _check(call(_live, out, error), error);
    return _copy(_api, out);
  });
  Uint8List toBytes() => _bytes(_api.serialize);
  Uint8List decodeMerged() => _bytes(_api.decodeMerged);
  Uint8List colorModeData() => _bytes(_api.colorModeData);
  Uint8List iccProfile() => resource(1039);
  Uint8List resource(int id) =>
      _bytes((p, out, e) => _api.resource(p, _range(id, 0, 65535), out, e));
  Uint8List decodeChannel(int layer, int id) => _bytes(
    (p, out, e) =>
        _api.decodeChannel(p, _index(layer), _range(id, -32768, 32767), out, e),
  );
  PsdLayer layer(int index) => using((arena) {
    final out = arena<_Layer>(),
        name = arena<_Buffer>(),
        error = arena<_Error>();
    final p = _live, i = _index(index);
    _check(_api.layerInfo(p, i, out, error), error);
    _check(_api.layerName(p, i, name, error), error);
    final l = out.ref;
    return (
      name: utf8.decode(_copy(_api, name)),
      bounds: (top: l.top, left: l.left, bottom: l.bottom, right: l.right),
      channelCount: l.channelCount,
    );
  });
  int channelId(int layer, int channel) => using((arena) {
    final out = arena<Int16>(), error = arena<_Error>();
    _check(
      _api.channelId(_live, _index(layer), _index(channel), out, error),
      error,
    );
    return out.value;
  });
  LayerProperties layerProperties(int layer) => using((arena) {
    final out = arena<_Properties>(), error = arena<_Error>();
    _check(_api.properties(_live, _index(layer), out, error), error);
    final o = out.ref;
    return (
      blendMode: Uint8List.fromList([
        for (var i = 0; i < 4; i++) o.blendMode[i],
      ]),
      sectionType: o.sectionType,
      id: o.hasId == 0 ? null : o.id,
      visible: o.visible != 0,
      opacity: o.opacity,
      fillOpacity: o.fillOpacity,
      clipping: o.clipping,
    );
  });
  LayerMask? layerMask(int layer) => using((arena) {
    final out = arena<_Mask>(), error = arena<_Error>();
    _check(_api.mask(_live, _index(layer), out, error), error);
    final o = out.ref;
    if (o.present == 0) return null;
    Bounds bounds(Array<Int32> b) =>
        (top: b[0], left: b[1], bottom: b[2], right: b[3]);
    return (
      bounds: bounds(o.bounds),
      flags: o.flags,
      defaultColor: o.defaultColor,
      userDensity: o.parameters & 1 == 0 ? null : o.userDensity,
      userFeather: o.parameters & 2 == 0 ? null : o.userFeather,
      vectorDensity: o.parameters & 4 == 0 ? null : o.vectorDensity,
      vectorFeather: o.parameters & 8 == 0 ? null : o.vectorFeather,
      realBounds: o.hasReal == 0 ? null : bounds(o.realBounds),
      realFlags: o.hasReal == 0 ? null : o.realFlags,
      realBackground: o.hasReal == 0 ? null : o.realBackground,
    );
  });
  List<TreeNode> layerTree() => using((arena) {
    final out = arena<_Tree>(), error = arena<_Error>();
    _check(_api.tree(_live, out, error), error);
    final absent = sizeOf<Size>() == 4 ? 0xffffffff : -1;
    try {
      return List.generate(out.ref.len, (i) {
        final n = (out.ref.data + i).ref;
        return (
          index: n.index,
          parent: n.parent == absent ? null : n.parent,
          divider: n.divider == absent ? null : n.divider,
          isGroup: n.isGroup != 0,
        );
      });
    } finally {
      _api.treeFree(out);
    }
  });
  Preview _preview(int? layer, int maxEdge) => using((arena) {
    final out = arena<_Preview>(),
        error = arena<_Error>(),
        edge = _range(maxEdge, 0, 0xffffffff);
    final status = layer == null
        ? _api.preview(_live, edge, out, error)
        : _api.layerPreview(_live, _index(layer), edge, out, error);
    _check(status, error);
    final buffer = arena<_Buffer>()..ref = out.ref.data;
    return (
      width: out.ref.width,
      height: out.ref.height,
      data: _copy(_api, buffer),
    );
  });

  /// Owned sRGB RGBA8 with straight alpha, clipped after ICC conversion.
  Preview preview(int maxEdge) => _preview(null, maxEdge);
  Preview layerPreview(int layer, int maxEdge) => _preview(layer, maxEdge);
}

final class PsdBuilder extends _Handle {
  PsdBuilder._(_Api api, Pointer<Void> pointer) : super(api, pointer, true);
  void _mutate(int Function(Pointer<Void>, Pointer<_Error>, Arena) call) =>
      using((arena) {
        final error = arena<_Error>();
        _check(call(_live, error, arena), error);
      });
  void addLayer(
    String name,
    Bounds bounds,
    Uint8List pixels, {
    LayerOptions options = const LayerOptions(),
    Uint8List? mask,
  }) => _mutate((p, e, arena) {
    final input = arena<_Input>();
    input.ref
      ..name = _input(arena, utf8.encode(name))
      ..pixels = _input(arena, pixels)
      ..mask = _input(arena, mask ?? const []);
    _bounds(input.ref.bounds, bounds);
    _bounds(input.ref.options.maskBounds, options.maskBounds);
    final blend = utf8.encode(options.blendMode);
    if (blend.length != 4)
      throw ArgumentError('Blend mode must contain four UTF-8 bytes');
    for (var i = 0; i < 4; i++) {
      input.ref.options.blendMode[i] = blend[i];
    }
    input.ref.options
      ..opacity = _range(options.opacity, 0, 255)
      ..fillOpacity = _range(options.fillOpacity, 0, 255)
      ..maskDefault = _range(options.maskDefault, 0, 255)
      ..visible = (options.visible ? 1 : 0)
      ..clipping = (options.clipping ? 1 : 0)
      ..maskDisabled = (options.maskDisabled ? 1 : 0);
    return _api.addLayer(p, input, e);
  });
  void composite(Uint8List pixels) =>
      _mutate((p, e, arena) => _api.composite(p, _input(arena, pixels), e));
  void beginGroup(String name) => _mutate(
    (p, e, arena) => _api.beginGroup(p, _input(arena, utf8.encode(name)), e),
  );
  void endGroup() => _mutate((p, e, arena) => _api.endGroup(p, e));
  void resource(int id, Uint8List data, {Uint8List? name}) => _mutate(
    (p, e, arena) => _api.builderResource(
      p,
      _range(id, 0, 65535),
      _input(arena, name ?? const []),
      _input(arena, data),
      e,
    ),
  );
  void iccProfile(Uint8List data) => resource(1039, data);
  void resolution(double dpi) =>
      _mutate((p, e, arena) => _api.resolution(p, dpi, e));
  Psd build() => using((arena) {
    final out = arena<Pointer<Void>>(), error = arena<_Error>();
    _check(_api.build(_live, out, error), error);
    return Psd._(_api, out.value);
  });
}

int _range(int value, int min, int max) {
  if (value < min || value > max) throw RangeError.range(value, min, max);
  return value;
}

int _index(int value) =>
    _range(value, 0, sizeOf<Size>() == 4 ? 0xffffffff : 0x7fffffffffffffff);
void _bounds(Array<Int32> out, Bounds bounds) {
  final values = [bounds.top, bounds.left, bounds.bottom, bounds.right];
  for (var i = 0; i < 4; i++) {
    out[i] = _range(values[i], -2147483648, 2147483647);
  }
}

// Inputs live in the arena through the synchronous call; C borrows and copies them.
_Bytes _input(Arena arena, List<int> data) {
  final out = arena<_Bytes>();
  if (data.isNotEmpty) {
    out.ref.data = arena<Uint8>(data.length);
    out.ref.data.asTypedList(data.length).setAll(0, data);
  }
  out.ref.len = data.length;
  return out.ref;
}

void _check(int status, Pointer<_Error> error) {
  if (status == 0) return;
  final message = utf8.decode([
    for (var i = 0; i < 256 && error.ref.message[i] != 0; i++)
      error.ref.message[i],
  ], allowMalformed: true);
  if (status == 1) throw ArgumentError(message);
  throw FormatException(message);
}

Uint8List _copy(_Api api, Pointer<_Buffer> out) {
  try {
    return out.ref.len == 0
        ? Uint8List(0)
        : Uint8List.fromList(out.ref.data.asTypedList(out.ref.len));
  } finally {
    api.bufferFree(out);
  }
}
