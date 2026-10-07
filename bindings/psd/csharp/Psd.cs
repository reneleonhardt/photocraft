using System.Runtime.InteropServices;
using System.Text;

namespace PhotoCraft.Psd;

public enum Compression : ushort { Raw, Rle, Zip, ZipPrediction }
public enum ColorMode : ushort { Bitmap, Grayscale, Indexed, Rgb, Cmyk, Multichannel = 7, Duotone, Lab }
[StructLayout(LayoutKind.Sequential)] public readonly record struct Bounds(int Top, int Left, int Bottom, int Right);
[StructLayout(LayoutKind.Sequential)] public readonly record struct DocumentInfo(Compression Compression, byte MergedHasAlpha, sbyte HasRealMergedData);
[StructLayout(LayoutKind.Sequential)] public readonly struct Header
{
    public readonly uint Width, Height;
    public readonly ushort Version, Channels, Depth;
    public readonly ColorMode ColorMode;
    public readonly nuint LayerCount;
}
[StructLayout(LayoutKind.Sequential)] public readonly struct LayerInfo
{
    public readonly int Top, Left, Bottom, Right;
    public readonly nuint ChannelCount;
}
public readonly record struct FloatImage(uint Width, uint Height, ColorMode ColorMode, ushort Channels, float[] Data);
[StructLayout(LayoutKind.Sequential)] public readonly struct ResolutionInfo
{
    public readonly double Horizontal, Vertical;
    public readonly ushort HorizontalUnit, VerticalUnit, WidthUnit, HeightUnit;
    public readonly byte Present;
}
[StructLayout(LayoutKind.Sequential)] public readonly struct ThumbnailInfo
{
    public readonly uint Format, Width, Height, RowBytes;
    public readonly ushort ResourceId;
    public readonly byte Present, Bgr;
}
public readonly record struct Preview(uint Width, uint Height, byte[] Data);
[StructLayout(LayoutKind.Sequential)] public readonly struct LayerProperties
{
    private readonly byte blend0, blend1, blend2, blend3;
    public readonly uint SectionType, Id;
    public readonly byte HasId, Visible, Opacity, FillOpacity, Clipping;
    public byte[] BlendModeKey => [blend0, blend1, blend2, blend3];
}
[StructLayout(LayoutKind.Sequential)] public readonly struct MaskInfo
{
    public readonly Bounds Bounds, RealBounds;
    public readonly double UserFeather, VectorFeather;
    public readonly byte Present, Flags, DefaultColor, Parameters, UserDensity, VectorDensity, HasReal, RealFlags, RealBackground;
}
public readonly record struct TreeNode(nuint Index, nuint? Parent, nuint? Divider, bool IsGroup);
public sealed record LayerOptions(
    string BlendMode = "norm", byte Opacity = 255, byte FillOpacity = 255, bool Visible = true,
    bool Clipping = false, Bounds MaskBounds = default, byte MaskDefault = 255, bool MaskDisabled = false);

public sealed class Document : IDisposable
{
    private readonly DocumentHandle handle;
    internal Document(DocumentHandle handle) { this.handle = handle; }
    public static Document Parse(byte[] data, bool lazy = false)
    {
        ArgumentNullException.ThrowIfNull(data);
        Native.CheckAbi();
        using var input = new Native.Pinned(data);
        DocumentHandle result;
        Native.Error error;
        Native.Check(lazy ? Native.ParseLazy(input.Value.Data, input.Value.Length, out result, out error)
            : Native.Parse(input.Value.Data, input.Value.Length, out result, out error), error);
        return new(result);
    }
    public void Dispose() => handle.Dispose();
    public Header Header { get { Native.Check(Native.Header(handle, out var value, out var error), error); return value; } }
    public DocumentInfo Info { get { Native.Check(Native.Info(handle, out var value, out var error), error); return value; } }
    public LayerInfo LayerInfo(nuint layer) { Native.Check(Native.LayerInfo(handle, layer, out var value, out var error), error); return value; }
    public string LayerName(nuint layer) { Native.Check(Native.LayerName(handle, layer, out var value, out var error), error); return Encoding.UTF8.GetString(Native.Copy(value)); }
    public LayerProperties LayerProperties(nuint layer) { Native.Check(Native.Properties(handle, layer, out var value, out var error), error); return value; }
    public MaskInfo? LayerMask(nuint layer) { Native.Check(Native.Mask(handle, layer, out var value, out var error), error); return value.Present == 0 ? null : value; }
    public TreeNode[] LayerTree()
    {
        Native.Check(Native.Tree(handle, out var value, out var error), error);
        try
        {
            var nodes = new TreeNode[checked((int)value.Length)];
            for (var i = 0; i < nodes.Length; i++)
            {
                var n = Marshal.PtrToStructure<Native.TreeNode>(value.Data + checked(i * Marshal.SizeOf<Native.TreeNode>()));
                nodes[i] = new(n.Index, n.Parent == nuint.MaxValue ? null : n.Parent, n.Divider == nuint.MaxValue ? null : n.Divider, n.IsGroup != 0);
            }
            return nodes;
        }
        finally { Native.TreeFree(ref value); }
    }
    public short ChannelId(nuint layer, nuint channel) { Native.Check(Native.ChannelId(handle, layer, channel, out var value, out var error), error); return value; }
    public Bounds ChannelBounds(nuint layer, short id) { Native.Check(Native.ChannelBounds(handle, layer, id, out var value, out var error), error); return value; }
    public Compression? ChannelCompression(nuint layer, short id) { Native.Check(Native.ChannelCompression(handle, layer, id, out var value, out var error), error); return value < 0 ? null : (Compression)value; }
    public ResolutionInfo? Resolution { get { Native.Check(Native.GetResolution(handle, out var value, out var error), error); return value.Present == 0 ? null : value; } }
    public ThumbnailInfo? ThumbnailInfo { get { Native.Check(Native.ThumbnailInfo(handle, out var value, out var error), error); return value.Present == 0 ? null : value; } }
    public byte[] ThumbnailData() { Native.Check(Native.ThumbnailData(handle, out var value, out var error), error); return Native.Copy(value); }
    public FloatImage MergedPixelsF32() { Native.Check(Native.MergedPixelsF32(handle, out var value, out var error), error); return Native.Copy(value); }
    public FloatImage LayerPixelsF32(nuint layer) { Native.Check(Native.LayerPixelsF32(handle, layer, out var value, out var error), error); return Native.Copy(value); }
    public Document EditLayer(nuint layer, string? name = null, bool? visible = null)
    {
        using var input = new Native.Pinned(name is null ? null : Encoding.UTF8.GetBytes(name));
        Native.Check(Native.EditLayer(handle, layer, input.Value, visible is null ? (sbyte)-1 : (sbyte)(visible.Value ? 1 : 0), out var result, out var error), error);
        return new(result);
    }
    public Document ReplaceChannel(nuint layer, short id, byte[] data)
    {
        ArgumentNullException.ThrowIfNull(data);
        using var input = new Native.Pinned(data);
        Native.Check(Native.ReplaceChannel(handle, layer, id, input.Value, out var result, out var error), error);
        return new(result);
    }
    public byte[] DecodeMerged() { Native.Check(Native.DecodeMerged(handle, out var value, out var error), error); return Native.Copy(value); }
    public byte[] DecodeChannel(nuint layer, short id) { Native.Check(Native.DecodeChannel(handle, layer, id, out var value, out var error), error); return Native.Copy(value); }
    public byte[] IccProfile() { Native.Check(Native.IccProfile(handle, out var value, out var error), error); return Native.Copy(value); }
    public byte[] ColorModeData() { Native.Check(Native.ColorModeData(handle, out var value, out var error), error); return Native.Copy(value); }
    public byte[] Resource(ushort id) { Native.Check(Native.Resource(handle, id, out var value, out var error), error); return Native.Copy(value); }
    public byte[] ToBytes() { Native.Check(Native.Serialize(handle, out var value, out var error), error); return Native.Copy(value); }
    public byte[] MergedRgba8() { Native.Check(Native.MergedRgba8(handle, out var value, out var error), error); return Native.Copy(value); }
    public byte[] LayerRgba8(nuint layer) { Native.Check(Native.LayerRgba8(handle, layer, out var value, out var error), error); return Native.Copy(value); }
    public Preview Preview(uint maxEdge) { Native.Check(Native.GetPreview(handle, maxEdge, out var value, out var error), error); return new(value.Width, value.Height, Native.Copy(value.Data)); }
    public Preview LayerPreview(nuint layer, uint maxEdge) { Native.Check(Native.GetLayerPreview(handle, layer, maxEdge, out var value, out var error), error); return new(value.Width, value.Height, Native.Copy(value.Data)); }
}

public sealed class Builder : IDisposable
{
    private readonly BuilderHandle handle;
    private readonly Lock gate = new();
    public Builder(uint width, uint height, ushort depth = 8, ColorMode colorMode = ColorMode.Rgb, ushort version = 1, Compression compression = Compression.Rle)
    {
        Native.CheckAbi();
        Native.Check(Native.BuilderNew(new(width, height, (ushort)colorMode, depth, version, (ushort)compression), out handle, out var error), error);
    }
    public void Dispose() { lock (gate) handle.Dispose(); }
    public void AddLayer(string name, Bounds bounds, byte[] pixels, LayerOptions? options = null, byte[]? mask = null)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(pixels);
        options ??= new();
        var blend = Encoding.UTF8.GetBytes(options.BlendMode);
        if (blend.Length != 4) throw new ArgumentException("Blend mode must contain four UTF-8 bytes.", nameof(options));
        using var n = new Native.Pinned(Encoding.UTF8.GetBytes(name));
        using var p = new Native.Pinned(pixels);
        using var m = new Native.Pinned(mask);
        var layer = new Native.Layer {
            Name = n.Value, Bounds = bounds, Pixels = p.Value, Mask = m.Value,
            Options = new() {
                Blend0 = blend[0], Blend1 = blend[1], Blend2 = blend[2], Blend3 = blend[3], MaskBounds = options.MaskBounds,
                Opacity = options.Opacity, FillOpacity = options.FillOpacity, Visible = (byte)(options.Visible ? 1 : 0),
                Clipping = (byte)(options.Clipping ? 1 : 0), MaskDefault = options.MaskDefault, MaskDisabled = (byte)(options.MaskDisabled ? 1 : 0)
            }
        };
        lock (gate) Native.Check(Native.AddLayer(handle, in layer, out var error), error);
    }
    public void Composite(byte[] pixels)
    {
        ArgumentNullException.ThrowIfNull(pixels);
        using var input = new Native.Pinned(pixels);
        lock (gate) Native.Check(Native.Composite(handle, input.Value, out var error), error);
    }
    public void BeginGroup(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        using var input = new Native.Pinned(Encoding.UTF8.GetBytes(name));
        lock (gate) Native.Check(Native.BeginGroup(handle, input.Value, out var error), error);
    }
    public void EndGroup() { lock (gate) Native.Check(Native.EndGroup(handle, out var error), error); }
    public void Resource(ushort id, byte[] data, byte[]? name = null)
    {
        ArgumentNullException.ThrowIfNull(data);
        using var d = new Native.Pinned(data);
        using var n = new Native.Pinned(name);
        lock (gate) Native.Check(Native.AddResource(handle, id, n.Value, d.Value, out var error), error);
    }
    public void IccProfile(byte[] data) => Resource(1039, data);
    public void Resolution(double dpi) { lock (gate) Native.Check(Native.Resolution(handle, dpi, out var error), error); }
    public Document Build() { lock (gate) { Native.Check(Native.Build(handle, out var result, out var error), error); return new(result); } }
}
