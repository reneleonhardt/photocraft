using PhotoCraft.Psd;
using System.Buffers.Binary;

static void Check(bool value) { if (!value) throw new Exception("PSD binding assertion failed."); }
static void Fails<T>(Action action) where T : Exception
{
    try { action(); } catch (T) { return; }
    throw new Exception($"Expected {typeof(T).Name}.");
}
foreach (var path in args)
{
    var input = File.ReadAllBytes(path);
    using var doc = Document.Parse(input, lazy: true);
    var header = doc.Header;
    Check(header.Width > 0 && header.Height > 0 && doc.ToBytes().SequenceEqual(input));
    var pixels = doc.DecodeMerged();
    Check(pixels.Length > 0 && doc.Resource(65535).Length == 0);
    Check(doc.IccProfile().SequenceEqual(doc.Resource(1039)));
    Check(doc.ColorModeData().Length == 0 && doc.Info.MergedHasAlpha == 1);
    Check(doc.Resolution is { Horizontal: 300, Vertical: 300, HorizontalUnit: 1, VerticalUnit: 1, WidthUnit: 1, HeightUnit: 1 });
    Check(doc.ThumbnailInfo is { Format: 0, Width: 2, Height: 1, RowBytes: 8, ResourceId: 1036, Bgr: 0 });
    Check(doc.ThumbnailData().SequenceEqual(new byte[] {255, 0, 0, 255, 0, 0, 0, 0}));
    var floats = doc.MergedPixelsF32();
    Check(floats.Width == 2 && floats.Height == 1 && floats.ColorMode == ColorMode.Rgb && floats.Channels == 4 && floats.Data.Length == 8);
    Check(floats.Data[0] == 1 && Math.Abs(floats.Data[5]) < 0.0001 && Math.Abs(floats.Data[7] - 0.5) < 0.003);
    nuint raster = header.LayerCount == 3 ? 1u : 0u;
    Check(doc.ChannelCompression(raster, 0) == doc.Info.Compression && doc.LayerPixelsF32(raster).Data[0] == 1);
    using (var renamed = doc.EditLayer(raster, "Ink 🎨"))
    {
        Check(renamed.LayerName(raster) == "Ink 🎨" && renamed.ThumbnailInfo is not null);
        var replacement = new byte[doc.DecodeChannel(raster, 0).Length];
        if (header.Depth == 32) for (var i = 0; i < replacement.Length; i += 4) BinaryPrimitives.WriteSingleBigEndian(replacement.AsSpan(i), 2);
        using var changed = renamed.ReplaceChannel(raster, 0, replacement);
        if (header.LayerCount == 1) { using var black = changed.EditLayer(raster, "Black", false); File.WriteAllBytes(Path.Combine(Path.GetDirectoryName(path)!, "edited-csharp.psd"), black.ToBytes()); }
        using var cleared = changed.EditLayer(raster, "", false);
        using var reopened = Document.Parse(cleared.ToBytes());
        Check(reopened.LayerName(raster) == "" && reopened.LayerProperties(raster).Visible == 0 && reopened.DecodeChannel(raster, 0).SequenceEqual(replacement));
        Check(reopened.Info.HasRealMergedData == 0 && reopened.ThumbnailInfo is null && reopened.ThumbnailData().Length == 0);
        Check(reopened.LayerPixelsF32(raster).Data[0] == (header.Depth == 32 ? 2 : 0));
        Fails<InvalidDataException>(() => reopened.MergedPixelsF32());
        Check(doc.LayerName(raster) == "Red ☺" && doc.Info.HasRealMergedData == 1);
    }
    Fails<InvalidDataException>(() => doc.EditLayer(nuint.MaxValue));
    Fails<InvalidDataException>(() => doc.ReplaceChannel(raster, 123, []));
    Fails<InvalidDataException>(() => doc.ReplaceChannel(raster, 0, []));
    Fails<InvalidDataException>(() => doc.LayerPixelsF32(nuint.MaxValue));
    Fails<InvalidDataException>(() => doc.ChannelCompression(raster, 123));
    var tree = doc.LayerTree();
    if (header.LayerCount == 3)
    {
        Check(tree.SequenceEqual(new TreeNode[] { new(2, null, 0, true), new(1, 2, null, false) }));
        var layer = doc.LayerProperties(1);
        Check(layer.Visible == 0 && layer.Opacity == 127 && layer.FillOpacity == 128 && layer.Clipping == 1 && layer.HasId == 1);
        Check(layer.BlendModeKey.SequenceEqual("norm"u8.ToArray()));
        Check(doc.LayerMask(1) is { Bounds: var bounds, Flags: 2 } && bounds == new Bounds(2, 0, 3, 1));
    }
    else Check(doc.LayerMask(0) is null && tree.Length == 1);
    Fails<InvalidDataException>(() => doc.LayerProperties(nuint.MaxValue));
    Fails<InvalidDataException>(() => doc.LayerMask(nuint.MaxValue));
    for (nuint layer = 0; layer < header.LayerCount; layer++)
    {
        var info = doc.LayerInfo(layer);
        Check(doc.LayerName(layer) is not null && info.Right >= info.Left && info.Bottom >= info.Top);
        for (nuint channel = 0; channel < info.ChannelCount; channel++) doc.DecodeChannel(layer, doc.ChannelId(layer, channel));
    }
    if (header.ColorMode == ColorMode.Rgb)
    {
        Check(doc.MergedRgba8().Length == header.Width * header.Height * 4);
        Check(doc.Preview(1).Width == 1);
        if (header.LayerCount != 0) { doc.LayerRgba8(0); doc.LayerPreview(0, 1); }
    }
    Parallel.For(0, 8, _ => { using var copy = doc.EditLayer(raster); Check(doc.DecodeMerged().SequenceEqual(pixels) && copy.MergedPixelsF32().Data[0] == 1); });
    Fails<InvalidDataException>(() => doc.LayerName(nuint.MaxValue));
    doc.Dispose(); doc.Dispose();
    Check(pixels.Length > 0 && floats.Data[0] == 1);
    Fails<ObjectDisposedException>(() => doc.DecodeMerged());
    Fails<ObjectDisposedException>(() => doc.MergedPixelsF32());
    Fails<ObjectDisposedException>(() => doc.EditLayer(raster));
}
Fails<InvalidDataException>(() => Document.Parse([]));
Fails<ArgumentNullException>(() => Document.Parse(null!));
Fails<InvalidDataException>(() => new Builder(0, 1));
foreach (var compression in Enum.GetValues<Compression>())
{
    using var builder = new Builder(1, 1, compression: compression);
    byte[] rgba = [255, 0, 0, 128];
    using (var empty = builder.Build()) Check(empty.Resolution is null && empty.ThumbnailInfo is null);
    builder.BeginGroup("Folder");
    builder.AddLayer("Ink 🎨", new(-1, 2, 0, 3), rgba, new(MaskBounds: new(0, 0, 1, 1)), [128]);
    Fails<InvalidDataException>(() => builder.Build());
    builder.EndGroup();
    builder.Composite(rgba);
    builder.Resource(1060, [1, 2], [255]);
    builder.Resolution(300);
    using var fixture = Document.Parse(File.ReadAllBytes(args[0]), lazy: true);
    var profile = fixture.IccProfile();
    builder.IccProfile(profile);
    using var created = builder.Build();
    using var parsed = Document.Parse(created.ToBytes());
    Check(parsed.Header.LayerCount == 3 && parsed.LayerName(1) == "Ink 🎨");
    Check(parsed.Info.Compression == compression && parsed.Info.MergedHasAlpha == 1);
    Check(parsed.MergedRgba8().SequenceEqual(rgba) && parsed.LayerRgba8(1).SequenceEqual(rgba));
    Check(parsed.DecodeChannel(1, -2).SequenceEqual(new byte[] {128}) && parsed.Resource(1060).SequenceEqual(new byte[] {1, 2}));
    Check(parsed.LayerPixelsF32(0).Data.Length == 0);
    Check(parsed.ChannelBounds(1, -2) == new Bounds(0, 0, 1, 1));
    Check(parsed.LayerProperties(2).SectionType == 1 && parsed.LayerMask(1)?.DefaultColor == 255);
    Check(parsed.IccProfile().SequenceEqual(profile) && parsed.Resource(1005).Length == 16);
    Fails<InvalidDataException>(() => parsed.Preview(0));
    Fails<ArgumentException>(() => builder.AddLayer("", new(), [], new(BlendMode: "bad")));
    Fails<InvalidDataException>(() => builder.AddLayer("", new(0, 0, 1, 1), []));
    builder.Dispose();
    Fails<ObjectDisposedException>(() => builder.Build());
}
using (var builder = new Builder(1, 1, depth: 16, colorMode: ColorMode.Cmyk, version: 2))
{
    byte[] pixel = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0xff, 0xff];
    Parallel.For(0, 8, i => builder.AddLayer(i.ToString(), new(0, 0, 1, 1), pixel));
    builder.Composite(pixel);
    using var built = builder.Build();
    using var doc = Document.Parse(built.ToBytes());
    Check(doc.Header.Version == 2 && doc.Header.Depth == 16 && doc.Header.Channels == 4 && doc.Header.LayerCount == 8);
    Check(doc.Info.MergedHasAlpha == 0 && doc.Info.HasRealMergedData is >= -1 and <= 1);
    var ink = doc.MergedPixelsF32();
    Check(ink.ColorMode == ColorMode.Cmyk && ink.Channels == 5 && Math.Abs(ink.Data[0] - 0x1234 / 65535f) < 0.000001 && ink.Data[4] == 1);
    Check(doc.DecodeChannel(0, 0).SequenceEqual(new byte[] {0xed, 0xcb}));
    Check(doc.DecodeMerged().SequenceEqual(new byte[] {0xed, 0xcb, 0xa9, 0x87, 0x65, 0x43, 0x21, 0x0f}));
}
Console.WriteLine("C# package consumer checks passed.");
