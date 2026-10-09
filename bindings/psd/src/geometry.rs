use crate::{Document, KnotInfo, PathInfo, SliceInfo, SlicesInfo, SubpathInfo};
use photocraft_psd::{
    PsdError, Result,
    path::{PathData, PsdSubpath, VectorMaskBlock},
    slices::SlicesResource,
};

pub struct Path {
    info: PathInfo,
    subpaths: Vec<PsdSubpath>,
}
impl Path {
    fn parse(data: &[u8], vector: bool) -> Result<Self> {
        Self::parse_with_limit(data, vector, 256 * 1024 * 1024)
    }
    fn parse_with_limit(data: &[u8], vector: bool, limit: usize) -> Result<Self> {
        if data.len().saturating_sub(if vector { 8 } else { 0 }) % 26 != 0 {
            return Err(PsdError::Invalid("truncated path record".into()));
        }
        let (path, version, flags) = if vector {
            let mask = VectorMaskBlock::from_bytes(data)?;
            if mask.version != 3 {
                return Err(PsdError::Unsupported("vector mask version".into()));
            }
            (mask.path, mask.version, mask.flags)
        } else {
            (PathData::from_bytes(data)?, 0, 0)
        };
        // Allow for growth and the minimum capacity of small vectors.
        let mut allocation = path.records.len().saturating_mul(std::mem::size_of::<photocraft_psd::path::PathRecord>());
        for record in &path.records {
            if record.is_length() {
                allocation = allocation.saturating_add(4 * std::mem::size_of::<PsdSubpath>());
            } else if record.is_knot() {
                allocation = allocation.saturating_add(4 * (std::mem::size_of::<photocraft_psd::path::PsdKnot>() + std::mem::size_of::<PsdSubpath>()));
            }
        }
        if allocation > limit {
            return Err(PsdError::LimitExceeded("typed path allocation exceeds 256 MiB"));
        }
        let subpaths = path.subpaths();
        Ok(Self { info: PathInfo { subpath_count: subpaths.len(), version, flags, initial_fill: path.initial_fill(), is_vector_mask: vector }, subpaths })
    }
    pub fn info(&self) -> PathInfo {
        self.info.clone()
    }
    fn subpath(&self, index: usize) -> Result<&PsdSubpath> {
        self.subpaths.get(index).ok_or_else(|| PsdError::Invalid("subpath index out of range".into()))
    }
    pub fn subpath_info(&self, index: usize) -> Result<SubpathInfo> {
        let s = self.subpath(index)?;
        Ok(SubpathInfo { knot_count: s.knots.len(), operation: s.operation, closed: s.closed })
    }
    /// Coordinates are fractions of the document size, in x/y order.
    pub fn knot(&self, subpath: usize, index: usize) -> Result<KnotInfo> {
        let k = self.subpath(subpath)?.knots.get(index).ok_or_else(|| PsdError::Invalid("knot index out of range".into()))?;
        Ok(KnotInfo { pre: [k.pre.0, k.pre.1], anchor: [k.anchor.0, k.anchor.1], post: [k.post.0, k.post.1], linked: k.linked })
    }
}

pub struct Slices(SlicesResource);
impl Slices {
    pub fn info(&self) -> SlicesInfo {
        SlicesInfo { bounds: self.0.bounds, count: self.0.slices.len(), version: self.0.version }
    }
    pub fn group_name(&self) -> &str {
        &self.0.group_name
    }
    fn slice(&self, index: usize) -> Result<&photocraft_psd::slices::SliceRecord> {
        self.0.slices.get(index).ok_or_else(|| PsdError::Invalid("slice index out of range".into()))
    }
    pub fn slice_info(&self, index: usize) -> Result<SliceInfo> {
        let s = self.slice(index)?;
        Ok(SliceInfo {
            id: s.id,
            group_id: s.group_id,
            origin: s.origin,
            layer_id: s.layer_id.unwrap_or_default(),
            kind: s.kind,
            bounds: s.rect,
            outsets: s.outsets,
            horizontal_align: s.horizontal_align,
            vertical_align: s.vertical_align,
            color: s.color,
            has_layer_id: s.layer_id.is_some(),
            cell_text_is_html: s.cell_text_is_html,
        })
    }
    pub fn text(&self, index: usize, field: &str) -> Result<&str> {
        let s = self.slice(index)?;
        match field {
            "name" => Ok(&s.name),
            "url" => Ok(&s.url),
            "target" => Ok(&s.target),
            "message" => Ok(&s.message),
            "alt" => Ok(&s.alt),
            "cell_text" => Ok(&s.cell_text),
            _ => Err(PsdError::Invalid("unknown slice text field".into())),
        }
    }
}

impl Document {
    pub fn resource_path(&self, index: usize) -> Result<Option<Path>> {
        let id = self.resource_info(index)?.id;
        if id != 1025 && !(2000..=2997).contains(&id) {
            return Ok(None);
        }
        Path::parse(self.resource_data(index)?, false).map(Some)
    }
    pub fn vector_mask(&self, layer: usize) -> Result<Option<Path>> {
        let layer = self.layer(layer)?;
        layer.block(b"vmsk").or_else(|| layer.block(b"vsms")).map(|block| Path::parse(&block.data, true)).transpose()
    }
    pub fn slices(&self) -> Result<Option<Slices>> {
        self.resource(1050).map(SlicesResource::from_bytes).transpose().map(|s| s.map(Slices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_psd::{
        ImageResource, PsdBuilder,
        path::{PathRecord, PsdKnot, selector},
        slices::SliceRecord,
    };

    #[test]
    fn owned_geometry_preserves_coordinates_flags_and_slice_fields() {
        let point = (0.25, -0.5);
        let path = PathData::from_subpaths(
            &[PsdSubpath { closed: false, operation: -1, knots: vec![PsdKnot { pre: point, anchor: point, post: point, linked: false }] }],
            true,
        );
        let mask = VectorMaskBlock { version: 3, flags: 7, path: path.clone() };
        let parsed = Path::parse(&mask.to_bytes(), true).unwrap();
        assert_eq!((parsed.info().version, parsed.info().flags, parsed.info().initial_fill), (3, 7, true));
        assert_eq!(parsed.subpath_info(0).unwrap(), SubpathInfo { knot_count: 1, operation: -1, closed: false });
        assert_eq!(parsed.knot(0, 0).unwrap(), KnotInfo { pre: [0.25, -0.5], anchor: [0.25, -0.5], post: [0.25, -0.5], linked: false });
        assert!(parsed.knot(0, 1).is_err() && parsed.subpath_info(1).is_err());
        let slices = SlicesResource {
            version: 6,
            bounds: [1, 2, 4, 6],
            group_name: "assets".into(),
            slices: vec![SliceRecord {
                id: 7,
                origin: 1,
                layer_id: Some(2),
                name: "slice".into(),
                rect: [2, 1, 6, 4],
                outsets: [1, 2, 3, 4],
                url: "url".into(),
                target: "target".into(),
                message: "message".into(),
                alt: "alt".into(),
                cell_text: "html".into(),
                cell_text_is_html: true,
                color: [255, 1, 2, 3],
                horizontal_align: 1,
                vertical_align: 2,
                ..Default::default()
            }],
        };
        let mut v8 = slices.to_descriptor_bytes();
        v8[..4].copy_from_slice(&8u32.to_be_bytes());
        for bytes in [slices.to_bytes(), slices.to_descriptor_bytes(), v8] {
            let file = PsdBuilder::new(1, 1).resource(ImageResource::new(2000, path.to_bytes())).resource(ImageResource::new(1050, bytes)).build().unwrap();
            let doc = Document::from(file);
            let owned = doc.resource_path(0).unwrap().unwrap();
            assert!(doc.resource_path(1).unwrap().is_none());
            let s = doc.slices().unwrap().unwrap();
            drop(doc);
            assert_eq!(owned.knot(0, 0).unwrap().anchor, [0.25, -0.5]);
            assert_eq!((s.info().bounds, s.info().count, s.group_name()), ([1, 2, 4, 6], 1, "assets"));
            let info = s.slice_info(0).unwrap();
            assert_eq!((info.id, info.layer_id, info.has_layer_id, info.bounds), (7, 2, true, [2, 1, 6, 4]));
            for (field, value) in [("name", "slice"), ("url", "url"), ("target", "target"), ("message", "message"), ("alt", "alt"), ("cell_text", "html")] {
                assert_eq!(s.text(0, field).unwrap(), value);
            }
            assert!(s.slice_info(1).is_err() && s.text(0, "invalid").is_err());
        }
    }
    #[test]
    fn malformed_geometry_and_untrusted_counts_are_handled() {
        assert!(Path::parse(&[0; 25], false).is_err());
        assert!(Path::parse(&[0; 7], true).is_err());
        assert!(Path::parse(&VectorMaskBlock { version: 2, flags: 0, path: PathData::default() }.to_bytes(), true).is_err());
        let mut record = PathRecord { selector: selector::CLOSED_LENGTH, data: [0; 24] };
        record.data[..2].copy_from_slice(&65535u16.to_be_bytes());
        let hostile = PathData { records: vec![record; 2048] }.to_bytes();
        let parsed = Path::parse(&hostile, false).unwrap();
        assert_eq!(parsed.info().subpath_count, 2048);
        assert!(parsed.subpaths.iter().all(|s| s.knots.capacity() == 0));
        let knot = PathData { records: vec![PathRecord { selector: selector::CLOSED_KNOT_LINKED, data: [0; 24] }] }.to_bytes();
        assert!(matches!(Path::parse_with_limit(&knot, false, 256), Err(PsdError::LimitExceeded(_))));
        let doc = Document::from(PsdBuilder::new(1, 1).build().unwrap());
        assert!(doc.slices().unwrap().is_none());
        assert!(doc.resource_path(999).is_err() && doc.vector_mask(0).is_err());
        let doc = Document::from(PsdBuilder::new(1, 1).resource(ImageResource::new(1050, vec![0; 4])).build().unwrap());
        assert!(doc.slices().is_err());
    }
}
