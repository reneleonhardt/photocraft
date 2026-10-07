using System.Runtime.InteropServices;
using System.Text;

namespace PhotoCraft.Psd;

internal static partial class Native
{
    private const string Library = "photocraft_psd_bindings";
    [StructLayout(LayoutKind.Sequential, Size = 256)] internal struct Error { }
    [StructLayout(LayoutKind.Sequential)] internal struct Buffer { internal nint Data; internal nuint Length; }
    [StructLayout(LayoutKind.Sequential)] internal struct Preview { internal uint Width, Height; internal Buffer Data; }
    [StructLayout(LayoutKind.Sequential)] internal struct FloatImage { internal uint Width, Height; internal ushort ColorMode, Channels; internal Buffer Data; }
    [StructLayout(LayoutKind.Sequential)] internal struct TreeNode { internal nuint Index, Parent, Divider; internal byte IsGroup; }
    [StructLayout(LayoutKind.Sequential)] internal readonly record struct Bytes(nint Data, nuint Length);
    [StructLayout(LayoutKind.Sequential)] internal readonly record struct Config(uint Width, uint Height, ushort ColorMode, ushort Depth, ushort Version, ushort Compression);
    [StructLayout(LayoutKind.Sequential)] internal struct Options
    {
        internal byte Blend0, Blend1, Blend2, Blend3;
        internal Bounds MaskBounds;
        internal byte Opacity, FillOpacity, Visible, Clipping, MaskDefault, MaskDisabled;
    }
    [StructLayout(LayoutKind.Sequential)] internal struct Layer
    {
        internal Bytes Name;
        internal Bounds Bounds;
        internal Bytes Pixels, Mask;
        internal Options Options;
    }

    internal sealed class Pinned : IDisposable
    {
        private GCHandle handle;
        internal Bytes Value { get; }
        internal Pinned(byte[]? data)
        {
            if (data is not null)
            {
                // A non-null empty name clears it; NULL keeps the existing name.
                handle = GCHandle.Alloc(data.Length == 0 ? new byte[1] : data, GCHandleType.Pinned);
                Value = new(handle.AddrOfPinnedObject(), (nuint)data.Length);
            }
        }
        public void Dispose() { if (handle.IsAllocated) handle.Free(); }
    }

    internal static void CheckAbi()
    {
        if (AbiVersion() != 1) throw new NotSupportedException("Unsupported PhotoCraft PSD C ABI.");
    }
    internal static void Check(int status, Error error)
    {
        if (status == 0) return;
        var bytes = MemoryMarshal.AsBytes(MemoryMarshal.CreateReadOnlySpan(ref error, 1));
        var end = bytes.IndexOf((byte)0);
        var message = Encoding.UTF8.GetString(end < 0 ? bytes : bytes[..end]);
        throw status == 1 ? new ArgumentException(message) : new InvalidDataException(message);
    }
    internal static byte[] Copy(Buffer buffer)
    {
        try
        {
            var result = new byte[checked((int)buffer.Length)];
            if (result.Length != 0) Marshal.Copy(buffer.Data, result, 0, result.Length);
            return result;
        }
        finally { BufferFree(ref buffer); }
    }

    internal static PhotoCraft.Psd.FloatImage Copy(FloatImage image)
    {
        try
        {
            var data = new float[checked((int)image.Data.Length)];
            if (data.Length != 0) Marshal.Copy(image.Data.Data, data, 0, data.Length);
            return new(image.Width, image.Height, (ColorMode)image.ColorMode, image.Channels, data);
        }
        finally { FloatBufferFree(ref image.Data); }
    }

    [LibraryImport(Library, EntryPoint = "pc_psd_float_buffer_free")] internal static partial void FloatBufferFree(ref Buffer buffer);
    [LibraryImport(Library, EntryPoint = "pc_psd_merged_pixels_f32")] internal static partial int MergedPixelsF32(DocumentHandle handle, out FloatImage result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_pixels_f32")] internal static partial int LayerPixelsF32(DocumentHandle handle, nuint layer, out FloatImage result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_edit_layer")] internal static partial int EditLayer(DocumentHandle handle, nuint layer, Bytes name, sbyte visible, out DocumentHandle result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_replace_channel")] internal static partial int ReplaceChannel(DocumentHandle handle, nuint layer, short id, Bytes data, out DocumentHandle result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_channel_compression")] internal static partial int ChannelCompression(DocumentHandle handle, nuint layer, short id, out int result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_resolution")] internal static partial int GetResolution(DocumentHandle handle, out ResolutionInfo result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_thumbnail_info")] internal static partial int ThumbnailInfo(DocumentHandle handle, out ThumbnailInfo result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_thumbnail_data")] internal static partial int ThumbnailData(DocumentHandle handle, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_abi_version")] internal static partial uint AbiVersion();
    [LibraryImport(Library, EntryPoint = "pc_psd_parse")] internal static partial int Parse(nint data, nuint length, out DocumentHandle result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_parse_lazy")] internal static partial int ParseLazy(nint data, nuint length, out DocumentHandle result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_free")] internal static partial void Free(nint handle);
    [LibraryImport(Library, EntryPoint = "pc_psd_buffer_free")] internal static partial void BufferFree(ref Buffer buffer);
    [LibraryImport(Library, EntryPoint = "pc_psd_header")] internal static partial int Header(DocumentHandle handle, out Header result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_document_info")] internal static partial int Info(DocumentHandle handle, out DocumentInfo result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_info")] internal static partial int LayerInfo(DocumentHandle handle, nuint layer, out LayerInfo result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_name")] internal static partial int LayerName(DocumentHandle handle, nuint layer, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_properties")] internal static partial int Properties(DocumentHandle handle, nuint layer, out LayerProperties result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_mask")] internal static partial int Mask(DocumentHandle handle, nuint layer, out MaskInfo result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_tree")] internal static partial int Tree(DocumentHandle handle, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_tree_free")] internal static partial void TreeFree(ref Buffer buffer);
    [LibraryImport(Library, EntryPoint = "pc_psd_channel_id")] internal static partial int ChannelId(DocumentHandle handle, nuint layer, nuint channel, out short result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_channel_bounds")] internal static partial int ChannelBounds(DocumentHandle handle, nuint layer, short id, out Bounds result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_decode_merged")] internal static partial int DecodeMerged(DocumentHandle handle, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_decode_channel")] internal static partial int DecodeChannel(DocumentHandle handle, nuint layer, short id, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_icc_profile")] internal static partial int IccProfile(DocumentHandle handle, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_color_mode_data")] internal static partial int ColorModeData(DocumentHandle handle, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_resource")] internal static partial int Resource(DocumentHandle handle, ushort id, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_serialize")] internal static partial int Serialize(DocumentHandle handle, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_merged_rgba8")] internal static partial int MergedRgba8(DocumentHandle handle, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_rgba8")] internal static partial int LayerRgba8(DocumentHandle handle, nuint layer, out Buffer result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_preview")] internal static partial int GetPreview(DocumentHandle handle, uint maxEdge, out Preview result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_layer_preview")] internal static partial int GetLayerPreview(DocumentHandle handle, nuint layer, uint maxEdge, out Preview result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_new")] internal static partial int BuilderNew(Config config, out BuilderHandle result, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_free")] internal static partial void BuilderFree(nint handle);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_add_layer")] internal static partial int AddLayer(BuilderHandle handle, in Layer layer, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_composite")] internal static partial int Composite(BuilderHandle handle, Bytes pixels, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_begin_group")] internal static partial int BeginGroup(BuilderHandle handle, Bytes name, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_end_group")] internal static partial int EndGroup(BuilderHandle handle, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_resource")] internal static partial int AddResource(BuilderHandle handle, ushort id, Bytes name, Bytes data, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_resolution")] internal static partial int Resolution(BuilderHandle handle, double dpi, out Error error);
    [LibraryImport(Library, EntryPoint = "pc_psd_builder_build")] internal static partial int Build(BuilderHandle handle, out DocumentHandle result, out Error error);
}

internal sealed class DocumentHandle : SafeHandle
{
    public DocumentHandle() : base(0, true) { }
    public override bool IsInvalid => handle == 0;
    protected override bool ReleaseHandle() { Native.Free(handle); return true; }
}
internal sealed class BuilderHandle : SafeHandle
{
    public BuilderHandle() : base(0, true) { }
    public override bool IsInvalid => handle == 0;
    protected override bool ReleaseHandle() { Native.BuilderFree(handle); return true; }
}
