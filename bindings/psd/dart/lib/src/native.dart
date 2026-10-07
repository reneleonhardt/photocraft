part of '../photocraft_psd.dart';

final class _Error extends Struct {
  @Array(256)
  external Array<Uint8> message;
}

final class _Bytes extends Struct {
  external Pointer<Uint8> data;
  @Size()
  external int len;
}

// Borrowed inputs and owned outputs have the same pointer/length layout.
typedef _Buffer = _Bytes;

final class _Header extends Struct {
  @Uint32()
  external int width;
  @Uint32()
  external int height;
  @Uint16()
  external int version;
  @Uint16()
  external int channels;
  @Uint16()
  external int depth;
  @Uint16()
  external int colorMode;
  @Size()
  external int layerCount;
}

final class _Info extends Struct {
  @Uint16()
  external int compression;
  @Uint8()
  external int mergedHasAlpha;
  @Int8()
  external int hasRealMergedData;
}

final class _Resolution extends Struct {
  @Double()
  external double horizontal;
  @Double()
  external double vertical;
  @Uint16()
  external int horizontalUnit;
  @Uint16()
  external int verticalUnit;
  @Uint16()
  external int widthUnit;
  @Uint16()
  external int heightUnit;
  @Uint8()
  external int present;
}

final class _Thumbnail extends Struct {
  @Uint32()
  external int format;
  @Uint32()
  external int width;
  @Uint32()
  external int height;
  @Uint32()
  external int rowBytes;
  @Uint16()
  external int resourceId;
  @Uint8()
  external int present;
  @Uint8()
  external int bgr;
}

final class _FloatImage extends Struct {
  @Uint32()
  external int width;
  @Uint32()
  external int height;
  @Uint16()
  external int colorMode;
  @Uint16()
  external int channels;
  external _Buffer data;
}

final class _Layer extends Struct {
  @Int32()
  external int top;
  @Int32()
  external int left;
  @Int32()
  external int bottom;
  @Int32()
  external int right;
  @Size()
  external int channelCount;
}

final class _Preview extends Struct {
  @Uint32()
  external int width;
  @Uint32()
  external int height;
  external _Buffer data;
}

final class _Properties extends Struct {
  @Array(4)
  external Array<Uint8> blendMode;
  @Uint32()
  external int sectionType;
  @Uint32()
  external int id;
  @Uint8()
  external int hasId;
  @Uint8()
  external int visible;
  @Uint8()
  external int opacity;
  @Uint8()
  external int fillOpacity;
  @Uint8()
  external int clipping;
}

final class _Mask extends Struct {
  @Array(4)
  external Array<Int32> bounds;
  @Array(4)
  external Array<Int32> realBounds;
  @Double()
  external double userFeather;
  @Double()
  external double vectorFeather;
  @Uint8()
  external int present;
  @Uint8()
  external int flags;
  @Uint8()
  external int defaultColor;
  @Uint8()
  external int parameters;
  @Uint8()
  external int userDensity;
  @Uint8()
  external int vectorDensity;
  @Uint8()
  external int hasReal;
  @Uint8()
  external int realFlags;
  @Uint8()
  external int realBackground;
}

final class _TreeNode extends Struct {
  @Size()
  external int index;
  @Size()
  external int parent;
  @Size()
  external int divider;
  @Uint8()
  external int isGroup;
}

final class _Tree extends Struct {
  external Pointer<_TreeNode> data;
  @Size()
  external int len;
}

final class _Config extends Struct {
  @Uint32()
  external int width;
  @Uint32()
  external int height;
  @Uint16()
  external int colorMode;
  @Uint16()
  external int depth;
  @Uint16()
  external int version;
  @Uint16()
  external int compression;
}

final class _Options extends Struct {
  @Array(4)
  external Array<Uint8> blendMode;
  @Array(4)
  external Array<Int32> maskBounds;
  @Uint8()
  external int opacity;
  @Uint8()
  external int fillOpacity;
  @Uint8()
  external int visible;
  @Uint8()
  external int clipping;
  @Uint8()
  external int maskDefault;
  @Uint8()
  external int maskDisabled;
}

final class _Input extends Struct {
  external _Bytes name;
  @Array(4)
  external Array<Int32> bounds;
  external _Bytes pixels;
  external _Bytes mask;
  external _Options options;
}

typedef _FreeNative = Void Function(Pointer<Void>);
typedef _ParseNative = Int32 Function(
  Pointer<Uint8>,
  Size,
  Pointer<Pointer<Void>>,
  Pointer<_Error>,
);
typedef _ParseDart = int Function(
  Pointer<Uint8>,
  int,
  Pointer<Pointer<Void>>,
  Pointer<_Error>,
);
typedef _OutputNative = Int32 Function(
  Pointer<Void>,
  Pointer<_Buffer>,
  Pointer<_Error>,
);
typedef _OutputDart = int Function(
  Pointer<Void>,
  Pointer<_Buffer>,
  Pointer<_Error>,
);
typedef _BytesNative = Int32 Function(Pointer<Void>, _Bytes, Pointer<_Error>);
typedef _BytesDart = int Function(Pointer<Void>, _Bytes, Pointer<_Error>);

final class _Api {
  final DynamicLibrary lib;
  _Api(this.lib);
  late final abi = lib.lookupFunction<Uint32 Function(), int Function()>(
    'pc_psd_abi_version',
  );
  late final documentFinalizer = NativeFinalizer(
    lib.lookup<NativeFunction<_FreeNative>>('pc_psd_free'),
  );
  late final builderFinalizer = NativeFinalizer(
    lib.lookup<NativeFunction<_FreeNative>>('pc_psd_builder_free'),
  );
  late final documentFree = lib
      .lookupFunction<_FreeNative, void Function(Pointer<Void>)>('pc_psd_free');
  late final builderFree = lib
      .lookupFunction<_FreeNative, void Function(Pointer<Void>)>(
        'pc_psd_builder_free',
      );
  late final bufferFree = lib
      .lookupFunction<
        Void Function(Pointer<_Buffer>),
        void Function(Pointer<_Buffer>)
      >('pc_psd_buffer_free');
  late final parse = lib.lookupFunction<_ParseNative, _ParseDart>(
    'pc_psd_parse',
  );
  late final parseLazy = lib.lookupFunction<_ParseNative, _ParseDart>(
    'pc_psd_parse_lazy',
  );
  _OutputDart output(String name) =>
      lib.lookupFunction<_OutputNative, _OutputDart>(name);
  late final serialize = output('pc_psd_serialize');
  late final decodeMerged = output('pc_psd_decode_merged');
  late final colorModeData = output('pc_psd_color_mode_data');
  late final thumbnailData = output('pc_psd_thumbnail_data');
  late final floatBufferFree = lib
      .lookupFunction<
        Void Function(Pointer<_Buffer>),
        void Function(Pointer<_Buffer>)
      >('pc_psd_float_buffer_free');
  late final info = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_Info>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_Info>, Pointer<_Error>)
      >('pc_psd_document_info');
  late final channelBounds = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Int16,
          Pointer<Int32>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, int, Pointer<Int32>, Pointer<_Error>)
      >('pc_psd_channel_bounds');
  late final channelCompression = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Int16,
          Pointer<Int32>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, int, Pointer<Int32>, Pointer<_Error>)
      >('pc_psd_channel_compression');
  late final getResolution = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_Resolution>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_Resolution>, Pointer<_Error>)
      >('pc_psd_resolution');
  late final thumbnailInfo = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_Thumbnail>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_Thumbnail>, Pointer<_Error>)
      >('pc_psd_thumbnail_info');
  late final mergedPixelsF32 = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_FloatImage>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_FloatImage>, Pointer<_Error>)
      >('pc_psd_merged_pixels_f32');
  late final layerPixelsF32 = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Pointer<_FloatImage>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, Pointer<_FloatImage>, Pointer<_Error>)
      >('pc_psd_layer_pixels_f32');
  late final editLayer = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          _Bytes,
          Int8,
          Pointer<Pointer<Void>>,
          Pointer<_Error>,
        ),
        int Function(
          Pointer<Void>,
          int,
          _Bytes,
          int,
          Pointer<Pointer<Void>>,
          Pointer<_Error>,
        )
      >('pc_psd_edit_layer');
  late final replaceChannel = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Int16,
          _Bytes,
          Pointer<Pointer<Void>>,
          Pointer<_Error>,
        ),
        int Function(
          Pointer<Void>,
          int,
          int,
          _Bytes,
          Pointer<Pointer<Void>>,
          Pointer<_Error>,
        )
      >('pc_psd_replace_channel');
  late final header = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_Header>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_Header>, Pointer<_Error>)
      >('pc_psd_header');
  late final resource = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Uint16,
          Pointer<_Buffer>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, Pointer<_Buffer>, Pointer<_Error>)
      >('pc_psd_resource');
  late final decodeChannel = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Int16,
          Pointer<_Buffer>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, int, Pointer<_Buffer>, Pointer<_Error>)
      >('pc_psd_decode_channel');
  late final layerInfo = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Size, Pointer<_Layer>, Pointer<_Error>),
        int Function(Pointer<Void>, int, Pointer<_Layer>, Pointer<_Error>)
      >('pc_psd_layer_info');
  late final layerName = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Size, Pointer<_Buffer>, Pointer<_Error>),
        int Function(Pointer<Void>, int, Pointer<_Buffer>, Pointer<_Error>)
      >('pc_psd_layer_name');
  late final properties = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Pointer<_Properties>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, Pointer<_Properties>, Pointer<_Error>)
      >('pc_psd_layer_properties');
  late final mask = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Size, Pointer<_Mask>, Pointer<_Error>),
        int Function(Pointer<Void>, int, Pointer<_Mask>, Pointer<_Error>)
      >('pc_psd_layer_mask');
  late final tree = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_Tree>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_Tree>, Pointer<_Error>)
      >('pc_psd_layer_tree');
  late final treeFree = lib
      .lookupFunction<
        Void Function(Pointer<_Tree>),
        void Function(Pointer<_Tree>)
      >('pc_psd_tree_free');
  late final channelId = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Size,
          Pointer<Int16>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, int, Pointer<Int16>, Pointer<_Error>)
      >('pc_psd_channel_id');
  late final preview = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Uint32,
          Pointer<_Preview>,
          Pointer<_Error>,
        ),
        int Function(Pointer<Void>, int, Pointer<_Preview>, Pointer<_Error>)
      >('pc_psd_preview');
  late final layerPreview = lib
      .lookupFunction<
        Int32 Function(
          Pointer<Void>,
          Size,
          Uint32,
          Pointer<_Preview>,
          Pointer<_Error>,
        ),
        int Function(
          Pointer<Void>,
          int,
          int,
          Pointer<_Preview>,
          Pointer<_Error>,
        )
      >('pc_psd_layer_preview');
  late final builderNew = lib
      .lookupFunction<
        Int32 Function(_Config, Pointer<Pointer<Void>>, Pointer<_Error>),
        int Function(_Config, Pointer<Pointer<Void>>, Pointer<_Error>)
      >('pc_psd_builder_new');
  late final addLayer = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_Input>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_Input>, Pointer<_Error>)
      >('pc_psd_builder_add_layer');
  _BytesDart bytes(String name) =>
      lib.lookupFunction<_BytesNative, _BytesDart>(name);
  late final composite = bytes('pc_psd_builder_composite');
  late final beginGroup = bytes('pc_psd_builder_begin_group');
  late final endGroup = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<_Error>)
      >('pc_psd_builder_end_group');
  late final builderResource = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Uint16, _Bytes, _Bytes, Pointer<_Error>),
        int Function(Pointer<Void>, int, _Bytes, _Bytes, Pointer<_Error>)
      >('pc_psd_builder_resource');
  late final resolution = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Double, Pointer<_Error>),
        int Function(Pointer<Void>, double, Pointer<_Error>)
      >('pc_psd_builder_resolution');
  late final build = lib
      .lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Pointer<Void>>, Pointer<_Error>),
        int Function(Pointer<Void>, Pointer<Pointer<Void>>, Pointer<_Error>)
      >('pc_psd_builder_build');
}
