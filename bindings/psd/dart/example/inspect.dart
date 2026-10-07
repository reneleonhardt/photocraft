import 'dart:io';

import 'package:photocraft_psd/photocraft_psd.dart';

void main(List<String> args) {
  try {
    if (args.length != 3)
      throw ArgumentError('usage: inspect <library> <input.psd> <output.psd>');
    final doc = PsdLibrary.open(args[0])
        .parse(File(args[1]).readAsBytesSync(), lazy: true);
    try {
      final h = doc.header, image = doc.preview(256);
      print(
        '${h.width} × ${h.height}, ${h.depth}-bit, ${h.layerCount} layers; preview ${image.width} × ${image.height}',
      );
      File(args[2]).writeAsBytesSync(doc.toBytes());
    } finally {
      doc.close();
    }
  } catch (error) {
    stderr.writeln(error);
    exitCode = 1;
  }
}
