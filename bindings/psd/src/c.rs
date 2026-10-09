//! Native C ABI. See include/photocraft_psd.h for ownership and pointer contracts.
#![allow(unsafe_code)]

use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use crate::{
    BlockInfo, Builder, BuilderConfig, Document, Header, KnotInfo, LayerInfo, Path, PathInfo, ResourceInfo, SliceInfo, Slices, SlicesInfo, SubpathInfo,
};

#[repr(C)]
#[derive(Default)]
pub struct DocumentInfo {
    pub compression: u16,
    pub merged_has_alpha: u8,
    pub has_real_merged_data: i8,
}
#[repr(C)]
#[derive(Default)]
pub struct LayerProperties {
    pub blend_mode: [u8; 4],
    pub section_type: u32,
    pub id: u32,
    pub has_id: u8,
    pub visible: u8,
    pub opacity: u8,
    pub fill_opacity: u8,
    pub clipping: u8,
}
#[repr(C)]
#[derive(Default)]
pub struct Mask {
    pub bounds: [i32; 4],
    pub real_bounds: [i32; 4],
    pub user_feather: f64,
    pub vector_feather: f64,
    pub present: u8,
    pub flags: u8,
    pub default_color: u8,
    pub parameters: u8,
    pub user_density: u8,
    pub vector_density: u8,
    pub has_real: u8,
    pub real_flags: u8,
    pub real_background: u8,
}
#[repr(C)]
#[derive(Default)]
pub struct Resolution {
    pub horizontal: f64,
    pub vertical: f64,
    pub horizontal_unit: u16,
    pub vertical_unit: u16,
    pub width_unit: u16,
    pub height_unit: u16,
    pub present: u8,
}
#[repr(C)]
pub struct TreeNode {
    pub index: usize,
    pub parent: usize,
    pub divider: usize,
    pub is_group: u8,
}
pub type Tree = Buffer<TreeNode>;

#[repr(C)]
#[derive(Default)]
pub struct Thumbnail {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub row_bytes: u32,
    pub resource_id: u16,
    pub present: u8,
    pub bgr: u8,
}
#[repr(C)]
#[derive(Default)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub data: Buffer,
}

#[repr(C)]
#[derive(Default)]
pub struct FloatImage {
    pub width: u32,
    pub height: u32,
    pub color_mode: u16,
    pub channels: u16,
    pub data: Buffer<f32>,
}

impl From<crate::FloatImage> for FloatImage {
    fn from(image: crate::FloatImage) -> Self {
        Self { width: image.width, height: image.height, color_mode: image.color_mode, channels: image.channels, data: buffer(image.data) }
    }
}

fn bounds(b: crate::Bounds) -> [i32; 4] {
    [b.top, b.left, b.bottom, b.right]
}

pub const OK: i32 = 0;
pub const INVALID_ARGUMENT: i32 = 1;
pub const ERROR: i32 = 2;
pub const PANIC: i32 = 3;

#[repr(C)]
pub struct Buffer<T = u8> {
    pub data: *mut T,
    pub len: usize,
}

impl<T> Default for Buffer<T> {
    fn default() -> Self {
        Self { data: ptr::null_mut(), len: 0 }
    }
}

#[repr(C)]
pub struct Error {
    pub message: [c_char; 256],
}

struct Failure(i32, String);
impl From<photocraft_psd::PsdError> for Failure {
    fn from(e: photocraft_psd::PsdError) -> Self {
        Self(ERROR, e.to_string())
    }
}

fn valid<T>(p: *const T) -> bool {
    !p.is_null() && (p as usize).is_multiple_of(std::mem::align_of::<T>())
}

unsafe fn handle<'a, T>(p: *const T) -> Result<&'a T, Failure> {
    if !valid(p) {
        return Err(Failure(INVALID_ARGUMENT, "handle is null or misaligned".into()));
    }
    // SAFETY: the C contract requires a live value of the matching type.
    Ok(unsafe { &*p })
}

unsafe fn bytes<'a>(p: *const u8, len: usize) -> Result<&'a [u8], Failure> {
    if len > isize::MAX as usize || (p.is_null() && len != 0) {
        return Err(Failure(INVALID_ARGUMENT, "invalid input pointer or length".into()));
    }
    if len == 0 {
        return Ok(&[]);
    }
    // SAFETY: the C contract requires len readable bytes in one live allocation.
    Ok(unsafe { std::slice::from_raw_parts(p, len) })
}

fn buffer<T>(data: Vec<T>) -> Buffer<T> {
    if data.is_empty() {
        return Buffer::default();
    }
    let data = data.into_boxed_slice();
    let len = data.len();
    Buffer { data: Box::into_raw(data) as *mut T, len }
}

unsafe fn report(error: *mut Error, message: &str) {
    if error.is_null() {
        return;
    }
    let mut value = Error { message: [0; 256] };
    let mut len = message.len().min(value.message.len() - 1);
    while !message.is_char_boundary(len) {
        len -= 1;
    }
    for (out, byte) in value.message.iter_mut().zip(message.bytes().take(len)) {
        *out = byte as c_char;
    }
    // SAFETY: the caller provides non-overlapping writable storage.
    unsafe { error.write(value) };
}

unsafe fn run<T: Default>(out: *mut T, error: *mut Error, f: impl FnOnce() -> Result<T, Failure>) -> i32 {
    if !valid(out) {
        // SAFETY: error is null or valid writable storage under the C contract.
        unsafe { report(error, "output is null or misaligned") };
        return INVALID_ARGUMENT;
    }
    // SAFETY: out is aligned writable storage under the C contract. Reset on failure too.
    unsafe { out.write(T::default()) };
    let result = catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(Failure(PANIC, "unexpected Rust panic".into())));
    match result {
        Ok(value) => {
            // SAFETY: the caller keeps out/error writable and non-overlapping for this call.
            unsafe {
                out.write(value);
                report(error, "")
            };
            OK
        }
        Err(Failure(code, message)) => {
            // SAFETY: error is null or valid writable storage under the C contract.
            unsafe { report(error, &message) };
            code
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_psd_abi_version() -> u32 {
    1
}

macro_rules! export {
    ($handle:ty; $name:ident($($arg:ident: $ty:ty),*) -> $out:ty, |$doc:ident| $body:expr) => {
        export!($name($doc: *const $handle $(, $arg: $ty)*) -> $out, { let $doc = handle($doc)?; $body });
    };
    ($name:ident($($arg:ident: $ty:ty),*) -> $out:ty, |$doc:ident| $body:expr) => {
        export!(Document; $name($($arg: $ty),*) -> $out, |$doc| $body);
    };
    ($name:ident($($arg:ident: $ty:ty),*) -> $out:ty, $body:expr) => {
        /// # Safety
        /// See the header's shared pointer and ownership contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $ty,)* out: *mut $out, error: *mut Error) -> i32 {
            // SAFETY: pointers obey the shared C contract; helpers check null/alignment/length.
            unsafe { run(out, error, || $body) }
        }
    };
}

export!(pc_psd_parse(data: *const u8, len: usize) -> *mut Document, Ok(Box::into_raw(Box::new(Document::parse(bytes(data, len)?)?))));
export!(pc_psd_parse_lazy(data: *const u8, len: usize) -> *mut Document, Ok(Box::into_raw(Box::new(Document::parse_lazy(bytes(data, len)?)?))));
export!(pc_psd_builder_new(config: BuilderConfig) -> *mut Builder, Ok(Box::into_raw(Box::new(Builder::new(config)?))));

macro_rules! free_buffer {
    ($name:ident, $ty:ty) => {
        /// # Safety
        /// value is null or writable initialized storage holding an unmodified returned buffer.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(value: *mut Buffer<$ty>) {
            if value.is_null() {
                return;
            }
            // SAFETY: initialized storage is reset so repeated cleanup of this value is harmless.
            let value = unsafe { std::mem::take(&mut *value) };
            if !value.data.is_null() {
                // SAFETY: data/len are the original returned boxed slice allocation.
                drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(value.data, value.len)) });
            }
        }
    };
}
free_buffer!(pc_psd_buffer_free, u8);
free_buffer!(pc_psd_float_buffer_free, f32);
free_buffer!(pc_psd_tree_free, TreeNode);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bytes {
    pub data: *const u8,
    pub len: usize,
}
#[repr(C)]
pub struct LayerOptions {
    pub blend_mode: [u8; 4],
    pub mask_bounds: [i32; 4],
    pub opacity: u8,
    pub fill_opacity: u8,
    pub visible: u8,
    pub clipping: u8,
    pub mask_default: u8,
    pub mask_disabled: u8,
}
#[repr(C)]
pub struct LayerInput {
    pub name: Bytes,
    pub bounds: [i32; 4],
    pub pixels: Bytes,
    pub mask: Bytes,
    pub options: LayerOptions,
}

unsafe fn input<'a>(value: Bytes) -> Result<&'a [u8], Failure> {
    // SAFETY: borrowed inputs obey the header's shared pointer contract.
    unsafe { bytes(value.data, value.len) }
}
fn text(data: &[u8]) -> Result<&str, Failure> {
    std::str::from_utf8(data).map_err(|_| Failure(INVALID_ARGUMENT, "text must be UTF-8".into()))
}
macro_rules! free_handle {
    ($name:ident,$ty:ty) => {
        /// # Safety
        /// The handle is null or live, uniquely owned and freed exactly once.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(value: *mut $ty) {
            if !value.is_null() {
                // SAFETY: the caller transfers the original Box exactly once.
                drop(unsafe { Box::from_raw(value) });
            }
        }
    };
}
free_handle!(pc_psd_free, Document);
free_handle!(pc_psd_path_free, Path);
free_handle!(pc_psd_slices_free, Slices);
free_handle!(pc_psd_builder_free, Builder);
export!(pc_psd_resource_path(index: usize) -> *mut Path, |doc| Ok(doc.resource_path(index)?.map_or(ptr::null_mut(), |p| Box::into_raw(Box::new(p)))));
export!(pc_psd_vector_mask(layer: usize) -> *mut Path, |doc| Ok(doc.vector_mask(layer)?.map_or(ptr::null_mut(), |p| Box::into_raw(Box::new(p)))));
export!(pc_psd_slices() -> *mut Slices, |doc| Ok(doc.slices()?.map_or(ptr::null_mut(), |s| Box::into_raw(Box::new(s)))));
export!(Path; pc_psd_path_info() -> PathInfo, |path| Ok(path.info()));
export!(Path; pc_psd_subpath_info(index: usize) -> SubpathInfo, |path| Ok(path.subpath_info(index)?));
export!(Path; pc_psd_knot(subpath: usize,index: usize) -> KnotInfo, |path| Ok(path.knot(subpath,index)?));
export!(Slices; pc_psd_slices_info() -> SlicesInfo, |slices| Ok(slices.info()));
export!(Slices; pc_psd_slices_group_name() -> Buffer, |slices| Ok(buffer(slices.group_name().as_bytes().to_vec())));
export!(Slices; pc_psd_slice_info(index: usize) -> SliceInfo, |slices| Ok(slices.slice_info(index)?));
export!(Slices; pc_psd_slice_text(index: usize,field: Bytes) -> Buffer, |slices| Ok(buffer(slices.text(index,text(input(field)?)?)?.as_bytes().to_vec())));
export!(Builder; pc_psd_builder_build() -> *mut Document, |builder| Ok(Box::into_raw(Box::new(builder.build()?))));

macro_rules! mutate {
    ($name:ident($($arg:ident: $ty:ty),*), |$builder:ident| $body:expr) => {
        /// # Safety
        /// The builder has exclusive access; borrowed inputs and error obey the shared pointer contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(value: *mut Builder,$($arg: $ty,)* error: *mut Error) -> i32 {
            // SAFETY: the caller grants an exclusive borrow for this call; null/alignment are checked.
            unsafe { run(&mut (),error,|| {
                if !valid(value) { return Err(Failure(INVALID_ARGUMENT,"builder is null or misaligned".into())) }
                let $builder = &mut *value;
                $body
            }) }
        }
    };
}
mutate!(pc_psd_builder_add_layer(layer: *const LayerInput), |builder| {
    let l = handle(layer)?;
    let o = &l.options;
    let options = crate::LayerOptions {
        blend_mode: o.blend_mode,
        opacity: o.opacity,
        fill_opacity: o.fill_opacity,
        visible: o.visible != 0,
        clipping: o.clipping != 0,
        mask_bounds: o.mask_bounds,
        mask_default: o.mask_default,
        mask_disabled: o.mask_disabled != 0,
    };
    let mask = if l.mask.data.is_null() && l.mask.len == 0 { None } else { Some(input(l.mask)?) };
    Ok(builder.add_layer(text(input(l.name)?)?, l.bounds, input(l.pixels)?, options, mask)?)
});
mutate!(pc_psd_builder_composite(pixels: Bytes), |builder| Ok(builder.composite(input(pixels)?)?));
mutate!(pc_psd_builder_begin_group(name: Bytes), |builder| Ok(builder.begin_group(text(input(name)?)?)?));
mutate!(pc_psd_builder_end_group(), |builder| Ok(builder.end_group()?));
mutate!(pc_psd_builder_resource(id: u16, name: Bytes, data: Bytes), |builder| Ok(builder.resource(id, input(name)?, input(data)?)?));
mutate!(pc_psd_builder_icc_profile(data: Bytes), |builder| Ok(builder.resource(1039, &[], input(data)?)?));
mutate!(pc_psd_builder_resolution(dpi: f64), |builder| Ok(builder.resolution(dpi)?));

export!(pc_psd_header() -> Header, |doc| Ok(doc.header()));
export!(pc_psd_layer_info(layer: usize) -> LayerInfo, |doc| Ok(doc.layer_info(layer)?));
export!(pc_psd_layer_name(layer: usize) -> Buffer, |doc| Ok(buffer(doc.layer_name(layer)?.into_bytes())));
export!(pc_psd_channel_id(layer: usize, channel: usize) -> i16, |doc| Ok(doc.channel_id(layer, channel)?));
export!(pc_psd_channel_bounds(layer: usize, id: i16) -> [i32; 4], |doc| Ok(doc.channel_bounds(layer, id)?));
export!(pc_psd_decode_channel(layer: usize, id: i16) -> Buffer, |doc| Ok(buffer(doc.decode_channel(layer, id)?)));
export!(pc_psd_decode_merged() -> Buffer, |doc| Ok(buffer(doc.decode_merged()?)));
export!(pc_psd_merged_pixels_f32() -> FloatImage, |doc| Ok(doc.merged_pixels_f32()?.into()));
export!(pc_psd_layer_pixels_f32(layer: usize) -> FloatImage, |doc| Ok(doc.layer_pixels_f32(layer)?.into()));
export!(pc_psd_icc_profile() -> Buffer, |doc| Ok(buffer(doc.icc_profile().to_vec())));
export!(pc_psd_serialize() -> Buffer, |doc| Ok(buffer(doc.to_bytes()?)));
export!(pc_psd_edit_layer(layer: usize, name: Bytes, visible: i8) -> *mut Document, |doc| {
    let name = if name.data.is_null() && name.len == 0 { None } else { Some(text(input(name)?)?) };
    let visible = match visible {
        -1 => None,
        0 => Some(false),
        1 => Some(true),
        _ => return Err(Failure(INVALID_ARGUMENT, "visible must be -1, 0 or 1".into())),
    };
    Ok(Box::into_raw(Box::new(doc.edit_layer(layer, name, visible)?)))
});
export!(pc_psd_replace_channel(layer: usize, id: i16, data: Bytes) -> *mut Document, |doc| {
    Ok(Box::into_raw(Box::new(doc.replace_channel(layer, id, input(data)?)?)))
});

export!(pc_psd_document_info() -> DocumentInfo, |doc| {
    let h = doc.header_info();
    Ok(DocumentInfo { compression: h.compression, merged_has_alpha: u8::from(h.merged_has_alpha),
        has_real_merged_data: h.has_real_merged_data.map_or(-1, i8::from) })
});
export!(pc_psd_layer_properties(layer: usize) -> LayerProperties, |doc| {
    let l = doc.layer(layer)?;
    Ok(LayerProperties {
        blend_mode: l.blend_mode.key(),
        section_type: l.section_type().as_u32(),
        id: l.layer_id().unwrap_or_default(),
        has_id: u8::from(l.layer_id().is_some()),
        visible: u8::from(l.is_visible()),
        opacity: l.opacity,
        fill_opacity: l.fill_opacity(),
        clipping: l.clipping,
    })
});
export!(pc_psd_layer_mask(layer: usize) -> Mask, |doc| {
    let Some(m) = doc.mask_info(layer)? else {
        return Ok(Mask::default());
    };
    Ok(Mask {
        bounds: bounds(m.bounds),
        real_bounds: m.real_bounds.clone().map_or([0; 4], bounds),
        user_feather: m.user_feather.unwrap_or_default(),
        vector_feather: m.vector_feather.unwrap_or_default(),
        present: 1,
        flags: m.flags,
        default_color: m.default_color,
        parameters: u8::from(m.user_density.is_some())
            | u8::from(m.user_feather.is_some()) << 1
            | u8::from(m.vector_density.is_some()) << 2
            | u8::from(m.vector_feather.is_some()) << 3,
        user_density: m.user_density.unwrap_or_default(),
        vector_density: m.vector_density.unwrap_or_default(),
        has_real: u8::from(m.real_bounds.is_some()),
        real_flags: m.real_flags.unwrap_or_default(),
        real_background: m.real_background.unwrap_or_default(),
    })
});
export!(pc_psd_resolution() -> Resolution, |doc| {
    Ok(doc.resolution().map_or_else(Resolution::default, |r| Resolution { horizontal: r.horizontal,
        vertical: r.vertical, horizontal_unit: r.horizontal_unit, vertical_unit: r.vertical_unit,
        width_unit: r.width_unit, height_unit: r.height_unit, present: 1 }))
});
export!(pc_psd_color_mode_data() -> Buffer, |doc| Ok(buffer(doc.color_mode_data().to_vec())));
export!(pc_psd_resource(id: u16) -> Buffer, |doc| Ok(buffer(doc.resource(id).unwrap_or_default().to_vec())));
export!(pc_psd_channel_compression(layer: usize, id: i16) -> i32, |doc| Ok(doc.channel_compression(layer, id)?.map_or(-1, i32::from)));
export!(pc_psd_resource_count() -> usize, |doc| Ok(doc.resource_count()));
export!(pc_psd_resource_info(index: usize) -> ResourceInfo, |doc| Ok(doc.resource_info(index)?));
export!(pc_psd_resource_name(index: usize) -> Buffer, |doc| Ok(buffer(doc.resource_name(index)?.to_vec())));
export!(pc_psd_resource_data(index: usize) -> Buffer, |doc| Ok(buffer(doc.resource_data(index)?.to_vec())));
export!(pc_psd_layer_block_count(layer: usize) -> usize, |doc| Ok(doc.layer_block_count(layer)?));
export!(pc_psd_global_block_count() -> usize, |doc| Ok(doc.global_block_count()));
export!(pc_psd_layer_block_info(layer: usize, index: usize) -> BlockInfo, |doc| Ok(doc.layer_block_info(layer, index)?));
export!(pc_psd_global_block_info(index: usize) -> BlockInfo, |doc| Ok(doc.global_block_info(index)?));
export!(pc_psd_layer_block_data(layer: usize, index: usize) -> Buffer, |doc| Ok(buffer(doc.layer_block_data(layer, index)?.to_vec())));
export!(pc_psd_global_block_data(index: usize) -> Buffer, |doc| Ok(buffer(doc.global_block_data(index)?.to_vec())));
export!(pc_psd_thumbnail_info() -> Thumbnail, |doc| Ok(doc.thumbnail_info()?.map_or_else(Thumbnail::default, |t| Thumbnail {
    format: t.format, width: t.width, height: t.height, row_bytes: t.row_bytes, resource_id: t.resource_id, present: 1, bgr: u8::from(t.bgr),
})));
export!(pc_psd_thumbnail_data() -> Buffer, |doc| Ok(buffer(doc.thumbnail_data()?.unwrap_or_default().to_vec())));
export!(pc_psd_preview(max_edge: u32) -> Preview, |doc| {
    let p = doc.preview(max_edge)?;
    Ok(Preview { width: p.width, height: p.height, data: buffer(p.data) })
});
export!(pc_psd_layer_preview(layer: usize, max_edge: u32) -> Preview, |doc| {
    let p = doc.layer_preview(layer, max_edge)?;
    Ok(Preview { width: p.width, height: p.height, data: buffer(p.data) })
});
export!(pc_psd_merged_rgba8() -> Buffer, |doc| Ok(buffer(doc.merged_rgba8()?)));
export!(pc_psd_layer_rgba8(layer: usize) -> Buffer, |doc| Ok(buffer(doc.layer_rgba8(layer)?)));
export!(pc_psd_layer_tree() -> Tree, |doc| {
    let nodes: Vec<_> = doc.layer_tree()?.into_iter().map(|n| TreeNode { index: n.index,
        parent: n.parent.unwrap_or(usize::MAX), divider: n.divider.unwrap_or(usize::MAX), is_group: u8::from(n.is_group) }).collect();
    Ok(buffer(nodes))
});

#[cfg(test)]
mod tests;
