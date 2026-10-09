use crate::{Document, preview::raw_sample};
use photocraft_psd::{ColorMode, PsdError, Result};

/// Packed source-space colour plus straight alpha, without ICC conversion or clipping.
pub struct FloatImage {
    pub width: u32,
    pub height: u32,
    pub color_mode: u16,
    pub channels: u16,
    pub data: Vec<f32>,
}

impl Document {
    fn float_layout(&self, width: usize, height: usize) -> Result<FloatImage> {
        let mode = match (self.0.header.color_mode, self.0.header.depth) {
            (ColorMode::Bitmap, 1) => ColorMode::Grayscale,
            (ColorMode::Indexed, 8) => ColorMode::Rgb,
            (mode @ (ColorMode::Grayscale | ColorMode::Rgb), 8 | 16 | 32) | (mode @ (ColorMode::Cmyk | ColorMode::Lab), 8 | 16) => mode,
            (mode, depth) => return Err(PsdError::Unsupported(format!("float pixels for {mode:?} at depth {depth}"))),
        };
        let channels = mode.color_channels().ok_or_else(|| PsdError::Unsupported("float pixel channels".into()))? + 1;
        let bytes = width.checked_mul(height).and_then(|n| n.checked_mul(usize::from(channels) * 4));
        if bytes.is_none_or(|n| n > 1024 * 1024 * 1024) {
            return Err(PsdError::LimitExceeded("float pixels exceed 1 GiB"));
        }
        Ok(FloatImage { width: width as u32, height: height as u32, color_mode: mode.as_u16(), channels, data: Vec::new() })
    }

    pub fn merged_pixels_f32(&self) -> Result<FloatImage> {
        let h = &self.0.header;
        let image = self.float_layout(h.width as usize, h.height as usize)?;
        if self.0.has_real_merged_data() == Some(false) {
            return Err(PsdError::Unsupported("document has no real merged composite; decode individual layers instead".into()));
        }
        let colors = usize::from(h.color_mode.color_channels().ok_or_else(|| PsdError::Unsupported("float pixel channels".into()))?);
        if usize::from(h.channels) < colors {
            return Err(PsdError::Invalid("fewer channels than the colour mode requires".into()));
        }
        let data = self.decode_merged()?;
        let planes: Vec<_> = data.chunks_exact(h.row_bytes().saturating_mul(h.height as usize).max(1)).collect();
        let alpha =
            if self.0.merged_has_alpha() { Some(*planes.get(colors).ok_or_else(|| PsdError::Invalid("missing composite alpha".into()))?) } else { None };
        self.float_pixels(image, planes.get(..colors).ok_or_else(|| PsdError::Invalid("truncated composite".into()))?, alpha, true)
    }

    pub fn layer_pixels_f32(&self, index: usize) -> Result<FloatImage> {
        let layer = self.layer(index)?;
        let (w, h) = layer.rect.size()?;
        let image = self.float_layout(w, h)?;
        if w == 0 || h == 0 {
            return Ok(image);
        }
        let colors = self.0.header.color_mode.color_channels().ok_or_else(|| PsdError::Unsupported("float pixel channels".into()))?;
        let planes: Vec<_> = (0..colors as i16).map(|id| self.decode_channel(index, id)).collect::<Result<_>>()?;
        let alpha = layer.channel(-1).map(|_| self.decode_channel(index, -1)).transpose()?;
        self.float_pixels(image, &planes.iter().map(Vec::as_slice).collect::<Vec<_>>(), alpha.as_deref(), false)
    }

    fn float_pixels(&self, mut image: FloatImage, planes: &[&[u8]], alpha: Option<&[u8]>, composite: bool) -> Result<FloatImage> {
        let h = &self.0.header;
        let width = image.width as usize;
        let height = image.height as usize;
        let row_bytes = if h.depth == 1 { width.div_ceil(8) } else { width * usize::from(h.depth / 8) };
        if planes.iter().copied().chain(alpha).any(|p| p.len() != row_bytes * height) {
            return Err(PsdError::Invalid("float pixel channel size mismatch".into()));
        }
        if h.color_mode == ColorMode::Indexed && self.color_mode_data().len() != 768 {
            return Err(PsdError::Invalid("indexed pixels require a 768-byte palette".into()));
        }
        let transparent_index = self.transparent_index()?;
        let channels = usize::from(image.channels);
        image.data = vec![1.0; width * height * channels];
        for c in 0..channels - 1 {
            let plane = planes.get(if h.color_mode == ColorMode::Indexed { 0 } else { c }).ok_or_else(|| PsdError::Invalid("missing colour channel".into()))?;
            float_plane(plane, h.depth, width, image.data.iter_mut().skip(c).step_by(channels));
        }
        if let Some(alpha) = alpha {
            float_plane(alpha, h.depth, width, image.data.iter_mut().skip(channels - 1).step_by(channels));
            if image.data.iter().skip(channels - 1).step_by(channels).any(|a| !(0.0..=1.0).contains(a)) {
                return Err(PsdError::Invalid("float pixel alpha must be within 0..1".into()));
            }
        }
        if h.depth == 32 && image.data.iter().any(|v| !v.is_finite()) {
            return Err(PsdError::Invalid("float pixel samples must be finite".into()));
        }
        let palette = self.color_mode_data();
        if matches!(h.color_mode, ColorMode::Bitmap | ColorMode::Cmyk | ColorMode::Indexed) || (h.color_mode == ColorMode::Rgb && composite && alpha.is_some())
        {
            for (i, pixel) in image.data.chunks_exact_mut(channels).enumerate() {
                let (a, colors) = pixel.split_last_mut().ok_or_else(|| PsdError::Invalid("empty float pixel".into()))?;
                if transparent_index.is_some_and(|index| planes.first().and_then(|p| p.get(i)) == Some(&index)) {
                    *a = 0.0;
                }
                for (c, value) in colors.iter_mut().enumerate() {
                    *value = match h.color_mode {
                        ColorMode::Bitmap | ColorMode::Cmyk => 1.0 - *value,
                        ColorMode::Indexed => f32::from(palette.get(c * 256 + (*value * 255.0).round() as usize).copied().unwrap_or_default()) / 255.0,
                        _ => {
                            if *a == 0.0 {
                                0.0
                            } else {
                                (*value - (1.0 - *a)) / *a
                            }
                        }
                    };
                    if !value.is_finite() {
                        return Err(PsdError::Invalid("float pixel colour must be finite".into()));
                    }
                }
            }
        }
        Ok(image)
    }
}

fn float_plane<'a>(plane: &[u8], depth: u16, width: usize, output: impl Iterator<Item = &'a mut f32>) {
    match depth {
        8 => output.zip(plane).for_each(|(out, byte)| *out = f32::from(*byte) / 255.0),
        16 => output.zip(plane.as_chunks::<2>().0).for_each(|(out, bytes)| *out = f32::from(u16::from_be_bytes(*bytes)) / 65535.0),
        32 => output.zip(plane.as_chunks::<4>().0).for_each(|(out, bytes)| *out = f32::from_be_bytes(*bytes)),
        _ => output.enumerate().for_each(|(i, out)| *out = raw_sample(Some(plane), depth, width, i, false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_psd::{Compression, ImageData, LayerSpec, PixelData, PsdBuilder, Version, testgen};

    fn merged(mode: ColorMode, depth: u16, samples: &[u8]) -> Document {
        let mut file = testgen::merged_only(Version::Psd, mode, depth, Compression::Raw, 1, 1);
        file.image_data = ImageData::encode(Compression::Raw, samples, &file.header).unwrap();
        Document::from(file)
    }

    #[test]
    fn source_colour_depth_and_hdr_survive_without_icc_conversion() {
        for (mode, bytes, expected) in [
            (ColorMode::Grayscale, vec![128], vec![128.0 / 255.0, 1.0]),
            (ColorMode::Cmyk, vec![255, 128, 0, 255], vec![0.0, 1.0 - 128.0 / 255.0, 1.0, 0.0, 1.0]),
            (ColorMode::Lab, vec![64, 128, 192], vec![64.0 / 255.0, 128.0 / 255.0, 192.0 / 255.0, 1.0]),
        ] {
            let image = merged(mode, 8, &bytes).merged_pixels_f32().unwrap();
            assert_eq!((image.width, image.height, image.color_mode, image.channels), (1, 1, mode.as_u16(), expected.len() as u16));
            assert_eq!(image.data, expected);
        }
        let bytes: Vec<_> = [1u16, 32768, 65534].into_iter().flat_map(u16::to_be_bytes).collect();
        assert_eq!(merged(ColorMode::Rgb, 16, &bytes).merged_pixels_f32().unwrap().data, [1.0 / 65535.0, 32768.0 / 65535.0, 65534.0 / 65535.0, 1.0]);
        let bytes: Vec<_> = [-0.5f32, 1e-20, 2.0].into_iter().flat_map(f32::to_be_bytes).collect();
        assert_eq!(merged(ColorMode::Rgb, 32, &bytes).merged_pixels_f32().unwrap().data, [-0.5, 1e-20, 2.0, 1.0]);
        let mut bitmap = testgen::merged_only(Version::Psd, ColorMode::Bitmap, 1, Compression::Raw, 9, 2);
        bitmap.image_data = ImageData::encode(Compression::Raw, &[128, 0, 0, 128], &bitmap.header).unwrap();
        let image = Document::from(bitmap).merged_pixels_f32().unwrap();
        assert_eq!((image.color_mode, image.channels), (1, 2));
        assert_eq!(
            image.data.as_chunks::<2>().0.iter().map(|p| p[0]).collect::<Vec<_>>(),
            [0., 1., 1., 1., 1., 1., 1., 1., 1., 1., 1., 1., 1., 1., 1., 1., 1., 0.]
        );
        let mut indexed = merged(ColorMode::Indexed, 8, &[7]);
        indexed.0.color_mode_data = vec![0; 768];
        indexed.0.color_mode_data[7] = 255;
        indexed.0.color_mode_data[263] = 128;
        let image = indexed.merged_pixels_f32().unwrap();
        assert_eq!((image.color_mode, image.data), (3, vec![1., 128.0 / 255.0, 0., 1.]));
        let mut file = testgen::layered(Version::Psd, ColorMode::Rgb, 8, Compression::Raw);
        file.header.width = 1;
        file.header.height = 1;
        file.header.channels = 5;
        file.layer_info.as_mut().unwrap().merged_alpha = false;
        file.image_data = ImageData::encode(Compression::Raw, &[17, 89, 203, 0, 255], &file.header).unwrap();
        assert_eq!(Document::from(file).merged_pixels_f32().unwrap().data, [17.0 / 255.0, 89.0 / 255.0, 203.0 / 255.0, 1.]);
    }

    #[test]
    fn indexed_transparency_applies_to_previews_and_float_pixels() {
        let mut doc = merged(ColorMode::Indexed, 8, &[7]);
        doc.0.resources.clear();
        doc.0.color_mode_data = vec![0; 768];
        doc.0.color_mode_data[7] = 255;
        doc.0.resources.push(photocraft_psd::ImageResource::new(1047, 7u16.to_be_bytes().to_vec()));
        doc.0.layers_mut().push(photocraft_psd::LayerRecord {
            rect: photocraft_psd::Rect::from_xywh(0, 0, 1, 1),
            channels: vec![
                photocraft_psd::ChannelData { id: 0, compression: Some(Compression::Raw), data: vec![7] },
                photocraft_psd::ChannelData { id: -1, compression: Some(Compression::Raw), data: vec![128] },
            ],
            ..Default::default()
        });
        assert_eq!(doc.decode_merged().unwrap(), [7]);
        assert_eq!(doc.decode_channel(0, 0).unwrap(), [7]);
        for pixels in [doc.merged_rgba8().unwrap(), doc.layer_rgba8(0).unwrap()] {
            assert_eq!(pixels, [255, 0, 0, 0]);
        }
        for image in [doc.merged_pixels_f32().unwrap(), doc.layer_pixels_f32(0).unwrap()] {
            assert_eq!(image.data, [1., 0., 0., 0.]);
        }
        for index in [8u16, 256, 65535] {
            doc.0.resources[0].data = index.to_be_bytes().to_vec();
            assert_eq!(doc.merged_rgba8().unwrap()[3], 255);
            assert_eq!(doc.merged_pixels_f32().unwrap().data[3], 1.0);
            assert_eq!(doc.layer_rgba8(0).unwrap()[3], 128);
            assert_eq!(doc.layer_pixels_f32(0).unwrap().data[3], 128.0 / 255.0);
        }
        doc.0.resources[0].data.clear();
        assert!(doc.merged_rgba8().is_err() && doc.merged_pixels_f32().is_err());
    }

    #[test]
    fn composites_unmatte_but_layers_keep_straight_samples_and_ignore_opacity() {
        for version in [Version::Psd, Version::Psb] {
            for depth in [8, 16] {
                for compression in Compression::ALL {
                    let pixels = if depth == 8 {
                        PixelData::Rgba8(vec![255, 0, 0, 128, 255, 0, 0, 0])
                    } else {
                        PixelData::Rgba16(vec![65535, 0, 0, 32768, 65535, 0, 0, 0])
                    };
                    let mut builder = PsdBuilder::new(2, 1).version(version).depth(depth).compression(compression);
                    let mut layer = LayerSpec::new("pixel", -1, 2, 2, 1, pixels.clone());
                    layer.opacity = 127;
                    layer.fill_opacity = Some(128);
                    builder.push_layer(layer);
                    builder.composite(pixels);
                    let doc = Document::parse(&builder.build().unwrap().to_bytes().unwrap()).unwrap();
                    let alpha = if depth == 8 { 128.0 / 255.0 } else { 32768.0 / 65535.0 };
                    for (actual, expected) in doc.merged_pixels_f32().unwrap().data.iter().zip([1., 0., 0., alpha, 0., 0., 0., 0.]) {
                        assert!((actual - expected).abs() < 1e-6);
                    }
                    assert_eq!(doc.layer_pixels_f32(0).unwrap().data, [1., 0., 0., alpha, 1., 0., 0., 0.]);
                }
            }
        }
        let mut doc = Document::from(testgen::layered(Version::Psd, ColorMode::Rgb, 32, Compression::Raw));
        doc.0.header.width = 1;
        doc.0.header.height = 1;
        let raw: Vec<_> = [1.5f32, 0.25, 0.75, 0.5].into_iter().flat_map(f32::to_be_bytes).collect();
        doc.0.image_data = ImageData::encode(Compression::Raw, &raw, &doc.0.header).unwrap();
        assert_eq!(doc.merged_pixels_f32().unwrap().data, [2., -0.5, 0.5, 0.5]);
    }

    #[test]
    fn bad_samples_layouts_and_missing_data_fail_while_empty_layers_succeed() {
        for value in [f32::NAN, f32::INFINITY] {
            let raw: Vec<_> = [value, 0., 0.].into_iter().flat_map(f32::to_be_bytes).collect();
            assert!(merged(ColorMode::Rgb, 32, &raw).merged_pixels_f32().is_err());
        }
        let mut doc = Document::from(testgen::layered(Version::Psd, ColorMode::Rgb, 32, Compression::Raw));
        for alpha in [-0.5, 1.5, f32::NAN] {
            let raw: Vec<_> = [1.0f32, 0., 0., alpha].into_iter().flat_map(f32::to_be_bytes).collect();
            doc.0.header.width = 1;
            doc.0.header.height = 1;
            doc.0.image_data = ImageData::encode(Compression::Raw, &raw, &doc.0.header).unwrap();
            assert!(doc.merged_pixels_f32().is_err());
        }
        assert!(doc.float_layout(usize::MAX, 2).is_err());
        assert!(doc.float_layout(32769, 2048).is_err());
        assert!(doc.float_layout(6000, 4000).is_ok());
        assert!(doc.layer_pixels_f32(usize::MAX).is_err());
        assert!(doc.float_pixels(doc.float_layout(1, 1).unwrap(), &[&[0]], None, false).is_err());
        assert!(doc.float_pixels(doc.float_layout(1, 1).unwrap(), &[], None, false).is_err());
        assert!(merged(ColorMode::Duotone, 8, &[0]).merged_pixels_f32().is_err());
        assert!(merged(ColorMode::Indexed, 16, &[0, 7]).merged_pixels_f32().is_err());
        assert!(merged(ColorMode::Bitmap, 8, &[255]).merged_pixels_f32().is_err());
        let mut indexed = merged(ColorMode::Indexed, 8, &[0]);
        indexed.0.color_mode_data.clear();
        assert!(indexed.merged_pixels_f32().is_err());
        let mut builder = PsdBuilder::new(1, 1);
        builder.begin_group(photocraft_psd::GroupSpec::new("empty"));
        builder.end_group().unwrap();
        let doc = Document::from(builder.build().unwrap());
        assert!(doc.merged_pixels_f32().is_err());
        let image = doc.layer_pixels_f32(0).unwrap();
        assert_eq!((image.width, image.height, image.data), (0, 0, vec![]));
    }
}
