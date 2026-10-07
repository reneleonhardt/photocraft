//! Small owned metadata values; large pixel/resource payloads remain byte buffers.
macro_rules! metadata {
    ($(#[$attr:meta])* $name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        $(#[$attr])*
        #[derive(Clone, Debug, Default, PartialEq)]
        #[cfg_attr(all(feature = "python", not(target_arch = "wasm32")), derive(pyo3::IntoPyObject))]
        #[cfg_attr(all(feature = "javascript", target_arch = "wasm32"), derive(serde::Serialize, tsify::Tsify))]
        #[cfg_attr(all(feature = "javascript", target_arch = "wasm32"), serde(rename_all = "camelCase"))]
        #[cfg_attr(all(feature = "javascript", target_arch = "wasm32"), tsify(missing_as_null))]
        pub struct $name { $(pub $field: $ty),* }
    };
}

metadata!(Bounds { top: i32, left: i32, bottom: i32, right: i32 });
impl From<photocraft_psd::Rect> for Bounds {
    fn from(r: photocraft_psd::Rect) -> Self {
        Self { top: r.top, left: r.left, bottom: r.bottom, right: r.right }
    }
}
metadata!(HeaderInfo {
    width: u32, height: u32, version: u16, channels: u16, depth: u16,
    color_mode: u16, layer_count: usize, compression: u16,
    merged_has_alpha: bool, has_real_merged_data: Option<bool>,
});
metadata!(LayerMetadata {
    name: String, bounds: Bounds, channel_count: usize, blend_mode: String, blend_mode_key: [u8; 4],
    opacity: u8, fill_opacity: u8, clipping: u8, visible: bool,
    section_type: u32, id: Option<u32>, mask: Option<MaskInfo>,
});
metadata!(MaskInfo {
    bounds: Bounds, default_color: u8, flags: u8,
    user_density: Option<u8>, user_feather: Option<f64>,
    vector_density: Option<u8>, vector_feather: Option<f64>,
    real_bounds: Option<Bounds>, real_flags: Option<u8>, real_background: Option<u8>,
});
metadata!(ResolutionInfo { horizontal: f64, vertical: f64, horizontal_unit: u16, vertical_unit: u16, width_unit: u16, height_unit: u16 });
metadata!(TreeNode { index: usize, parent: Option<usize>, divider: Option<usize>, is_group: bool });
metadata!(
    #[repr(C)]
    ResourceInfo { signature: [u8; 4], id: u16, name_len: usize, data_len: usize }
);
metadata!(
    #[repr(C)]
    BlockInfo { signature: [u8; 4], key: [u8; 4], data_len: usize }
);
metadata!(ThumbnailInfo { resource_id: u16, format: u32, width: u32, height: u32, row_bytes: u32, bgr: bool });
metadata!(
    #[repr(C)]
    PathInfo { subpath_count: usize, version: u32, flags: u32, initial_fill: bool, is_vector_mask: bool }
);
metadata!(
    #[repr(C)]
    SubpathInfo { knot_count: usize, operation: i16, closed: bool }
);
metadata!(
    #[repr(C)]
    KnotInfo { pre: [f64; 2], anchor: [f64; 2], post: [f64; 2], linked: bool }
);
metadata!(
    #[repr(C)]
    SlicesInfo { bounds: [i32; 4], count: usize, version: u32 }
);
metadata!(
    #[repr(C)]
    SliceInfo {
        id: u32,
        group_id: u32,
        origin: u32,
        layer_id: u32,
        kind: u32,
        bounds: [i32; 4],
        outsets: [i32; 4],
        horizontal_align: u32,
        vertical_align: u32,
        color: [u8; 4],
        has_layer_id: bool,
        cell_text_is_html: bool,
    }
);
