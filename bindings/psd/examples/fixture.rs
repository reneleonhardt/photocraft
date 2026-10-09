//! Small, deterministic fixtures for consumer checks; no external corpus required.
use photocraft_psd::{ChannelData, Compression, GroupSpec, ImageData, ImageResource, LayerSpec, MaskSpec, PixelData, PsdBuilder, Rect, Version};
use photocraft_psd::{
    TaggedBlock,
    path::{PathData, PsdKnot, PsdSubpath, VectorMaskBlock},
    slices::{SliceRecord, SlicesResource},
};
use std::path::Path;

fn fixture(version: Version, depth: u16, compression: Compression, grouped: bool) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let pixels = match depth {
        8 => PixelData::Rgba8(vec![255, 0, 0, 255, 255, 0, 0, 128]),
        16 | 32 => PixelData::Rgba16(vec![65535, 0, 0, 65535, 65535, 0, 0, 32768]),
        _ => return Err("unsupported fixture depth".into()),
    };
    let mut builder = PsdBuilder::new(2, 1)
        .version(version)
        .depth(depth.min(16))
        .compression(compression)
        .icc_profile(photocraft_cms::Builtin::Srgb.profile().to_bytes().as_ref().clone())
        .resolution(300.0)
        .resource(ImageResource::new(1060, b"<xmp/>".to_vec()));
    let path = PathData::from_subpaths(
        &[PsdSubpath {
            closed: true,
            operation: 1,
            knots: [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)].into_iter().map(|p| PsdKnot { pre: p, anchor: p, post: p, linked: true }).collect(),
        }],
        false,
    );
    builder = builder.resource(ImageResource { name: b"Outline".to_vec(), ..ImageResource::new(2000, path.to_bytes()) }).resource(ImageResource::new(
        1050,
        SlicesResource {
            version: 6,
            bounds: [0, 0, 1, 2],
            group_name: "assets".into(),
            slices: vec![SliceRecord {
                id: 7,
                origin: 1,
                layer_id: Some(if grouped { 2 } else { 1 }),
                kind: 1,
                name: "red".into(),
                rect: [0, 0, 2, 1],
                alt: "Red".into(),
                color: [255, 255, 0, 0],
                ..Default::default()
            }],
        }
        .to_bytes(),
    ));
    let mut layer = LayerSpec::new("Red ☺", -1, 2, 2, 1, pixels.clone());
    layer.extra_blocks.push(TaggedBlock::new(*b"vmsk", VectorMaskBlock { version: 3, flags: 4, path }.to_bytes()));
    if grouped {
        builder.begin_group(GroupSpec::new("Folder"));
        layer.opacity = 127;
        layer.fill_opacity = Some(128);
        layer.visible = false;
        layer.clipping = true;
        layer.mask = Some(MaskSpec { rect: Rect::from_xywh(0, 2, 1, 1), default_color: 255, disabled: true, data: vec![128] });
    }
    builder.push_layer(layer);
    if grouped {
        builder.end_group()?;
    }
    builder.composite(pixels);
    let mut file = builder.build()?;
    if depth == 32 {
        let floats = |raw: &[u8]| -> Vec<u8> {
            raw.as_chunks::<2>()
                .0
                .iter()
                .flat_map(|sample| {
                    let value = u16::from_be_bytes(*sample);
                    (if value == 65535 { 1.0f32 } else { f32::from(value) / 65536.0 }).to_be_bytes()
                })
                .collect()
        };
        let merged: Vec<_> = [1.0f32, 1.0, 0.0, 0.5, 0.0, 0.5, 1.0, 0.5].into_iter().flat_map(f32::to_be_bytes).collect();
        for layer in file.layers_mut() {
            for index in 0..layer.channels.len() {
                let id = layer.channels.get(index).ok_or("missing fixture channel")?.id;
                let (w, h) = layer.channel_rect(id).size()?;
                let raw = layer.decode_channel(id, 16, version)?;
                *layer.channels.get_mut(index).ok_or("missing fixture channel")? = ChannelData::encode(id, compression, &floats(&raw), w, h, 32, version)?;
            }
        }
        file.header.depth = 32;
        file.image_data = ImageData::encode(compression, &merged, &file.header)?;
    }
    let mut thumbnail: Vec<_> = [0u32, 2, 1, 8, 8, 8].into_iter().flat_map(u32::to_be_bytes).collect();
    thumbnail.extend([0, 24, 0, 1, 255, 0, 0, 255, 0, 0, 0, 0]);
    file.resources.push(ImageResource::new(1036, thumbnail));
    if let Some(r) = file.resources.iter_mut().find(|r| r.id == 1060) {
        r.name = b"XMP".to_vec();
    }
    file.global_blocks.push(photocraft_psd::TaggedBlock::new(*b"TEST", b"global".to_vec()));
    Ok(file.to_bytes()?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let path = args.first().ok_or("usage: fixture <output.psd> | --matrix <directory>")?;
    if path == "--matrix" {
        let dir = Path::new(args.get(1).ok_or("missing directory")?);
        std::fs::create_dir_all(dir)?;
        for version in [Version::Psd, Version::Psb] {
            for depth in [8, 16, 32] {
                for compression in Compression::ALL {
                    let name = format!("v{}-d{depth}-c{}.psd", version.as_u16(), compression.as_u16());
                    std::fs::write(dir.join(name), fixture(version, depth, compression, true)?)?;
                }
            }
        }
    } else {
        std::fs::write(path, fixture(Version::Psd, 16, Compression::Rle, false)?)?;
    }
    Ok(())
}
