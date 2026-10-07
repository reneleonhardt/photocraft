import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:photocraft_psd/photocraft_psd.dart';

void check(bool value, [Object message = 'PSD consumer check failed']) {
  if (!value) throw StateError('$message');
}

void equal(List<int> a, List<int> b) => check(
  a.length == b.length &&
      Iterable<int>.generate(a.length).every((i) => a[i] == b[i]),
);
void fails<T extends Object>(void Function() call) {
  try {
    call();
  } catch (error) {
    if (error is T) return;
    rethrow;
  }
  throw StateError('Expected $T');
}

void main(List<String> args) {
  if (args.length < 2)
    throw ArgumentError('usage: smoke <library> <fixtures...>');
  final api = PsdLibrary.open(args.first);
  final port = ReceivePort();
  try {
    for (final path in args.skip(1)) {
      final input = File(path).readAsBytesSync();
      final doc = api.parse(input, lazy: true);
      try {
        input.fillRange(0, input.length, 0);
        equal(doc.toBytes(), File(path).readAsBytesSync());
        final h = doc.header;
        check(h.width > 0 && h.height > 0 && [8, 16, 32].contains(h.depth));
        final pixels = doc.decodeMerged();
        check(
          pixels.length == h.width * h.height * h.channels * (h.depth ~/ 8),
        );
        equal(doc.iccProfile(), doc.resource(1039));
        check(doc.resource(65535).isEmpty && doc.colorModeData().isEmpty);
        check(doc.info.mergedHasAlpha && doc.info.hasRealMergedData == true);
        check(
          doc.resolution ==
              (
                horizontal: 300.0,
                vertical: 300.0,
                horizontalUnit: 1,
                verticalUnit: 1,
                widthUnit: 1,
                heightUnit: 1,
              ),
        );
        check(
          doc.thumbnailInfo ==
              (
                format: 0,
                width: 2,
                height: 1,
                rowBytes: 8,
                resourceId: 1036,
                bgr: false,
              ),
        );
        equal(doc.thumbnailData(), [255, 0, 0, 255, 0, 0, 0, 0]);
        final floats = doc.mergedPixelsF32();
        check(
          floats.width == 2 &&
              floats.height == 1 &&
              floats.colorMode == rgb &&
              floats.channels == 4 &&
              floats.data.length == 8 &&
              floats.data[0] == 1 &&
              floats.data[5].abs() < 0.0001 &&
              (floats.data[7] - .5).abs() < .003,
        );
        final raster = h.layerCount == 3 ? 1 : 0;
        check(doc.channelCompression(raster, 0) == doc.info.compression);
        final renamed = doc.editLayer(raster, name: 'Ink 🎨');
        check(
          renamed.layer(raster).name == 'Ink 🎨' &&
              renamed.thumbnailInfo != null,
        );
        final replacement = Uint8List(doc.decodeChannel(raster, 0).length);
        if (h.depth == 32) {
          for (var i = 0; i < replacement.length; i += 4) {
            ByteData.sublistView(replacement).setFloat32(i, 2, Endian.big);
          }
        }
        final changed = renamed.replaceChannel(raster, 0, replacement);
        renamed.close();
        if (h.layerCount == 1) {
          final black = changed.editLayer(
            raster,
            name: 'Black',
            visible: false,
          );
          try {
            File('${File(path).parent.path}/edited-dart.psd')
                .writeAsBytesSync(black.toBytes());
          } finally {
            black.close();
          }
        }
        final cleared = changed.editLayer(raster, name: '', visible: false);
        changed.close();
        final reopened = api.parse(cleared.toBytes());
        cleared.close();
        try {
          check(
            reopened.layer(raster).name == '' &&
                !reopened.layerProperties(raster).visible &&
                reopened.info.hasRealMergedData == false &&
                reopened.thumbnailInfo == null &&
                reopened.thumbnailData().isEmpty,
          );
          equal(reopened.decodeChannel(raster, 0), replacement);
          check(
            reopened.layerPixelsF32(raster).data[0] == (h.depth == 32 ? 2 : 0),
          );
          fails<FormatException>(() => reopened.mergedPixelsF32());
          check(
            doc.layer(raster).name == 'Red ☺' &&
                doc.info.hasRealMergedData == true,
          );
        } finally {
          reopened.close();
        }
        fails<RangeError>(() => doc.editLayer(-1));
        fails<FormatException>(
          () => doc.replaceChannel(raster, 123, Uint8List(0)),
        );
        fails<FormatException>(
          () => doc.replaceChannel(raster, 0, Uint8List(0)),
        );
        fails<RangeError>(() => doc.layerPixelsF32(-1));
        fails<FormatException>(() => doc.channelCompression(raster, 123));
        final tree = doc.layerTree();
        if (h.layerCount == 3) {
          check(
            tree.length == 2 &&
                tree[0] ==
                    (index: 2, parent: null, divider: 0, isGroup: true) &&
                tree[1] == (index: 1, parent: 2, divider: null, isGroup: false),
          );
          final properties = doc.layerProperties(1), mask = doc.layerMask(1)!;
          equal(properties.blendMode, 'norm'.codeUnits);
          check(
            !properties.visible &&
                properties.opacity == 127 &&
                properties.fillOpacity == 128 &&
                properties.clipping == 1 &&
                properties.id != null,
          );
          check(
            mask.bounds == (top: 2, left: 0, bottom: 3, right: 1) &&
                mask.flags == 2 &&
                mask.realBounds == null &&
                mask.userDensity == null,
          );
        } else {
          check(tree.length == 1 && doc.layerMask(0) == null);
        }
        fails<FormatException>(() => doc.layerProperties(h.layerCount));
        fails<RangeError>(() => doc.layerMask(-1));
        for (var i = 0; i < h.layerCount; i++) {
          final layer = doc.layer(i);
          for (var j = 0; j < layer.channelCount; j++) {
            final id = doc.channelId(i, j), channel = doc.decodeChannel(i, id);
            check(
              doc.channelBounds(i, id) ==
                  (id == -2 ? doc.layerMask(i)!.bounds : layer.bounds),
            );
            if (id >= 0)
              check(
                channel.length ==
                    (layer.bounds.bottom - layer.bounds.top) *
                        (layer.bounds.right - layer.bounds.left) *
                        (h.depth ~/ 8),
              );
          }
          final p = doc.layerPreview(i, 1);
          final edge =
              layer.bounds.bottom == layer.bounds.top ||
                  layer.bounds.right == layer.bounds.left
              ? 0
              : 1;
          check(
            p.width == edge && p.height == edge && p.data.length == edge * 4,
            '$path layer $i: $p',
          );
        }
        final preview = doc.preview(1);
        check(
          preview.width == 1 && preview.height == 1 && preview.data.length == 4,
        );
        fails<ArgumentError>(() => port.sendPort.send(doc));
        fails<RangeError>(() => doc.layer(-1));
        fails<FormatException>(() => doc.layer(h.layerCount));
        fails<RangeError>(() => doc.channelId(0, -1));
        fails<FormatException>(() => doc.decodeChannel(0, 32767));
        fails<FormatException>(() => doc.preview(0));
        fails<RangeError>(() => doc.resource(65536));
        doc.close();
        doc.close();
        fails<StateError>(() => doc.header);
        check(
          pixels.isNotEmpty && preview.data.length == 4 && floats.data[0] == 1,
        );
        fails<StateError>(() => doc.mergedPixelsF32());
        fails<StateError>(() => doc.editLayer(raster));
      } finally {
        doc.close();
      }
    }
    final fixture = api.parse(File(args[1]).readAsBytesSync());
    final profile = fixture.iccProfile();
    fixture.close();
    for (var compression = raw; compression <= zipPrediction; compression++) {
      final builder = api.builder(1, 1, compression: compression);
      try {
        final pixel = Uint8List.fromList([255, 0, 0, 128]);
        const bounds = (top: -1, left: 2, bottom: 0, right: 3);
        builder.beginGroup('Folder');
        builder.addLayer(
          'Ink 🎨',
          bounds,
          pixel,
          options: const LayerOptions(
            maskBounds: (top: 0, left: 0, bottom: 1, right: 1),
          ),
          mask: Uint8List.fromList([128]),
        );
        fails<FormatException>(() => builder.build());
        builder.endGroup();
        builder.composite(pixel);
        builder.resource(
          1060,
          Uint8List.fromList([1, 2]),
          name: Uint8List.fromList([255]),
        );
        builder.resolution(300);
        builder.iccProfile(profile);
        final built = builder.build();
        final bytes = built.toBytes();
        built.close();
        builder.close();
        builder.close();
        fails<StateError>(() => builder.build());
        final doc = api.parse(bytes);
        try {
          check(
            doc.header.layerCount == 3 &&
                doc.layer(1).name == 'Ink 🎨' &&
                doc.layer(1).bounds == bounds,
          );
          equal(doc.layerPreview(1, 1).data, pixel);
          final preview = doc.preview(1);
          equal(preview.data, pixel);
          equal(preview.premultipliedRgba8(), [128, 0, 0, 128]);
          equal(preview.data, pixel);
          equal(doc.decodeChannel(1, -2), [128]);
          equal(doc.resource(1060), [1, 2]);
          check(doc.resource(1005).length == 16);
          equal(doc.iccProfile(), profile);
          check(
            doc.layerPreview(0, 1).data.isEmpty &&
                doc.layerPixelsF32(0).data.isEmpty &&
                doc.thumbnailInfo == null,
          );
        } finally {
          doc.close();
        }
      } finally {
        builder.close();
      }
    }
    final emptyBuilder = api.builder(1, 1), emptyDoc = emptyBuilder.build();
    check(emptyDoc.resolution == null && emptyDoc.thumbnailInfo == null);
    emptyDoc.close();
    emptyBuilder.close();
    final builder = api.builder(1, 1, depth: 16, colorMode: cmyk, version: 2);
    try {
      final pixel = Uint8List.fromList([
        0x12,
        0x34,
        0x56,
        0x78,
        0x9a,
        0xbc,
        0xde,
        0xf0,
        0xff,
        0xff,
      ]);
      builder.addLayer('Ink', (top: 0, left: 0, bottom: 1, right: 1), pixel);
      builder.composite(pixel);
      final doc = builder.build();
      try {
        check(
          doc.header.version == 2 &&
              doc.header.depth == 16 &&
              doc.header.colorMode == cmyk,
        );
        equal(doc.decodeMerged(), [
          0xed,
          0xcb,
          0xa9,
          0x87,
          0x65,
          0x43,
          0x21,
          0x0f,
        ]);
        final ink = doc.mergedPixelsF32();
        check(
          !doc.info.mergedHasAlpha &&
              ink.colorMode == cmyk &&
              ink.channels == 5 &&
              (ink.data[0] - 0x1234 / 65535).abs() < .000001 &&
              ink.data[4] == 1,
        );
        equal(doc.decodeChannel(0, 0), [0xed, 0xcb]);
      } finally {
        doc.close();
      }
      fails<ArgumentError>(() => port.sendPort.send(builder));
      fails<FormatException>(() => builder.endGroup());
      fails<FormatException>(() => builder.resolution(double.nan));
      fails<FormatException>(() => builder.composite(Uint8List(0)));
      fails<ArgumentError>(
        () => builder.addLayer(
          '',
          (top: 0, left: 0, bottom: 1, right: 1),
          pixel,
          options: const LayerOptions(blendMode: 'bad'),
        ),
      );
      fails<RangeError>(
        () => builder.addLayer(
          '',
          (top: 0, left: 0, bottom: 1, right: 1),
          pixel,
          options: const LayerOptions(opacity: 256),
        ),
      );
      fails<RangeError>(
        () => builder.addLayer('', (
          top: 0,
          left: 0,
          bottom: 1,
          right: 1 << 40,
        ), pixel),
      );
    } finally {
      builder.close();
    }
    equal(
      (
        width: 3,
        height: 1,
        data: Uint8List.fromList([1, 2, 3, 0, 1, 2, 3, 255, 255, 127, 1, 128]),
      ).premultipliedRgba8(),
      [0, 0, 0, 0, 1, 2, 3, 255, 128, 64, 1, 128],
    );
    fails<ArgumentError>(
      () => (width: 1, height: 1, data: Uint8List(1)).premultipliedRgba8(),
    );
    fails<FormatException>(() => api.parse(Uint8List(0)));
    fails<FormatException>(() => api.builder(0, 1));
    fails<RangeError>(() => api.builder(1 << 40, 1));
  } finally {
    port.close();
  }
  print('Dart PSD consumers passed (${args.length - 1} fixtures)');
}
