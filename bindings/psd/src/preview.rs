use crate::{Document, document::preview_size};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image};
use photocraft_cms::{Builtin, ColorSpace, Profile, Transform, TransformOptions, transform};
use photocraft_psd::{ColorMode, PsdError, Result};
use std::sync::Arc;

pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub(crate) enum PreviewTransform {
    Srgb8,
    Managed(Arc<Transform>),
}

impl Document {
    fn preview_transform(&self) -> Result<&PreviewTransform> {
        self.1
            .get_or_init(|| {
                let h = &self.0.header;
                let (space, fallback) = match h.color_mode {
                    ColorMode::Rgb => (ColorSpace::Rgb, if h.depth == 32 { Builtin::LinearSrgb } else { Builtin::Srgb }),
                    ColorMode::Indexed if h.depth == 8 => (ColorSpace::Rgb, Builtin::Srgb),
                    ColorMode::Bitmap | ColorMode::Grayscale => (ColorSpace::Gray, Builtin::SGray),
                    ColorMode::Cmyk => (ColorSpace::Cmyk, Builtin::CoatedCmyk),
                    ColorMode::Lab => (ColorSpace::Lab, Builtin::LabD50),
                    mode => return Err(format!("unsupported preview colour mode {mode:?}")),
                };
                let embedded = self.0.icc_profile().map(Profile::parse).transpose().map_err(|e| format!("embedded ICC profile: {e}"))?;
                let source = if let Some(profile) = &embedded {
                    if profile.color_space != space {
                        return Err("embedded ICC profile does not match the PSD colour mode".into());
                    }
                    profile
                } else {
                    if space == ColorSpace::Cmyk {
                        return Err("CMYK preview requires an embedded ICC profile".into());
                    }
                    // Unprofiled float grayscale uses linear light, replicated to RGB.
                    if space == ColorSpace::Gray && h.depth == 32 { Builtin::LinearSrgb.profile() } else { fallback.profile() }
                };
                let target = Builtin::Srgb.profile();
                if h.depth == 8 && h.color_mode == ColorMode::Rgb && source.to_bytes() == target.to_bytes() {
                    return Ok(PreviewTransform::Srgb8);
                }
                transform::cached(source, target, TransformOptions { precise_float: false, ..Default::default() })
                    .map(PreviewTransform::Managed)
                    .map_err(|e| format!("preview ICC transform: {e}"))
            })
            .as_ref()
            .map_err(|message| PsdError::Unsupported(message.clone()))
    }

    pub fn merged_rgba8(&self) -> Result<Vec<u8>> {
        let h = &self.0.header;
        preview_size(h.width as usize, h.height as usize)?;
        if self.0.has_real_merged_data() == Some(false) {
            return Err(PsdError::Unsupported("document has no real merged composite; decode individual layers instead".into()));
        }
        let transform = match self.preview_transform()? {
            PreviewTransform::Srgb8 => return self.0.composite_rgba8().map(|image| image.data),
            PreviewTransform::Managed(transform) => transform,
        };
        let colors = color_channels(h.color_mode)?;
        if usize::from(h.channels) < colors {
            return Err(PsdError::Invalid("fewer channels than the colour mode requires".into()));
        }
        let data = self.decode_merged()?;
        let size = h.row_bytes().checked_mul(h.height as usize).ok_or(PsdError::LimitExceeded("preview plane overflow"))?;
        let planes: Vec<_> = data.chunks_exact(size.max(1)).map(Some).collect();
        let alpha = if self.0.merged_has_alpha() { planes.get(colors).copied().flatten() } else { None };
        self.render(
            h.width as usize,
            h.height as usize,
            planes.get(..colors).ok_or_else(|| PsdError::Invalid("truncated composite".into()))?,
            alpha,
            transform,
            true,
        )
    }

    pub fn layer_rgba8(&self, index: usize) -> Result<Vec<u8>> {
        let layer = self.layer(index)?;
        let (w, h) = layer.rect.size()?;
        preview_size(w, h)?;
        if w == 0 || h == 0 {
            return Ok(Vec::new());
        }
        let transform = match self.preview_transform()? {
            PreviewTransform::Srgb8 => {
                return self.0.layer(index).ok_or_else(|| PsdError::Invalid("layer index out of range".into()))?.rgba8().map(|image| image.data);
            }
            PreviewTransform::Managed(transform) => transform,
        };
        let colors = color_channels(self.0.header.color_mode)?;
        let planes: Vec<_> = (0..colors as i16).map(|id| layer.channel(id).map(|_| self.decode_channel(index, id)).transpose()).collect::<Result<_>>()?;
        let alpha = layer.channel(-1).map(|_| self.decode_channel(index, -1)).transpose()?;
        self.render(w, h, &planes.iter().map(|p| p.as_deref()).collect::<Vec<_>>(), alpha.as_deref(), transform, false)
    }

    fn render(&self, width: usize, height: usize, planes: &[Option<&[u8]>], alpha: Option<&[u8]>, transform: &Transform, composite: bool) -> Result<Vec<u8>> {
        let depth = self.0.header.depth;
        let mode = self.0.header.color_mode;
        let row_bytes = if depth == 1 { width.div_ceil(8) } else { width * usize::from(depth / 8) };
        if planes.iter().filter_map(|p| *p).chain(alpha).any(|p| p.len() != row_bytes * height) {
            return Err(PsdError::Invalid("preview channel size mismatch".into()));
        }
        if mode == ColorMode::Indexed && self.color_mode_data().len() != 768 {
            return Err(PsdError::Invalid("indexed preview requires a 768-byte palette".into()));
        }
        let transparent_index = self.transparent_index()?;
        let mut result = vec![0; width * height * 4];
        let inputs = transform.inputs();
        let mut source = vec![0.0; 16384 * inputs];
        let mut target = vec![0.0; 16384 * 3];
        for (chunk, output) in result.chunks_mut(16384 * 4).enumerate() {
            let start = chunk * 16384;
            let count = output.len() / 4;
            for (i, pixel) in source.chunks_exact_mut(inputs).take(count).enumerate() {
                let a = if composite && mode == ColorMode::Rgb { sample(alpha, depth, width, start + i, false) } else { 0.0 };
                for (c, out) in pixel.iter_mut().enumerate() {
                    let plane = planes.get(if planes.len() == 1 { 0 } else { c }).copied().flatten();
                    let mut value = sample(plane, depth, width, start + i, mode == ColorMode::Cmyk);
                    if a > 0.0 {
                        value = (value + a - 1.0) / a;
                    }
                    *out = match mode {
                        ColorMode::Bitmap | ColorMode::Cmyk => 1.0 - value,
                        ColorMode::Indexed => {
                            let index = (value * 255.0).round() as usize;
                            f32::from(self.color_mode_data().get(c * 256 + index).copied().unwrap_or_default()) / 255.0
                        }
                        // Photoshop stores 16-bit a*/b* on 0..65280; ICC uses the full normalized range.
                        ColorMode::Lab if depth == 16 && c > 0 => value * (65535.0 / 65280.0),
                        _ => value,
                    };
                }
            }
            // Both strides match the validated profile channel counts; alpha bypasses colour conversion.
            let src = source.get(..count * inputs).ok_or_else(|| PsdError::Invalid("preview input size".into()))?;
            let dst = target.get_mut(..count * 3).ok_or_else(|| PsdError::Invalid("preview output size".into()))?;
            transform.convert_f32(src, inputs, dst, 3, false);
            // Lookup tables clamp their inputs; extended-range float colours need the exact transform.
            if depth == 32 {
                for (pixel, rgb) in src.chunks_exact(inputs).zip(dst.as_chunks_mut::<3>().0) {
                    if pixel.iter().any(|v| !(0.0..=1.0).contains(v)) {
                        transform.eval(pixel, rgb);
                    }
                }
            }
            for (i, (rgba, rgb)) in output.as_chunks_mut::<4>().0.iter_mut().zip(dst.as_chunks::<3>().0.iter()).enumerate() {
                for (out, value) in rgba.iter_mut().take(3).zip(rgb) {
                    *out = quantize(*value)
                }
                rgba[3] = if transparent_index.is_some_and(|index| planes.first().copied().flatten().and_then(|p| p.get(start + i)) == Some(&index)) {
                    0
                } else if alpha.is_some() {
                    quantize(sample(alpha, depth, width, start + i, false))
                } else {
                    255
                };
            }
        }
        Ok(result)
    }

    /// Fit within max_edge without upscaling; alpha-aware bilinear filtering in sRGB.
    pub fn preview(&self, max_edge: u32) -> Result<Preview> {
        if max_edge == 0 {
            return Err(PsdError::Invalid("preview max_edge must be positive".into()));
        }
        let h = self.header();
        resize(Preview { width: h.width, height: h.height, data: self.merged_rgba8()? }, max_edge)
    }

    /// Preview a layer at its own bounds, without applying masks, effects, opacity or blending.
    pub fn layer_preview(&self, index: usize, max_edge: u32) -> Result<Preview> {
        if max_edge == 0 {
            return Err(PsdError::Invalid("preview max_edge must be positive".into()));
        }
        let (width, height) = self.layer(index)?.rect.size()?;
        resize(Preview { width: width as u32, height: height as u32, data: self.layer_rgba8(index)? }, max_edge)
    }
}

fn resize(mut result: Preview, max_edge: u32) -> Result<Preview> {
    if result.data.is_empty() {
        return Ok(Preview { width: 0, height: 0, data: result.data });
    }
    let edge = result.width.max(result.height);
    if edge > max_edge {
        let source = Image::from_vec_u8(result.width, result.height, result.data, PixelType::U8x4).map_err(|e| PsdError::Invalid(e.to_string()))?;
        result.width = (u64::from(result.width) * u64::from(max_edge) / u64::from(edge)).max(1) as u32;
        result.height = (u64::from(result.height) * u64::from(max_edge) / u64::from(edge)).max(1) as u32;
        let mut target = Image::new(result.width, result.height, PixelType::U8x4);
        Resizer::new()
            .resize(&source, &mut target, &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear)))
            .map_err(|e| PsdError::Invalid(e.to_string()))?;
        result.data = target.into_vec();
    }
    Ok(result)
}

fn color_channels(mode: ColorMode) -> Result<usize> {
    mode.color_channels().map(usize::from).ok_or_else(|| PsdError::Unsupported(format!("preview colour mode {mode:?}")))
}
fn quantize(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn sample(plane: Option<&[u8]>, depth: u16, width: usize, index: usize, white: bool) -> f32 {
    let value = raw_sample(plane, depth, width, index, white);
    if value.is_finite() {
        value
    } else if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}
pub(crate) fn raw_sample(plane: Option<&[u8]>, depth: u16, width: usize, index: usize, white: bool) -> f32 {
    let Some(plane) = plane else { return if white { 1.0 } else { 0.0 } };
    match depth {
        1 => f32::from(plane.get(index / width * width.div_ceil(8) + index % width / 8).copied().unwrap_or_default() >> (7 - index % width % 8) & 1),
        8 => f32::from(plane.get(index).copied().unwrap_or_default()) / 255.0,
        16 => plane.get(index * 2..index * 2 + 2).and_then(|s| s.try_into().ok()).map(u16::from_be_bytes).map_or(0.0, |v| f32::from(v) / 65535.0),
        32 => plane.get(index * 4..index * 4 + 4).and_then(|s| s.try_into().ok()).map(f32::from_be_bytes).unwrap_or_default(),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_psd::{Compression, ImageData, ImageResource, LayerSpec, PixelData, PsdBuilder, Version, testgen};
    use std::sync::OnceLock;

    fn merged(mode: ColorMode, depth: u16, samples: Vec<u8>, profile: Option<Builtin>) -> Document {
        let mut file = testgen::merged_only(Version::Psd, mode, depth, Compression::Raw, 1, 1);
        file.resources.clear();
        if let Some(profile) = profile {
            file.resources.push(ImageResource::new(1039, profile.profile().to_bytes().as_ref().clone()));
        }
        file.image_data = ImageData::encode(Compression::Raw, &samples, &file.header).unwrap();
        Document::from(file)
    }

    #[test]
    fn srgb8_passthrough_is_exact_and_profile_guarded() {
        for profile in [None, Some(Builtin::Srgb)] {
            let doc = merged(ColorMode::Rgb, 8, vec![17, 89, 203], profile);
            assert!(matches!(doc.preview_transform().unwrap(), PreviewTransform::Srgb8));
            assert_eq!(doc.merged_rgba8().unwrap(), [17, 89, 203, 255]);
        }
        assert!(matches!(merged(ColorMode::Rgb, 8, vec![17, 89, 203], Some(Builtin::DisplayP3)).preview_transform().unwrap(), PreviewTransform::Managed(_)));
        assert!(matches!(merged(ColorMode::Rgb, 16, vec![0; 6], Some(Builtin::Srgb)).preview_transform().unwrap(), PreviewTransform::Managed(_)));
    }

    #[test]
    fn icc_conversion_precedes_quantization_and_leaves_alpha_and_native_samples_alone() {
        for (depth, value, expected) in [(8, 128, 188), (16, 128, 6)] {
            let pixels = if depth == 8 {
                PixelData::Rgba8(vec![value as u8, value as u8, value as u8, 64])
            } else {
                PixelData::Rgba16(vec![value, value, value, 16384])
            };
            let mut builder = PsdBuilder::new(1, 1).depth(depth).icc_profile(Builtin::LinearSrgb.profile().to_bytes().as_ref().clone());
            builder.push_layer(LayerSpec::new("pixel", 0, 0, 1, 1, pixels.clone()));
            builder.composite(pixels);
            let bytes = builder.build().unwrap().to_bytes().unwrap();
            let doc = Document::parse(&bytes).unwrap();
            assert_eq!(doc.merged_rgba8().unwrap(), [expected, expected, expected, 64]);
            assert_eq!(doc.layer_rgba8(0).unwrap(), doc.merged_rgba8().unwrap());
            assert_eq!(doc.to_bytes().unwrap(), bytes);
            let p = doc.preview(100).unwrap();
            assert_eq!((p.width, p.height, p.data), (1, 1, doc.merged_rgba8().unwrap()));
        }
        let raw: Vec<_> = [0.5f32; 3].into_iter().flat_map(f32::to_be_bytes).collect();
        for profile in [None, Some(Builtin::LinearSrgb)] {
            let doc = merged(ColorMode::Rgb, 32, raw.clone(), profile);
            assert_eq!(doc.merged_rgba8().unwrap(), [188, 188, 188, 255]);
            assert_eq!(doc.decode_merged().unwrap(), raw);
        }
        assert_eq!(merged(ColorMode::Grayscale, 32, 0.5f32.to_be_bytes().to_vec(), None).merged_rgba8().unwrap(), [188, 188, 188, 255]);
        assert_eq!(merged(ColorMode::Grayscale, 8, vec![128], None).merged_rgba8().unwrap(), [128, 128, 128, 255]);
        assert_eq!(merged(ColorMode::Bitmap, 1, vec![128], None).merged_rgba8().unwrap(), [0, 0, 0, 255]);
        let mut indexed = merged(ColorMode::Indexed, 8, vec![7], None);
        indexed.0.color_mode_data = vec![0; 768];
        indexed.0.color_mode_data[7] = 255;
        indexed.0.color_mode_data[263] = 128;
        assert_eq!(indexed.merged_rgba8().unwrap(), [255, 128, 0, 255]);
        let floats: Vec<_> = [f32::NAN, f32::INFINITY, -1.0].into_iter().flat_map(f32::to_be_bytes).collect();
        assert_eq!(merged(ColorMode::Rgb, 32, floats, None).merged_rgba8().unwrap(), [0, 255, 0, 255]);
        let p3 = merged(ColorMode::Rgb, 8, vec![128, 64, 32], Some(Builtin::DisplayP3)).merged_rgba8().unwrap();
        for (actual, expected) in p3.iter().zip([138u8, 59, 21, 255]) {
            assert!(actual.abs_diff(expected) <= 1, "{p3:?}");
        }
        let lab = merged(ColorMode::Lab, 8, vec![128; 3], None).merged_rgba8().unwrap();
        assert!(lab.iter().take(3).all(|c| (110..=130).contains(c)));
        for (stored, white) in [(vec![255; 4], true), (vec![255, 255, 255, 0], false)] {
            let rgba = merged(ColorMode::Cmyk, 8, stored, Some(Builtin::CoatedCmyk)).merged_rgba8().unwrap();
            assert!(rgba.iter().take(3).all(|v| if white { *v >= 254 } else { *v < 100 }));
        }
    }

    #[test]
    fn float_previews_clip_after_colour_conversion_and_bitmap_alpha_keeps_its_polarity() {
        let samples = [-0.5f32, 0.5, 1.5];
        let raw: Vec<_> = samples.into_iter().flat_map(f32::to_be_bytes).collect();
        let doc = merged(ColorMode::Rgb, 32, raw.clone(), Some(Builtin::ProPhotoCompat));
        let transform = transform::cached(Builtin::ProPhotoCompat.profile(), Builtin::Srgb.profile(), TransformOptions::default()).unwrap();
        let mut expected = [0.0; 3];
        transform.eval(&samples, &mut expected);
        let expected = [quantize(expected[0]), quantize(expected[1]), quantize(expected[2]), 255];
        let mut clipped = [0.0; 3];
        transform.eval(&samples.map(|v| v.clamp(0.0, 1.0)), &mut clipped);
        assert_ne!(&expected[..3], clipped.map(quantize));
        assert_eq!(doc.merged_rgba8().unwrap(), expected);
        assert_eq!(doc.decode_merged().unwrap(), raw);
        let mut doc = merged(ColorMode::Bitmap, 1, vec![0], None);
        doc.0.header.channels = 2;
        doc.0.image_data = ImageData::encode(Compression::Raw, &[0, 128], &doc.0.header).unwrap();
        assert_eq!(doc.merged_rgba8().unwrap(), [255; 4]);
    }

    #[test]
    fn lab16_previews_use_photoshops_chroma_scale() {
        let expected = merged(ColorMode::Lab, 8, vec![128; 3], None).merged_rgba8().unwrap();
        let raw: Vec<_> = [32896_u16, 32768, 32768].into_iter().flat_map(u16::to_be_bytes).collect();
        for profile in [None, Some(Builtin::LabD50)] {
            let mut doc = merged(ColorMode::Lab, 16, raw.clone(), profile);
            doc.0.layers_mut().push(photocraft_psd::LayerRecord {
                rect: photocraft_psd::Rect::from_xywh(0, 0, 1, 1),
                channels: raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .enumerate()
                    .map(|(id, sample)| photocraft_psd::ChannelData { id: id as i16, compression: Some(Compression::Raw), data: sample.to_vec() })
                    .collect(),
                ..Default::default()
            });
            assert_eq!(doc.merged_rgba8().unwrap(), expected);
            assert_eq!(doc.layer_rgba8(0).unwrap(), expected);
            assert_eq!(doc.decode_merged().unwrap(), raw);
        }
    }

    #[test]
    fn layer_thumbnails_use_layer_bounds_and_do_not_require_a_saved_composite() {
        let mut builder = PsdBuilder::new(20, 10);
        builder.push_layer(LayerSpec::new("asset", -2, 3, 2, 1, PixelData::Rgba8(vec![255, 0, 0, 255, 0, 0, 255, 0])));
        let doc = Document::from(builder.build().unwrap());
        assert!(doc.preview(1).is_err());
        let p = doc.layer_preview(0, 1).unwrap();
        assert_eq!((p.width, p.height, p.data), (1, 1, vec![255, 0, 0, 128]));
        let p = doc.layer_preview(0, 100).unwrap();
        assert_eq!((p.width, p.height, p.data), (2, 1, doc.layer_rgba8(0).unwrap()));
        assert!(doc.layer_preview(0, 0).is_err());
        assert!(doc.layer_preview(1, 1).is_err());
        for mode in [ColorMode::Rgb, ColorMode::Cmyk] {
            let mut builder = PsdBuilder::new(1, 1).color_mode(mode);
            builder.begin_group(photocraft_psd::GroupSpec::new("empty"));
            builder.end_group().unwrap();
            let p = Document::from(builder.build().unwrap()).layer_preview(0, 1).unwrap();
            assert_eq!((p.width, p.height, p.data), (0, 0, vec![]));
        }
    }

    #[test]
    fn invalid_profiles_missing_composites_and_preview_limits_fail_without_losing_raw_data() {
        let mut doc = merged(ColorMode::Rgb, 8, vec![128; 3], None);
        doc.0.resources.push(ImageResource::new(1039, b"invalid".to_vec()));
        assert!(doc.merged_rgba8().unwrap_err().to_string().contains("ICC"));
        assert_eq!(doc.decode_merged().unwrap(), [128; 3]);
        assert!(doc.to_bytes().is_ok());
        doc.0.resources[0].data.clear();
        doc.1 = OnceLock::new();
        assert!(doc.merged_rgba8().unwrap_err().to_string().contains("ICC"));
        assert!(merged(ColorMode::Rgb, 8, vec![128; 3], Some(Builtin::SGray)).merged_rgba8().is_err());
        assert!(merged(ColorMode::Cmyk, 8, vec![255; 4], None).merged_rgba8().is_err());
        let file = PsdBuilder::new(1, 1).build().unwrap();
        assert!(Document::from(file).merged_rgba8().unwrap_err().to_string().contains("no real merged"));
        let mut doc = merged(ColorMode::Rgb, 8, vec![255; 3], None);
        assert!(doc.preview(0).is_err());
        doc.0.header.width = 2;
        doc.0.header.channels = 4;
        doc.0.image_data = ImageData::encode(Compression::Raw, &[255, 0, 0, 0, 0, 255, 255, 0], &doc.0.header).unwrap();
        let p = doc.preview(1).unwrap();
        assert_eq!((p.width, p.height), (1, 1));
        assert_eq!(p.data, [255, 0, 0, 128]); // Transparent blue must not contaminate red.
    }
}
