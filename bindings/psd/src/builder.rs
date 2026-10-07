use crate::Document;
use photocraft_psd::{
    BlendMode, ColorMode, Compression, GroupSpec, Header, ImageResource, LayerSpec, MaskSpec, PixelData, PsdBuilder, PsdError, Rect, Result, Version,
};

const MAX_BYTES: usize = 1024 * 1024 * 1024;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BuilderConfig {
    pub width: u32,
    pub height: u32,
    pub color_mode: u16,
    pub depth: u16,
    pub version: u16,
    pub compression: u16,
}

#[derive(Clone, Debug)]
#[cfg_attr(all(feature = "javascript", target_arch = "wasm32"), derive(serde::Serialize, serde::Deserialize, tsify::Tsify))]
#[cfg_attr(all(feature = "javascript", target_arch = "wasm32"), serde(default, rename_all = "camelCase"))]
pub struct LayerOptions {
    pub blend_mode: [u8; 4],
    pub opacity: u8,
    pub fill_opacity: u8,
    pub visible: bool,
    pub clipping: bool,
    pub mask_bounds: [i32; 4],
    pub mask_default: u8,
    pub mask_disabled: bool,
}
impl Default for LayerOptions {
    fn default() -> Self {
        Self {
            blend_mode: *b"norm",
            opacity: 255,
            fill_opacity: 255,
            visible: true,
            clipping: false,
            mask_bounds: [0; 4],
            mask_default: 255,
            mask_disabled: false,
        }
    }
}

pub struct Builder {
    inner: PsdBuilder,
    config: BuilderConfig,
    used: usize,
    composite_bytes: usize,
    groups: usize,
    records: usize,
}
impl Builder {
    pub fn new(config: BuilderConfig) -> Result<Self> {
        let version = match config.version {
            1 => Version::Psd,
            2 => Version::Psb,
            v => return Err(PsdError::UnsupportedVersion(v)),
        };
        let mode = ColorMode::from_u16(config.color_mode);
        if !matches!(mode, ColorMode::Rgb | ColorMode::Grayscale | ColorMode::Cmyk) || !matches!(config.depth, 8 | 16) {
            return Err(PsdError::Unsupported("builder requires RGB, Gray or CMYK at 8/16 bits".into()));
        }
        let compression = Compression::from_u16(config.compression);
        if !Compression::ALL.contains(&compression) {
            return Err(PsdError::Unsupported("builder compression".into()));
        }
        Header::new(version, config.width, config.height, 1, config.depth, mode).validate()?;
        if version == Version::Psd && config.width.max(config.height) > 30000 {
            return Err(PsdError::LimitExceeded("PSD dimensions exceed 30000; use PSB"));
        }
        let inner = PsdBuilder::new(config.width, config.height).version(version).color_mode(mode).depth(config.depth).compression(compression);
        let result = Self { inner, config, used: 0, composite_bytes: 0, groups: 0, records: 0 };
        result.pixel_bytes(config.width, config.height)?;
        Ok(result)
    }
    fn reserve(&self, bytes: usize) -> Result<usize> {
        self.used.checked_add(bytes).filter(|n| *n <= MAX_BYTES).ok_or(PsdError::LimitExceeded("builder inputs exceed 1 GiB"))
    }
    fn reserve_records(&self, count: usize) -> Result<usize> {
        self.records.checked_add(count).filter(|n| *n <= i16::MAX as usize).ok_or(PsdError::LimitExceeded("builder layer records exceed 32767"))
    }
    fn pixel_bytes(&self, width: u32, height: u32) -> Result<usize> {
        let colors = match self.config.color_mode {
            1 => 1,
            3 => 3,
            _ => 4,
        };
        (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(colors + 1))
            .and_then(|n| n.checked_mul(usize::from(self.config.depth / 8)))
            .filter(|n| *n <= MAX_BYTES)
            .ok_or(PsdError::LimitExceeded("builder pixels exceed 1 GiB"))
    }
    fn pixels(&self, width: u32, height: u32, data: &[u8]) -> Result<PixelData> {
        if data.len() != self.pixel_bytes(width, height)? {
            return Err(PsdError::Invalid("pixel buffer size mismatch".into()));
        }
        Ok(match (self.config.color_mode, self.config.depth) {
            (1, 8) => PixelData::GrayA8(data.to_vec()),
            (3, 8) => PixelData::Rgba8(data.to_vec()),
            (4, 8) => PixelData::Cmyka8(data.to_vec()),
            (mode, _) => {
                let samples = data.as_chunks::<2>().0.iter().map(|s| u16::from_be_bytes(*s)).collect();
                match mode {
                    1 => PixelData::GrayA16(samples),
                    3 => PixelData::Rgba16(samples),
                    _ => PixelData::Cmyka16(samples),
                }
            }
        })
    }
    pub fn add_layer(&mut self, name: &str, bounds: [i32; 4], pixels: &[u8], options: LayerOptions, mask: Option<&[u8]>) -> Result<()> {
        let rect = bounds_rect(bounds);
        let (width, height) = rect.size()?;
        let records = self.reserve_records(1)?;
        let used = self.reserve(
            pixels
                .len()
                .checked_add(name.len())
                .and_then(|n| n.checked_add(std::mem::size_of::<LayerSpec>()))
                .and_then(|n| n.checked_add(mask.map_or(0, <[u8]>::len)))
                .ok_or(PsdError::LimitExceeded("builder input overflow"))?,
        )?;
        let pixels = self.pixels(width as u32, height as u32, pixels)?;
        let mask = if let Some(data) = mask {
            let rect = bounds_rect(options.mask_bounds);
            let (w, h) = rect.size()?;
            if w.checked_mul(h) != Some(data.len()) {
                return Err(PsdError::Invalid("mask buffer size mismatch".into()));
            }
            Some(MaskSpec { rect, data: data.to_vec(), default_color: options.mask_default, disabled: options.mask_disabled })
        } else {
            None
        };
        let mut layer = LayerSpec::new(name, rect.left, rect.top, width as u32, height as u32, pixels);
        layer.blend_mode = BlendMode::from_key(options.blend_mode);
        layer.opacity = options.opacity;
        layer.fill_opacity = Some(options.fill_opacity);
        layer.visible = options.visible;
        layer.clipping = options.clipping;
        layer.mask = mask;
        self.inner.push_layer(layer);
        self.records = records;
        self.used = used;
        Ok(())
    }
    pub fn composite(&mut self, pixels: &[u8]) -> Result<()> {
        let used = self
            .used
            .saturating_sub(self.composite_bytes)
            .checked_add(pixels.len())
            .filter(|n| *n <= MAX_BYTES)
            .ok_or(PsdError::LimitExceeded("builder inputs exceed 1 GiB"))?;
        let pixels_data = self.pixels(self.config.width, self.config.height, pixels)?;
        self.inner.composite(pixels_data);
        self.used = used;
        self.composite_bytes = pixels.len();
        Ok(())
    }
    pub fn begin_group(&mut self, name: &str) -> Result<()> {
        if self.groups >= 256 {
            return Err(PsdError::LimitExceeded("builder groups exceed 256 levels"));
        }
        let records = self.reserve_records(2)?;
        let used = self.reserve(name.len().saturating_add(2 * std::mem::size_of::<GroupSpec>()))?;
        self.inner.begin_group(GroupSpec::new(name));
        self.records = records;
        self.groups += 1;
        self.used = used;
        Ok(())
    }
    pub fn end_group(&mut self) -> Result<()> {
        self.inner.end_group()?;
        self.groups = self.groups.saturating_sub(1);
        Ok(())
    }
    pub fn resource(&mut self, id: u16, name: &[u8], data: &[u8]) -> Result<()> {
        if name.len() > 255 {
            return Err(PsdError::Invalid("resource name exceeds 255 bytes".into()));
        }
        let used = self.reserve(
            name.len()
                .checked_add(data.len())
                .and_then(|n| n.checked_add(std::mem::size_of::<ImageResource>()))
                .ok_or(PsdError::LimitExceeded("resource input overflow"))?,
        )?;
        let resource = ImageResource { name: name.to_vec(), ..ImageResource::new(id, data.to_vec()) };
        self.inner = std::mem::replace(&mut self.inner, PsdBuilder::new(self.config.width, self.config.height)).resource(resource);
        self.used = used;
        Ok(())
    }
    pub fn resolution(&mut self, dpi: f64) -> Result<()> {
        if !dpi.is_finite() || !(1.0 / 65536.0..=65535.0).contains(&dpi) {
            return Err(PsdError::Invalid("DPI must be positive and fit in 16.16 fixed point".into()));
        }
        self.resource(1005, &[], &photocraft_psd::ResolutionInfo::from_dpi(dpi).to_bytes())
    }
    pub fn build(&self) -> Result<Document> {
        self.inner.build().map(Document::from)
    }
}

fn bounds_rect(bounds: [i32; 4]) -> Rect {
    let [top, left, bottom, right] = bounds;
    Rect { top, left, bottom, right }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> BuilderConfig {
        BuilderConfig { width: 1, height: 1, color_mode: 3, depth: 8, version: 1, compression: 1 }
    }
    #[test]
    fn creation_roundtrips_modes_depths_versions_and_codecs() {
        for version in [1, 2] {
            for depth in [8, 16] {
                for color_mode in [1, 3, 4] {
                    for compression in 0..4 {
                        let mut b = Builder::new(BuilderConfig { version, depth, color_mode, compression, ..config() }).unwrap();
                        let colors = if color_mode == 1 {
                            1
                        } else if color_mode == 3 {
                            3
                        } else {
                            4
                        };
                        let samples: Vec<u16> = (0..colors).map(|i| 1000 + i * 10000).chain([32768]).collect();
                        let bytes: Vec<u8> = samples.iter().flat_map(|v| if depth == 8 { vec![(v >> 8) as u8] } else { v.to_be_bytes().to_vec() }).collect();
                        b.begin_group("Folder").unwrap();
                        b.add_layer(
                            "Ink",
                            [-1, 2, 0, 3],
                            &bytes,
                            LayerOptions {
                                opacity: 127,
                                fill_opacity: 128,
                                visible: false,
                                clipping: true,
                                mask_bounds: [0, 0, 1, 1],
                                mask_disabled: true,
                                ..Default::default()
                            },
                            Some(&[128]),
                        )
                        .unwrap();
                        assert!(b.build().is_err());
                        b.end_group().unwrap();
                        b.composite(&bytes).unwrap();
                        b.resolution(300.0).unwrap();
                        b.resource(1060, b"XMP", b"<xmp/>").unwrap();
                        let doc = b.build().unwrap();
                        let parsed = Document::parse(&doc.to_bytes().unwrap()).unwrap();
                        assert_eq!(parsed.header().depth, depth);
                        assert_eq!(parsed.header().version, version);
                        assert_eq!(parsed.header_info().compression, compression);
                        let mut expected: Vec<u8> = samples
                            .iter()
                            .enumerate()
                            .flat_map(|(i, v)| {
                                if depth == 8 {
                                    vec![if color_mode == 4 && i < colors as usize { 255 - (v >> 8) as u8 } else { (v >> 8) as u8 }]
                                } else {
                                    (if color_mode == 4 && i < colors as usize { 65535 - v } else { *v }).to_be_bytes().to_vec()
                                }
                            })
                            .collect();
                        if color_mode == 3 {
                            expected = if depth == 8 {
                                vec![129, 148, 168, 128]
                            } else {
                                [33267u16, 38267, 43267, 32768].into_iter().flat_map(u16::to_be_bytes).collect()
                            };
                        }
                        assert_eq!(parsed.decode_merged().unwrap(), expected);
                        assert_eq!(
                            parsed.decode_channel(1, 0).unwrap(),
                            if color_mode == 4 { expected[..usize::from(depth / 8)].to_vec() } else { bytes[..usize::from(depth / 8)].to_vec() }
                        );
                        let l = parsed.layer_metadata(1).unwrap();
                        assert_eq!((l.opacity, l.fill_opacity, l.visible, l.clipping), (127, 128, false, 1));
                        assert_eq!(parsed.decode_channel(1, -2).unwrap(), if depth == 8 { vec![128] } else { vec![128, 128] });
                        assert_eq!(parsed.layer_tree().unwrap().len(), 2);
                        assert_eq!(parsed.resolution().unwrap().horizontal, 300.0);
                        assert_eq!(parsed.resource(1060), Some(b"<xmp/>".as_slice()));
                        assert_eq!(b.build().unwrap().to_bytes().unwrap(), doc.to_bytes().unwrap());
                    }
                }
            }
        }
    }
    #[test]
    fn invalid_inputs_do_not_mutate_the_builder() {
        for c in [
            BuilderConfig { width: 0, ..config() },
            BuilderConfig { version: 3, ..config() },
            BuilderConfig { depth: 32, ..config() },
            BuilderConfig { color_mode: 9, ..config() },
            BuilderConfig { compression: 4, ..config() },
            BuilderConfig { width: 30001, ..config() },
            BuilderConfig { version: 2, width: 300000, height: 300000, ..config() },
        ] {
            assert!(Builder::new(c).is_err());
        }
        let mut b = Builder::new(config()).unwrap();
        let rgba = [255, 0, 0, 128];
        for bounds in [[1, 0, 0, 1], [0, i32::MIN, 1, i32::MAX], [0, 0, 1, 300001]] {
            assert!(b.add_layer("", bounds, &rgba, LayerOptions::default(), None).is_err());
            assert!(b.add_layer("", [0, 0, 1, 1], &rgba, LayerOptions { mask_bounds: bounds, ..Default::default() }, Some(&[1])).is_err());
        }
        assert!(b.add_layer("", [0, 0, 1, 1], &[], LayerOptions::default(), None).is_err());
        assert!(b.add_layer("", [0, 0, 1, 1], &rgba, LayerOptions::default(), Some(&[1])).is_err());
        assert!(b.composite(&[]).is_err());
        for dpi in [0.0, -1.0, f64::NAN, f64::INFINITY, 65536.0] {
            assert!(b.resolution(dpi).is_err());
        }
        assert!(b.resource(1, &[0; 256], &[]).is_err());
        assert!(b.end_group().is_err());
        assert_eq!((b.used, b.records), (0, 0));
        b.composite(&rgba).unwrap();
        b.composite(&rgba).unwrap();
        assert_eq!(b.used, rgba.len());
        b.used = MAX_BYTES;
        assert!(b.resource(1, &[], &[]).is_err());
        assert!(b.add_layer("", [0, 0, 1, 1], &rgba, LayerOptions::default(), None).is_err());
        b.used = 0;
        b.records = i16::MAX as usize;
        assert!(b.add_layer("", [0, 0, 1, 1], &rgba, LayerOptions::default(), None).is_err());
        assert!(b.begin_group("").is_err());
        b.records = 0;
        for _ in 0..256 {
            b.begin_group("").unwrap();
        }
        assert!(b.begin_group("").is_err());
        for _ in 0..256 {
            b.end_group().unwrap();
        }
        assert!(b.build().is_ok());
    }
}
