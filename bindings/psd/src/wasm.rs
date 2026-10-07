use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{
    BlockInfo, Builder as AdapterBuilder, BuilderConfig, Document, HeaderInfo, KnotInfo, LayerMetadata, LayerOptions, Path as AdapterPath, PathInfo,
    ResolutionInfo, ResourceInfo, SliceInfo, Slices as AdapterSlices, SlicesInfo, SubpathInfo, ThumbnailInfo, TreeNode,
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Object, js_name = keys, catch)]
    fn object_keys(value: &JsValue) -> Result<Vec<JsValue>, JsValue>;
}

fn layer_options(options: Ts<LayerOptions>) -> Result<LayerOptions, JsError> {
    // Struct deserialization visits known fields only, even with deny_unknown_fields.
    for key in object_keys(&options.js_value()).map_err(|_| JsError::new("invalid layer options"))? {
        if !matches!(
            key.as_string().as_deref(),
            Some("blendMode" | "opacity" | "fillOpacity" | "visible" | "clipping" | "maskBounds" | "maskDefault" | "maskDisabled")
        ) {
            return Err(JsError::new("unknown layer option"));
        }
    }
    Ok(options.to_rust()?)
}

#[wasm_bindgen]
pub enum Compression {
    Raw = 0,
    Rle = 1,
    Zip = 2,
    ZipPrediction = 3,
}
#[wasm_bindgen]
pub enum ColorMode {
    Bitmap = 0,
    Grayscale = 1,
    Indexed = 2,
    Rgb = 3,
    Cmyk = 4,
    Multichannel = 7,
    Duotone = 8,
    Lab = 9,
}
#[wasm_bindgen]
pub enum SectionType {
    Other = 0,
    OpenFolder = 1,
    ClosedFolder = 2,
    BoundingDivider = 3,
}

#[derive(serde::Serialize, Tsify)]
#[serde(transparent)]
#[tsify(missing_as_null)]
pub struct LayerTree(Vec<TreeNode>);

#[derive(serde::Serialize, Tsify)]
#[serde(transparent)]
#[tsify(missing_as_null)]
pub struct Resolution(Option<ResolutionInfo>);

#[derive(serde::Serialize, Tsify)]
#[serde(transparent)]
#[tsify(missing_as_null)]
pub struct Thumbnail(Option<ThumbnailInfo>);

#[wasm_bindgen]
pub struct Preview(crate::preview::Preview);
#[wasm_bindgen]
impl Preview {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.0.width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.0.height
    }
    #[wasm_bindgen(getter)]
    pub fn data(&self) -> Vec<u8> {
        self.0.data.clone()
    }
}

fn error(e: photocraft_psd::PsdError) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen]
pub struct FloatImage(crate::FloatImage);
#[wasm_bindgen]
impl FloatImage {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.0.width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.0.height
    }
    #[wasm_bindgen(getter, js_name = colorMode)]
    pub fn color_mode(&self) -> u16 {
        self.0.color_mode
    }
    #[wasm_bindgen(getter)]
    pub fn channels(&self) -> u16 {
        self.0.channels
    }
    #[wasm_bindgen(getter)]
    pub fn data(&self) -> Vec<f32> {
        self.0.data.clone()
    }
}

fn index(value: f64) -> Result<usize, JsError> {
    if !value.is_finite() || value.fract() != 0.0 || !(0.0..=u32::MAX as f64).contains(&value) {
        return Err(JsError::new("index must be an unsigned 32-bit integer"));
    }
    Ok(value as usize)
}

fn channel(value: f64) -> Result<i16, JsError> {
    if !value.is_finite() || value.fract() != 0.0 || !(i16::MIN as f64..=i16::MAX as f64).contains(&value) {
        return Err(JsError::new("channel ID must be a signed 16-bit integer"));
    }
    Ok(value as i16)
}

#[wasm_bindgen(js_name=Path)]
pub struct PsdPath(AdapterPath);
#[wasm_bindgen(js_class = Path)]
impl PsdPath {
    pub fn info(&self) -> Result<Ts<PathInfo>, JsError> {
        Ok(self.0.info().into_ts()?)
    }
    #[wasm_bindgen(js_name=subpathInfo)]
    pub fn subpath_info(&self, value: f64) -> Result<Ts<SubpathInfo>, JsError> {
        Ok(self.0.subpath_info(index(value)?).map_err(error)?.into_ts()?)
    }
    pub fn knot(&self, subpath: f64, value: f64) -> Result<Ts<KnotInfo>, JsError> {
        Ok(self.0.knot(index(subpath)?, index(value)?).map_err(error)?.into_ts()?)
    }
}
#[wasm_bindgen(js_name=Slices)]
pub struct PsdSlices(AdapterSlices);
#[wasm_bindgen(js_class = Slices)]
impl PsdSlices {
    pub fn info(&self) -> Result<Ts<SlicesInfo>, JsError> {
        Ok(self.0.info().into_ts()?)
    }
    #[wasm_bindgen(js_name=groupName)]
    pub fn group_name(&self) -> String {
        self.0.group_name().into()
    }
    #[wasm_bindgen(js_name=sliceInfo)]
    pub fn slice_info(&self, value: f64) -> Result<Ts<SliceInfo>, JsError> {
        Ok(self.0.slice_info(index(value)?).map_err(error)?.into_ts()?)
    }
    pub fn text(&self, value: f64, field: &str) -> Result<String, JsError> {
        self.0.text(index(value)?, field).map(str::to_owned).map_err(error)
    }
}
fn code(value: f64) -> Result<u16, JsError> {
    u16::try_from(index(value)?).map_err(|_| JsError::new("code must fit in 16 bits"))
}
#[wasm_bindgen(js_name=PsdBuilder)]
pub struct PsdBuilder(AdapterBuilder);
#[wasm_bindgen(js_class = PsdBuilder)]
impl PsdBuilder {
    #[wasm_bindgen(constructor)]
    pub fn new(
        width: f64,
        height: f64,
        depth: Option<f64>,
        color_mode: Option<f64>,
        version: Option<f64>,
        compression: Option<f64>,
    ) -> Result<PsdBuilder, JsError> {
        AdapterBuilder::new(BuilderConfig {
            width: index(width)? as u32,
            height: index(height)? as u32,
            depth: code(depth.unwrap_or(8.0))?,
            color_mode: code(color_mode.unwrap_or(3.0))?,
            version: code(version.unwrap_or(1.0))?,
            compression: code(compression.unwrap_or(1.0))?,
        })
        .map(Self)
        .map_err(error)
    }
    #[wasm_bindgen(js_name=addLayer)]
    pub fn add_layer(&mut self, name: &str, bounds: &[i32], pixels: &[u8], options: Option<Ts<LayerOptions>>, mask: Option<Vec<u8>>) -> Result<(), JsError> {
        let bounds = bounds.try_into().map_err(|_| JsError::new("bounds require four integers"))?;
        let options = options.map(layer_options).transpose()?.unwrap_or_default();
        self.0.add_layer(name, bounds, pixels, options, mask.as_deref()).map_err(error)
    }
    pub fn composite(&mut self, pixels: &[u8]) -> Result<(), JsError> {
        self.0.composite(pixels).map_err(error)
    }
    #[wasm_bindgen(js_name=beginGroup)]
    pub fn begin_group(&mut self, name: &str) -> Result<(), JsError> {
        self.0.begin_group(name).map_err(error)
    }
    #[wasm_bindgen(js_name=endGroup)]
    pub fn end_group(&mut self) -> Result<(), JsError> {
        self.0.end_group().map_err(error)
    }
    pub fn resource(&mut self, id: f64, data: &[u8], name: Option<Vec<u8>>) -> Result<(), JsError> {
        self.0.resource(code(id)?, name.as_deref().unwrap_or_default(), data).map_err(error)
    }
    #[wasm_bindgen(js_name=iccProfile)]
    pub fn icc_profile(&mut self, data: &[u8]) -> Result<(), JsError> {
        self.0.resource(1039, &[], data).map_err(error)
    }
    pub fn resolution(&mut self, dpi: f64) -> Result<(), JsError> {
        self.0.resolution(dpi).map_err(error)
    }
    pub fn build(&self) -> Result<Psd, JsError> {
        self.0.build().map(Psd).map_err(error)
    }
}

#[wasm_bindgen(js_name = Psd)]
pub struct Psd(Document);

#[wasm_bindgen]
impl Psd {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], lazy: Option<bool>) -> Result<Psd, JsError> {
        (if lazy.unwrap_or(false) { Document::parse_lazy(bytes) } else { Document::parse(bytes) }).map(Self).map_err(error)
    }

    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.0.header().width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.0.header().height
    }
    #[wasm_bindgen(getter)]
    pub fn version(&self) -> u16 {
        self.0.header().version
    }
    #[wasm_bindgen(getter)]
    pub fn channels(&self) -> u16 {
        self.0.header().channels
    }
    #[wasm_bindgen(getter)]
    pub fn depth(&self) -> u16 {
        self.0.header().depth
    }
    #[wasm_bindgen(getter, js_name = colorMode)]
    pub fn color_mode(&self) -> u16 {
        self.0.header().color_mode
    }
    #[wasm_bindgen(getter, js_name = layerCount)]
    pub fn layer_count(&self) -> usize {
        self.0.header().layer_count
    }

    #[wasm_bindgen(js_name = layerName)]
    pub fn layer_name(&self, layer: f64) -> Result<String, JsError> {
        self.0.layer_name(index(layer)?).map_err(error)
    }

    #[wasm_bindgen(js_name = layerBounds)]
    pub fn layer_bounds(&self, layer: f64) -> Result<Vec<i32>, JsError> {
        let i = self.0.layer_info(index(layer)?).map_err(error)?;
        Ok(vec![i.top, i.left, i.bottom, i.right])
    }

    #[wasm_bindgen(js_name = layerChannelCount)]
    pub fn layer_channel_count(&self, layer: f64) -> Result<usize, JsError> {
        Ok(self.0.layer_info(index(layer)?).map_err(error)?.channel_count)
    }

    #[wasm_bindgen(js_name = channelId)]
    pub fn channel_id(&self, layer: f64, channel: f64) -> Result<i16, JsError> {
        self.0.channel_id(index(layer)?, index(channel)?).map_err(error)
    }

    #[wasm_bindgen(js_name = channelBounds)]
    pub fn channel_bounds(&self, layer: f64, id: f64) -> Result<Vec<i32>, JsError> {
        self.0.channel_bounds(index(layer)?, channel(id)?).map(|r| r.to_vec()).map_err(error)
    }

    #[wasm_bindgen(js_name = decodeMerged)]
    pub fn decode_merged(&self) -> Result<Vec<u8>, JsError> {
        self.0.decode_merged().map_err(error)
    }

    #[wasm_bindgen(js_name = decodeChannel)]
    pub fn decode_channel(&self, layer: f64, id: f64) -> Result<Vec<u8>, JsError> {
        self.0.decode_channel(index(layer)?, channel(id)?).map_err(error)
    }

    #[wasm_bindgen(js_name = iccProfile)]
    pub fn icc_profile(&self) -> Vec<u8> {
        self.0.icc_profile().to_vec()
    }

    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Result<Vec<u8>, JsError> {
        self.0.to_bytes().map_err(error)
    }
    #[wasm_bindgen(js_name = editLayer)]
    pub fn edit_layer(&self, layer: f64, name: Option<String>, visible: Option<bool>) -> Result<Psd, JsError> {
        self.0.edit_layer(index(layer)?, name.as_deref(), visible).map(Psd).map_err(error)
    }
    #[wasm_bindgen(js_name = replaceChannel)]
    pub fn replace_channel(&self, layer: f64, id: f64, data: &[u8]) -> Result<Psd, JsError> {
        self.0.replace_channel(index(layer)?, channel(id)?, data).map(Psd).map_err(error)
    }
    #[wasm_bindgen(js_name = mergedPixelsF32)]
    pub fn merged_pixels_f32(&self) -> Result<FloatImage, JsError> {
        self.0.merged_pixels_f32().map(FloatImage).map_err(error)
    }
    #[wasm_bindgen(js_name = layerPixelsF32)]
    pub fn layer_pixels_f32(&self, layer: f64) -> Result<FloatImage, JsError> {
        self.0.layer_pixels_f32(index(layer)?).map(FloatImage).map_err(error)
    }

    #[wasm_bindgen(js_name = headerInfo)]
    pub fn header_info(&self) -> Result<Ts<HeaderInfo>, JsError> {
        Ok(self.0.header_info().into_ts()?)
    }
    #[wasm_bindgen(js_name = layerMetadata)]
    pub fn layer_metadata(&self, layer: f64) -> Result<Ts<LayerMetadata>, JsError> {
        Ok(self.0.layer_metadata(index(layer)?).map_err(error)?.into_ts()?)
    }
    #[wasm_bindgen(js_name = layerTree)]
    pub fn layer_tree(&self) -> Result<Ts<LayerTree>, JsError> {
        Ok(LayerTree(self.0.layer_tree().map_err(error)?).into_ts()?)
    }
    #[wasm_bindgen(js_name = channelCompression)]
    pub fn channel_compression(&self, layer: f64, id: f64) -> Result<Option<u16>, JsError> {
        self.0.channel_compression(index(layer)?, channel(id)?).map_err(error)
    }
    pub fn resolution(&self) -> Result<Ts<Resolution>, JsError> {
        Ok(Resolution(self.0.resolution()).into_ts()?)
    }
    #[wasm_bindgen(js_name = colorModeData)]
    pub fn color_mode_data(&self) -> Vec<u8> {
        self.0.color_mode_data().to_vec()
    }
    pub fn resource(&self, id: f64) -> Result<Option<Vec<u8>>, JsError> {
        Ok(self.0.resource(code(id)?).map(<[u8]>::to_vec))
    }
    #[wasm_bindgen(js_name=resourcePath)]
    pub fn resource_path(&self, value: f64) -> Result<Option<PsdPath>, JsError> {
        self.0.resource_path(index(value)?).map(|p| p.map(PsdPath)).map_err(error)
    }
    #[wasm_bindgen(js_name=vectorMask)]
    pub fn vector_mask(&self, layer: f64) -> Result<Option<PsdPath>, JsError> {
        self.0.vector_mask(index(layer)?).map(|p| p.map(PsdPath)).map_err(error)
    }
    pub fn slices(&self) -> Result<Option<PsdSlices>, JsError> {
        self.0.slices().map(|s| s.map(PsdSlices)).map_err(error)
    }
    #[wasm_bindgen(js_name = resourceCount)]
    pub fn resource_count(&self) -> usize {
        self.0.resource_count()
    }
    #[wasm_bindgen(js_name = resourceInfo)]
    pub fn resource_info(&self, value: f64) -> Result<Ts<ResourceInfo>, JsError> {
        Ok(self.0.resource_info(index(value)?).map_err(error)?.into_ts()?)
    }
    #[wasm_bindgen(js_name = resourceName)]
    pub fn resource_name(&self, value: f64) -> Result<Vec<u8>, JsError> {
        self.0.resource_name(index(value)?).map(<[u8]>::to_vec).map_err(error)
    }
    #[wasm_bindgen(js_name = resourceData)]
    pub fn resource_data(&self, value: f64) -> Result<Vec<u8>, JsError> {
        self.0.resource_data(index(value)?).map(<[u8]>::to_vec).map_err(error)
    }
    #[wasm_bindgen(js_name = layerBlockCount)]
    pub fn layer_block_count(&self, layer: f64) -> Result<usize, JsError> {
        self.0.layer_block_count(index(layer)?).map_err(error)
    }
    #[wasm_bindgen(js_name = globalBlockCount)]
    pub fn global_block_count(&self) -> usize {
        self.0.global_block_count()
    }
    #[wasm_bindgen(js_name = layerBlockInfo)]
    pub fn layer_block_info(&self, layer: f64, value: f64) -> Result<Ts<BlockInfo>, JsError> {
        Ok(self.0.layer_block_info(index(layer)?, index(value)?).map_err(error)?.into_ts()?)
    }
    #[wasm_bindgen(js_name = globalBlockInfo)]
    pub fn global_block_info(&self, value: f64) -> Result<Ts<BlockInfo>, JsError> {
        Ok(self.0.global_block_info(index(value)?).map_err(error)?.into_ts()?)
    }
    #[wasm_bindgen(js_name = layerBlockData)]
    pub fn layer_block_data(&self, layer: f64, value: f64) -> Result<Vec<u8>, JsError> {
        self.0.layer_block_data(index(layer)?, index(value)?).map(<[u8]>::to_vec).map_err(error)
    }
    #[wasm_bindgen(js_name = globalBlockData)]
    pub fn global_block_data(&self, value: f64) -> Result<Vec<u8>, JsError> {
        self.0.global_block_data(index(value)?).map(<[u8]>::to_vec).map_err(error)
    }
    #[wasm_bindgen(js_name = thumbnailInfo)]
    pub fn thumbnail_info(&self) -> Result<Ts<Thumbnail>, JsError> {
        Ok(Thumbnail(self.0.thumbnail_info().map_err(error)?).into_ts()?)
    }
    #[wasm_bindgen(js_name = thumbnailData)]
    pub fn thumbnail_data(&self) -> Result<Option<Vec<u8>>, JsError> {
        self.0.thumbnail_data().map(|data| data.map(<[u8]>::to_vec)).map_err(error)
    }
    pub fn preview(&self, max_edge: f64) -> Result<Preview, JsError> {
        self.0.preview(index(max_edge)? as u32).map(Preview).map_err(error)
    }
    #[wasm_bindgen(js_name = layerPreview)]
    pub fn layer_preview(&self, layer: f64, max_edge: f64) -> Result<Preview, JsError> {
        self.0.layer_preview(index(layer)?, index(max_edge)? as u32).map(Preview).map_err(error)
    }
    #[wasm_bindgen(js_name = mergedRgba8)]
    pub fn merged_rgba8(&self) -> Result<Vec<u8>, JsError> {
        self.0.merged_rgba8().map_err(error)
    }
    #[wasm_bindgen(js_name = layerRgba8)]
    pub fn layer_rgba8(&self, layer: f64) -> Result<Vec<u8>, JsError> {
        self.0.layer_rgba8(index(layer)?).map_err(error)
    }
}
