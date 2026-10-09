use crate::{BlockInfo, Document, ResourceInfo, ThumbnailInfo};
use photocraft_psd::{ColorMode, ImageResource, PsdError, Result, TaggedBlock};

impl Document {
    pub(crate) fn transparent_index(&self) -> Result<Option<u8>> {
        if self.0.header.color_mode != ColorMode::Indexed {
            return Ok(None);
        }
        let Some(data) = self.resource(1047) else { return Ok(None) };
        let bytes = data.try_into().map_err(|_| PsdError::Invalid("transparency index requires two bytes".into()))?;
        Ok(u8::try_from(u16::from_be_bytes(bytes)).ok())
    }

    fn resource_at(&self, index: usize) -> Result<&ImageResource> {
        self.0.resources.get(index).ok_or_else(|| PsdError::Invalid("resource index out of range".into()))
    }
    pub fn resource_count(&self) -> usize {
        self.0.resources.len()
    }
    pub fn resource_info(&self, index: usize) -> Result<ResourceInfo> {
        let r = self.resource_at(index)?;
        Ok(ResourceInfo { signature: r.signature, id: r.id, name_len: r.name.len(), data_len: r.data.len() })
    }
    pub fn resource_name(&self, index: usize) -> Result<&[u8]> {
        Ok(&self.resource_at(index)?.name)
    }
    pub fn resource_data(&self, index: usize) -> Result<&[u8]> {
        Ok(&self.resource_at(index)?.data)
    }
    pub fn layer_block_count(&self, layer: usize) -> Result<usize> {
        Ok(self.layer(layer)?.blocks.len())
    }
    pub fn global_block_count(&self) -> usize {
        self.0.global_blocks.len()
    }
    pub fn layer_block_info(&self, layer: usize, index: usize) -> Result<BlockInfo> {
        block_info(&self.layer(layer)?.blocks, index)
    }
    pub fn global_block_info(&self, index: usize) -> Result<BlockInfo> {
        block_info(&self.0.global_blocks, index)
    }
    pub fn layer_block_data(&self, layer: usize, index: usize) -> Result<&[u8]> {
        Ok(&block_at(&self.layer(layer)?.blocks, index)?.data)
    }
    pub fn global_block_data(&self, index: usize) -> Result<&[u8]> {
        Ok(&block_at(&self.0.global_blocks, index)?.data)
    }

    /// Prefer the RGB thumbnail (1036); legacy 1033 stores BGR colours.
    pub fn thumbnail_info(&self) -> Result<Option<ThumbnailInfo>> {
        let Some(r) = self.0.resource(1036).or_else(|| self.0.resource(1033)) else { return Ok(None) };
        let data = &r.data;
        let invalid = || PsdError::Invalid("invalid thumbnail header or payload".into());
        let u32_at =
            |offset: usize| -> Result<u32> { Ok(u32::from_be_bytes(data.get(offset..offset + 4).ok_or_else(invalid)?.try_into().map_err(|_| invalid())?)) };
        let (format, width, height, row_bytes, total, stored) = (u32_at(0)?, u32_at(4)?, u32_at(8)?, u32_at(12)?, u32_at(16)?, u32_at(20)?);
        let row = u64::from(width).checked_mul(3).ok_or_else(invalid)?.div_ceil(4) * 4;
        let total_bytes = row.checked_mul(u64::from(height)).ok_or_else(invalid)?;
        if format > 1
            || width == 0
            || height == 0
            || row != u64::from(row_bytes)
            || u64::from(total) != total_bytes
            || data.get(24..28) != Some(&[0, 24, 0, 1])
            || data.len().saturating_sub(28) != stored as usize
            || stored == 0
            || (format == 0 && stored != total)
        {
            return Err(invalid());
        }
        Ok(Some(ThumbnailInfo { resource_id: r.id, format, width, height, row_bytes, bgr: r.id == 1033 }))
    }
    pub fn thumbnail_data(&self) -> Result<Option<&[u8]>> {
        Ok(self.thumbnail_info()?.and_then(|t| self.resource(t.resource_id)).and_then(|data| data.get(28..)))
    }
}

fn block_at(blocks: &[TaggedBlock], index: usize) -> Result<&TaggedBlock> {
    blocks.get(index).ok_or_else(|| PsdError::Invalid("block index out of range".into()))
}
fn block_info(blocks: &[TaggedBlock], index: usize) -> Result<BlockInfo> {
    let b = block_at(blocks, index)?;
    Ok(BlockInfo { signature: b.signature, key: b.key, data_len: b.data.len() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_psd::{ColorMode, Compression, Version, testgen};

    #[test]
    fn enumeration_keeps_duplicate_ids_keys_and_raw_names() {
        let mut file = testgen::layered(Version::Psd, ColorMode::Rgb, 8, Compression::Raw);
        let resource = ImageResource { name: vec![255], ..ImageResource::new(2000, b"second".to_vec()) };
        file.resources.push(resource.clone());
        let block = TaggedBlock::new(*b"TEST", vec![1, 2, 3]);
        file.layers_mut()[0].blocks.extend([block.clone(), block.clone()]);
        file.global_blocks.clear();
        file.global_blocks.extend([block.clone(), block]);
        let doc = Document::parse(&file.to_bytes().unwrap()).unwrap();
        assert_eq!(doc.resource_count(), file.resources.len());
        let i = doc.resource_count() - 1;
        assert_eq!(doc.resource_info(i).unwrap().id, 2000);
        assert_eq!(doc.resource_name(i).unwrap(), [255]);
        assert_eq!(doc.resource_data(i).unwrap(), b"second");
        assert_ne!(doc.resource(2000).unwrap(), doc.resource_data(i).unwrap());
        let i = doc.layer_block_count(0).unwrap() - 1;
        assert_eq!(doc.layer_block_info(0, i).unwrap().key, *b"TEST");
        assert_eq!(doc.layer_block_data(0, i).unwrap(), [1, 2, 3]);
        assert_eq!(doc.global_block_count(), 2);
        assert_eq!(doc.global_block_info(1).unwrap().data_len, 3);
        assert_eq!(doc.global_block_data(0).unwrap(), doc.global_block_data(1).unwrap());
        assert!(doc.resource_info(usize::MAX).is_err() && doc.resource_name(usize::MAX).is_err() && doc.resource_data(usize::MAX).is_err());
        assert!(doc.layer_block_count(usize::MAX).is_err() && doc.layer_block_info(0, usize::MAX).is_err() && doc.layer_block_data(0, usize::MAX).is_err());
        assert!(doc.global_block_info(usize::MAX).is_err() && doc.global_block_data(usize::MAX).is_err());
    }

    #[test]
    fn thumbnail_headers_select_rgb_and_validate_raw_jpeg_and_legacy_payloads() {
        let mut file = testgen::merged_only(Version::Psd, ColorMode::Rgb, 8, Compression::Raw, 1, 1);
        file.resources.clear();
        let mut doc = Document::from(file);
        assert!(doc.thumbnail_info().unwrap().is_none() && doc.thumbnail_data().unwrap().is_none());
        let mut data: Vec<_> = [0u32, 1, 1, 4, 4, 4].into_iter().flat_map(u32::to_be_bytes).collect();
        data.extend([0, 24, 0, 1, 0, 0, 255, 0]);
        doc.0.resources.push(ImageResource::new(1033, data.clone()));
        assert!(doc.thumbnail_info().unwrap().unwrap().bgr);
        data[28..32].copy_from_slice(&[255, 0, 0, 0]);
        doc.0.resources.insert(0, ImageResource::new(1036, data));
        assert!(!doc.thumbnail_info().unwrap().unwrap().bgr);
        assert_eq!(doc.thumbnail_data().unwrap().unwrap(), [255, 0, 0, 0]);
        let data = &mut doc.0.resources[0].data;
        data[0..4].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(doc.thumbnail_info().unwrap().unwrap().format, 1);
        for length in [0, 24, 27, 28, 31] {
            let original = doc.0.resources[0].data.clone();
            doc.0.resources[0].data.truncate(length);
            assert!(doc.thumbnail_data().is_err());
            doc.0.resources[0].data = original;
        }
        doc.0.resources[0].data[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(doc.thumbnail_info().is_err());
    }
}
