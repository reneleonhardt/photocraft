#![forbid(unsafe_code)]

use crate::preview::PreviewTransform;
use crate::{HeaderInfo, LayerMetadata, MaskInfo, ResolutionInfo, TreeNode};
use photocraft_psd::{LayerNode, LayerRecord, PsdError, PsdFile, Result, SectionType};
use std::sync::OnceLock;

/// Header values use the numeric PSD specification codes, including unknown colour modes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub width: u32,
    pub height: u32,
    pub version: u16,
    pub channels: u16,
    pub depth: u16,
    pub color_mode: u16,
    pub layer_count: usize,
}

/// Layer bounds are signed document coordinates; file order is bottom-most first.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerInfo {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub channel_count: usize,
}

/// An owned parsed document; no input bytes or layer views are borrowed from the caller.
pub struct Document(pub(crate) PsdFile, pub(crate) OnceLock<std::result::Result<PreviewTransform, String>>);

impl From<PsdFile> for Document {
    fn from(file: PsdFile) -> Self {
        Self(file, OnceLock::new())
    }
}

impl Document {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        PsdFile::from_bytes(bytes).map(Self::from)
    }

    pub fn parse_lazy(bytes: &[u8]) -> Result<Self> {
        PsdFile::from_bytes_lazy(bytes).map(Self::from)
    }

    pub fn header(&self) -> Header {
        let h = &self.0.header;
        Header {
            width: h.width,
            height: h.height,
            version: h.version.as_u16(),
            channels: h.channels,
            depth: h.depth,
            color_mode: h.color_mode.as_u16(),
            layer_count: self.0.layers().len(),
        }
    }

    pub(crate) fn layer(&self, index: usize) -> Result<&LayerRecord> {
        self.0.layers().get(index).ok_or_else(|| PsdError::Invalid(format!("layer index {index} out of range")))
    }

    pub fn layer_info(&self, index: usize) -> Result<LayerInfo> {
        let layer = self.layer(index)?;
        let r = layer.rect;
        Ok(LayerInfo { top: r.top, left: r.left, bottom: r.bottom, right: r.right, channel_count: layer.channels.len() })
    }

    pub fn layer_name(&self, index: usize) -> Result<String> {
        Ok(self.layer(index)?.name())
    }

    pub fn channel_id(&self, layer: usize, channel: usize) -> Result<i16> {
        self.layer(layer)?.channels.get(channel).map(|c| c.id).ok_or_else(|| PsdError::Invalid(format!("channel index {channel} out of range")))
    }

    /// Bounds of a colour, transparency, user-mask (-2), or real-mask (-3) channel.
    pub fn channel_bounds(&self, layer: usize, id: i16) -> Result<[i32; 4]> {
        let layer = self.layer(layer)?;
        if layer.channel(id).is_none() {
            return Err(PsdError::Invalid(format!("layer has no channel {id}")));
        }
        let r = layer.channel_rect(id);
        Ok([r.top, r.left, r.bottom, r.right])
    }

    /// Planar samples in their original depth and colour mode, big-endian at 16/32 bits.
    pub fn decode_merged(&self) -> Result<Vec<u8>> {
        self.0.decode_merged()
    }

    pub fn decode_channel(&self, layer: usize, id: i16) -> Result<Vec<u8>> {
        self.layer(layer)?.decode_channel(id, self.0.header.depth, self.0.header.version)
    }

    /// Empty when the document has no embedded ICC profile.
    pub fn icc_profile(&self) -> &[u8] {
        self.0.icc_profile().unwrap_or_default()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.0.to_bytes()
    }

    pub fn header_info(&self) -> HeaderInfo {
        let h = self.header();
        HeaderInfo {
            width: h.width,
            height: h.height,
            version: h.version,
            channels: h.channels,
            depth: h.depth,
            color_mode: h.color_mode,
            layer_count: h.layer_count,
            compression: self.0.image_data.compression.as_u16(),
            merged_has_alpha: self.0.merged_has_alpha(),
            has_real_merged_data: self.0.has_real_merged_data(),
        }
    }

    pub fn layer_metadata(&self, index: usize) -> Result<LayerMetadata> {
        let l = self.layer(index)?;
        Ok(LayerMetadata {
            name: l.name(),
            bounds: l.rect.into(),
            channel_count: l.channels.len(),
            blend_mode: String::from_utf8_lossy(&l.blend_mode.key()).into_owned(),
            blend_mode_key: l.blend_mode.key(),
            opacity: l.opacity,
            fill_opacity: l.fill_opacity(),
            clipping: l.clipping,
            visible: l.is_visible(),
            section_type: l.section_type().as_u32(),
            id: l.layer_id(),
            mask: self.mask_info(index)?,
        })
    }

    pub(crate) fn mask_info(&self, index: usize) -> Result<Option<MaskInfo>> {
        Ok(self.layer(index)?.layer_mask().map(|m| MaskInfo {
            bounds: m.rect.into(),
            default_color: m.default_color,
            flags: m.flags,
            user_density: m.parameters.and_then(|p| p.user_density),
            user_feather: m.parameters.and_then(|p| p.user_feather),
            vector_density: m.parameters.and_then(|p| p.vector_density),
            vector_feather: m.parameters.and_then(|p| p.vector_feather),
            real_bounds: m.real.map(|r| r.rect.into()),
            real_flags: m.real.map(|r| r.flags),
            real_background: m.real.map(|r| r.background),
        }))
    }

    pub fn channel_compression(&self, layer: usize, id: i16) -> Result<Option<u16>> {
        self.layer(layer)?
            .channel(id)
            .map(|c| c.compression.map(|compression| compression.as_u16()))
            .ok_or_else(|| PsdError::Invalid(format!("layer has no channel {id}")))
    }

    pub fn color_mode_data(&self) -> &[u8] {
        &self.0.color_mode_data
    }

    pub fn resource(&self, id: u16) -> Option<&[u8]> {
        self.0.resource(id).map(|r| r.data.as_slice())
    }

    pub fn resolution(&self) -> Option<ResolutionInfo> {
        self.0.resolution().map(|r| ResolutionInfo {
            horizontal: r.h_res(),
            vertical: r.v_res(),
            horizontal_unit: r.h_res_unit,
            vertical_unit: r.v_res_unit,
            width_unit: r.width_unit,
            height_unit: r.height_unit,
        })
    }

    /// Flat preorder tree: parent is a layer-record index, never an offset in this array.
    pub fn layer_tree(&self) -> Result<Vec<TreeNode>> {
        let mut depth = 0usize;
        for layer in self.0.layers() {
            match layer.section_type() {
                SectionType::BoundingDivider => depth += 1,
                SectionType::OpenFolder | SectionType::ClosedFolder => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth > 256 {
                return Err(PsdError::LimitExceeded("layer tree exceeds 256 levels"));
            }
        }
        let tree = self.0.layer_tree();
        let mut pending: Vec<_> = tree.iter().rev().map(|node| (node, None)).collect();
        let mut result = Vec::new();
        while let Some((node, parent)) = pending.pop() {
            let divider = match node {
                LayerNode::Group { divider, .. } => *divider,
                _ => None,
            };
            result.push(TreeNode { index: node.index(), parent, divider, is_group: node.is_group() });
            pending.extend(node.children().iter().rev().map(|child| (child, Some(node.index()))));
        }
        Ok(result)
    }
}

// Bound the core preview helpers' pixel arithmetic and their output allocation.
pub(crate) fn preview_size(width: usize, height: usize) -> Result<()> {
    let bytes = width.checked_mul(height).and_then(|n| n.checked_mul(4));
    if bytes.is_none_or(|n| n > 256 * 1024 * 1024) {
        return Err(PsdError::LimitExceeded("RGBA8 preview exceeds 256 MiB"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_psd::testgen;

    #[test]
    fn adapters_preserve_generated_files_samples_and_channel_geometry() {
        for case in testgen::all_cases() {
            let file = case.file;
            let bytes = file.to_bytes().unwrap();
            let doc = Document::parse(&bytes).unwrap();
            let h = doc.header();
            assert_eq!(
                (h.width, h.height, h.depth, h.color_mode, h.version, h.channels, h.layer_count),
                (
                    file.header.width,
                    file.header.height,
                    file.header.depth,
                    file.header.color_mode.as_u16(),
                    file.header.version.as_u16(),
                    file.header.channels,
                    file.layers().len()
                ),
                "{}",
                case.name
            );
            assert_eq!(doc.to_bytes().unwrap(), bytes, "{}", case.name);
            assert_eq!(doc.decode_merged().unwrap(), file.decode_merged().unwrap());
            assert_eq!(doc.icc_profile(), file.icc_profile().unwrap_or_default());
            assert_eq!(doc.header_info().compression, file.image_data.compression.as_u16());
            assert_eq!(doc.header_info().merged_has_alpha, file.merged_has_alpha());
            assert_eq!(doc.header_info().has_real_merged_data, file.has_real_merged_data());
            assert_eq!(doc.color_mode_data(), file.color_mode_data);
            for resource in &file.resources {
                assert_eq!(doc.resource(resource.id), Some(resource.data.as_slice()));
            }
            assert!(doc.resource(65535).is_none());
            if let Some(r) = file.resolution() {
                assert_eq!(doc.resolution().unwrap().horizontal, r.h_res());
            }
            for node in doc.layer_tree().unwrap() {
                assert_eq!(node.is_group, file.layers()[node.index].section_type().is_folder());
                if let Some(parent) = node.parent {
                    assert!(file.layers()[parent].section_type().is_folder());
                }
            }
            for (i, layer) in file.layers().iter().enumerate() {
                let info = doc.layer_info(i).unwrap();
                assert_eq!(
                    (info.top, info.left, info.bottom, info.right, info.channel_count),
                    (layer.rect.top, layer.rect.left, layer.rect.bottom, layer.rect.right, layer.channels.len())
                );
                assert_eq!(doc.layer_name(i).unwrap(), layer.name());
                let metadata = doc.layer_metadata(i).unwrap();
                assert_eq!(metadata.blend_mode_key, layer.blend_mode.key());
                assert_eq!(
                    (metadata.opacity, metadata.fill_opacity, metadata.visible, metadata.clipping, metadata.id),
                    (layer.opacity, layer.fill_opacity(), layer.is_visible(), layer.clipping, layer.layer_id())
                );
                assert_eq!(metadata.mask.is_some(), layer.layer_mask().is_some());
                if let Some(mask) = layer.layer_mask() {
                    let m = metadata.mask.unwrap();
                    assert_eq!((m.bounds, m.flags, m.default_color), (mask.rect.into(), mask.flags, mask.default_color));
                    assert_eq!(m.user_feather, mask.parameters.and_then(|p| p.user_feather));
                    assert_eq!(m.real_bounds, mask.real.map(|r| r.rect.into()));
                }
                for (j, ch) in layer.channels.iter().enumerate() {
                    assert_eq!(doc.channel_id(i, j).unwrap(), ch.id);
                    assert_eq!(doc.channel_compression(i, ch.id).unwrap(), ch.compression.map(|c| c.as_u16()));
                    let r = layer.channel_rect(ch.id);
                    assert_eq!(doc.channel_bounds(i, ch.id).unwrap(), [r.top, r.left, r.bottom, r.right]);
                    assert_eq!(doc.decode_channel(i, ch.id).unwrap(), layer.decode_channel(ch.id, h.depth, file.header.version).unwrap());
                }
            }
            assert!(doc.layer_info(usize::MAX).is_err());
            assert!(doc.layer_name(usize::MAX).is_err());
            assert!(doc.channel_id(0, usize::MAX).is_err());
            assert!(doc.channel_bounds(0, i16::MAX).is_err());
            assert!(doc.decode_channel(usize::MAX, 0).is_err());
            assert!(doc.layer_metadata(usize::MAX).is_err());
            assert!(doc.layer_rgba8(usize::MAX).is_err());
            assert!(doc.channel_compression(usize::MAX, 0).is_err());
        }
        assert!(Document::parse(b"not a PSD").is_err());
    }

    #[test]
    fn preview_and_tree_limits_reject_input_before_allocating_or_recursing() {
        use photocraft_psd::{ColorMode, Compression, TaggedBlock, Version};
        assert!(preview_size(usize::MAX, 2).is_err());
        assert!(preview_size(8193, 8192).is_err());
        assert!(preview_size(6000, 6000).is_ok());
        let mut file = testgen::merged_only(Version::Psd, ColorMode::Rgb, 8, Compression::Raw, 1, 1);
        file.header.width = 300000;
        file.header.height = 300000;
        assert!(Document::from(file).merged_rgba8().is_err());
        let mut file = testgen::layered(Version::Psd, ColorMode::Rgb, 8, Compression::Raw);
        file.header.color_mode = ColorMode::Indexed;
        file.color_mode_data.clear();
        let doc = Document::from(file);
        assert!(doc.layer_rgba8(0).is_err());
        assert!(doc.merged_rgba8().is_err());
        let mut file = testgen::layered(Version::Psd, ColorMode::Rgb, 8, Compression::Raw);
        let divider = LayerRecord { blocks: vec![TaggedBlock::section_divider(SectionType::BoundingDivider, None, None)], ..LayerRecord::default() };
        file.layers_mut().splice(0..0, std::iter::repeat_n(divider, 257));
        assert!(Document::from(file).layer_tree().is_err());
    }
}
