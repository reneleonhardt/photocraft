use crate::Document;
use photocraft_psd::{ChannelData, Compression, LayerInfoPlacement, PsdError, Result, TaggedBlock};

impl Document {
    /// Returns an independent document; untouched blocks and channels retain their bytes.
    pub fn edit_layer(&self, index: usize, name: Option<&str>, visible: Option<bool>) -> Result<Self> {
        let original = self.layer(index)?;
        if name.is_some_and(|n| n.len() > 1024 * 1024) {
            return Err(PsdError::LimitExceeded("layer name exceeds 1 MiB"));
        }
        let changed = visible.is_some_and(|v| v != original.is_visible());
        let mut file = self.0.clone();
        let layer = file.layers_mut().get_mut(index).ok_or_else(|| PsdError::Invalid("layer index out of range".into()))?;
        if let Some(name) = name {
            layer.name = name.chars().take(255).map(|c| if c.is_ascii() && !c.is_ascii_control() { c as u8 } else { b'?' }).collect();
            let block = TaggedBlock::unicode_name(name);
            if let Some(old) = layer.block_mut(b"luni") {
                *old = block;
            } else {
                layer.blocks.push(block);
            }
        }
        if let Some(visible) = visible {
            layer.flags.set_hidden(!visible);
        }
        let mut result = Self::from(file);
        if name.is_some() || changed {
            result.reset_layer_padding();
        }
        if changed {
            result.invalidate_composite()?;
        }
        Ok(result)
    }

    /// Replaces an existing channel using native decoded samples at unchanged bounds.
    pub fn replace_channel(&self, index: usize, id: i16, data: &[u8]) -> Result<Self> {
        let layer = self.layer(index)?;
        let channel = layer.channels.iter().position(|c| c.id == id).ok_or_else(|| PsdError::Invalid("channel id out of range".into()))?;
        if data.len() > 1024 * 1024 * 1024 {
            return Err(PsdError::LimitExceeded("channel input exceeds 1 GiB"));
        }
        let (w, h) = layer.channel_rect(id).size()?;
        let compression = layer.channels.get(channel).and_then(|c| c.compression).unwrap_or(Compression::Rle);
        let encoded = ChannelData::encode(id, compression, data, w, h, self.0.header.depth, self.0.header.version)?;
        let mut file = self.0.clone();
        let target =
            file.layers_mut().get_mut(index).and_then(|l| l.channels.get_mut(channel)).ok_or_else(|| PsdError::Invalid("channel id out of range".into()))?;
        *target = encoded;
        let mut result = Self::from(file);
        result.reset_layer_padding();
        result.invalidate_composite()?;
        Ok(result)
    }

    fn reset_layer_padding(&mut self) {
        if let Some(info) = &mut self.0.layer_info {
            info.padding = None;
        }
        if let LayerInfoPlacement::GlobalBlock { padding, .. } = &mut self.0.layer_info_placement {
            *padding = None;
        }
    }

    fn invalidate_composite(&mut self) -> Result<()> {
        // Preserve encoded composite bytes, but never advertise them as the edited appearance.
        self.0.resources.retain(|r| !matches!(r.id, 1033 | 1036));
        let mut found = false;
        for resource in self.0.resources.iter_mut().filter(|r| r.id == 1057) {
            photocraft_psd::resources::VersionInfo::from_bytes(&resource.data)?;
            let flag = resource.data.get_mut(4).ok_or_else(|| PsdError::Invalid("truncated version info".into()))?;
            *flag = 0;
            found = true;
        }
        if !found {
            self.0.resources.push(photocraft_psd::resources::version_info_resource(false));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_psd::{ColorMode, Version, testgen};

    #[test]
    fn edits_validate_names_and_saved_composite_flags() {
        let mut doc = Document::from(testgen::layered(Version::Psd, ColorMode::Rgb, 8, Compression::Raw));
        doc.0.layer_info.as_mut().unwrap().padding = Some(vec![0; 2]);
        assert_eq!(doc.edit_layer(0, None, None).unwrap().to_bytes().unwrap(), doc.to_bytes().unwrap());
        assert!(doc.edit_layer(0, Some(&"x".repeat(1024 * 1024 + 1)), None).is_err());
        let original = doc.resource(1057).unwrap().to_vec();
        let hidden = doc.edit_layer(0, None, Some(false)).unwrap();
        let mut expected = original;
        expected[4] = 0;
        assert_eq!(hidden.resource(1057).unwrap(), expected);
        doc.0.resources.retain(|r| r.id != 1057);
        assert_eq!(doc.edit_layer(0, None, Some(false)).unwrap().header_info().has_real_merged_data, Some(false));
        doc.0.resources.push(photocraft_psd::ImageResource::new(1057, vec![0; 4]));
        assert!(doc.edit_layer(0, None, Some(false)).is_err());
        assert!(doc.layer_metadata(0).unwrap().visible);
    }

    #[test]
    fn edits_preserve_opaque_data_and_invalidate_saved_appearance_at_all_depths() {
        for version in [Version::Psd, Version::Psb] {
            for depth in [8, 16, 32] {
                for compression in Compression::ALL {
                    let mut file = testgen::layered(version, ColorMode::Rgb, depth, compression);
                    file.resources.push(photocraft_psd::ImageResource::new(1036, vec![1]));
                    file.global_blocks.push(TaggedBlock::new(*b"TEST", vec![3]));
                    file.layers_mut()[0].blocks.push(TaggedBlock::new(*b"TEST", vec![4]));
                    let doc = Document::parse(&file.to_bytes().unwrap()).unwrap();
                    let renamed = doc.edit_layer(0, Some("Ink 🎨"), None).unwrap();
                    let reopened = Document::parse(&renamed.to_bytes().unwrap()).unwrap();
                    assert_eq!(reopened.layer_name(0).unwrap(), "Ink 🎨");
                    assert_eq!(reopened.decode_merged().unwrap(), doc.decode_merged().unwrap());
                    assert_eq!(reopened.resource(1036), Some(&[1][..]));
                    let bytes = doc.decode_channel(0, 0).unwrap();
                    let changed = renamed.replace_channel(0, 0, &vec![0; bytes.len()]).unwrap();
                    let hidden = changed.edit_layer(0, None, Some(false)).unwrap();
                    let reopened = Document::parse(&hidden.to_bytes().unwrap()).unwrap();
                    assert!(!reopened.layer_metadata(0).unwrap().visible);
                    assert_eq!(reopened.decode_channel(0, 0).unwrap(), vec![0; bytes.len()]);
                    assert_eq!(reopened.0.global_blocks, doc.0.global_blocks);
                    assert_eq!(reopened.layer(0).unwrap().block(b"TEST"), doc.layer(0).unwrap().block(b"TEST"));
                    assert_eq!(reopened.header_info().has_real_merged_data, Some(false));
                    assert!(reopened.resource(1036).is_none());
                    assert!(reopened.merged_pixels_f32().is_err());
                    assert_eq!(doc.decode_channel(0, 0).unwrap(), bytes);
                    assert!(doc.edit_layer(usize::MAX, None, None).is_err());
                    assert!(doc.replace_channel(0, 123, &[]).is_err());
                    assert!(doc.replace_channel(0, 0, &[]).is_err());
                    for (layer, id) in [(1, -2), (7, -2), (7, -3), (2, 0)] {
                        let pixels = vec![0; doc.decode_channel(layer, id).unwrap().len()];
                        let edited = doc.replace_channel(layer, id, &pixels).unwrap();
                        let reopened = Document::parse(&edited.to_bytes().unwrap()).unwrap();
                        assert_eq!(reopened.decode_channel(layer, id).unwrap(), pixels);
                        assert_eq!(reopened.channel_bounds(layer, id).unwrap(), doc.channel_bounds(layer, id).unwrap());
                    }
                }
            }
        }
    }
}
