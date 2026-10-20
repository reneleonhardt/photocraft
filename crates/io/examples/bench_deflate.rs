//! Temporary x86 deflate comparison: real photos, common PSD inputs, in-memory wall time.
use photocraft_codecs::{EncodeOptions, Format};
use photocraft_color::{ColorMode, SampleType};
use photocraft_doc::{Document, Layer};
use photocraft_geom::{Rect, Size};
use serde_json::{Value, json};
use std::{error::Error, hint::black_box, path::Path, sync::Arc, time::Instant};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn psd(doc: &Document) -> Result<Vec<u8>> {
    let options = photocraft_io::PsdExportOptions { merged_compression: photocraft_psd::Compression::Zip, ..Default::default() };
    Ok(photocraft_io::document_to_psd_with(doc, &options).0.to_bytes()?)
}

fn pixels(doc: &Document) -> Result<String> {
    let mut hash = blake3::Hasher::new();
    hash.update(&doc.size.width.to_le_bytes());
    hash.update(&doc.size.height.to_le_bytes());
    hash.update(format!("{:?}/{:?}/{}", doc.mode, doc.depth, doc.layers.len()).as_bytes());
    if let Some(profile) = &doc.icc_profile {
        hash.update(profile);
    }
    for layer in &doc.layers {
        let surface = layer.surface().ok_or("expected a photo layer")?;
        for sample in surface.read_region(doc.bounds()) {
            hash.update(&sample.to_bits().to_le_bytes());
        }
    }
    Ok(hash.finalize().to_hex().to_string())
}

fn prepare(photos: &Path, output: &Path) -> Result<()> {
    std::fs::create_dir_all(output)?;
    for (name, file, width, height) in [("canon", "IMG_4059.CR2", 4000, 3000), ("panasonic", "P1000475.RW2", 6000, 4000)] {
        let developed = photocraft_raw::develop(&std::fs::read(photos.join(file))?, &Default::default())?;
        if developed.width < width || developed.height < height {
            return Err("photo is smaller than the expected crop".into());
        }
        let x = (developed.width - width) / 2;
        let y = (developed.height - height) / 2;
        for depth in [SampleType::U8, SampleType::U16] {
            let mut doc = Document::new(name, Size::new(width, height), ColorMode::Rgb, depth);
            doc.icc_profile = Some(Arc::new(photocraft_cms::Builtin::ProPhotoCompat.profile().to_bytes().to_vec()));
            let mut layer = Layer::raster("Photo", doc.pixel_format());
            let mut row = Vec::with_capacity(width as usize * 4);
            for dy in 0..height {
                row.clear();
                for dx in 0..width {
                    let index = ((y + dy) as usize * developed.width as usize + (x + dx) as usize) * 3;
                    let rgb = developed.rgb.get(index..index + 3).ok_or("short developed photo")?;
                    row.extend(rgb.iter().map(|v| f32::from(*v) / 65535.0));
                    row.push(1.0);
                }
                layer.surface_mut().ok_or("missing raster")?.write_region(Rect::new(0, dy as i32, width as i32, dy as i32 + 1), &row);
            }
            doc.layers.push(layer);
            let case = format!("{name}-{}", if depth == SampleType::U8 { 8 } else { 16 });
            let base = output.join(&case);
            std::fs::write(base.with_extension("pcraft"), photocraft_format::save_to_bytes(&doc, &Default::default())?)?;
            let bytes = psd(&doc)?;
            let imported = photocraft_io::import("common.psd", &bytes)?.document;
            if pixels(&imported)? != pixels(&doc)? {
                return Err("PSD fixture changes photo pixels".into());
            }
            std::fs::write(base.with_extension("psd"), &bytes)?;
            println!(
                "{}",
                json!({"case":case,"width":width,"height":height,"depth":format!("{depth:?}"),"profile":"ProPhoto RGB","psd_hash":blake3::hash(&bytes).to_hex().to_string()})
            );
        }
    }
    Ok(())
}

fn measure(folder: &Path, version: &str, trial: u64) -> Result<()> {
    for case in ["canon-8", "canon-16", "panasonic-8", "panasonic-16"] {
        let base = folder.join(case);
        let doc = photocraft_format::load_from_bytes(&std::fs::read(base.with_extension("pcraft"))?)?;
        let image = photocraft_io::document_to_image(&doc, &mut Vec::new())?;
        let common = std::fs::read(base.with_extension("psd"))?;
        let expected = pixels(&doc)?;
        for operation in ["png-encode", "psd-export", "psd-import"] {
            let start = Instant::now();
            let (ms, bytes, hash) = if operation == "png-encode" {
                let encoded = black_box(photocraft_codecs::encode(black_box(&image), Format::Png, &EncodeOptions::default())?);
                let ms = start.elapsed().as_secs_f64() * 1000.0;
                let decoded = photocraft_codecs::decode(&encoded)?;
                if decoded != image {
                    return Err("PNG output changes image data or metadata".into());
                }
                (ms, encoded.len(), blake3::hash(decoded.data()).to_hex().to_string())
            } else {
                let (ms, bytes, imported) = if operation == "psd-export" {
                    let encoded = black_box(psd(black_box(&doc))?);
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    (ms, encoded.len(), photocraft_io::import("own.psd", &encoded)?.document)
                } else {
                    let imported = black_box(photocraft_io::import("common.psd", black_box(&common))?.document);
                    (start.elapsed().as_secs_f64() * 1000.0, common.len(), imported)
                };
                let hash = pixels(&imported)?;
                if hash != expected {
                    return Err("PSD output changes photo pixels".into());
                }
                (ms, bytes, hash)
            };
            println!(
                "{}",
                json!({"case":case,"version":version,"trial":trial,"operation":operation,"ms":ms,"bytes":bytes,"decoded_hash":hash,"common_psd_hash":blake3::hash(&common).to_hex().to_string()})
            );
        }
    }
    Ok(())
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

fn summarize(path: &Path) -> Result<()> {
    let rows: Vec<Value> = std::fs::read_to_string(path)?.lines().map(serde_json::from_str).collect::<std::result::Result<_, _>>()?;
    println!("Comparison against main d95fdf31. Elapsed wall time, not CPU time. One warmup and nine alternating paired samples.\n");
    println!("| Case | Operation | Main ms | zlib-rs ms | Change | Paired range | Main / zlib-rs bytes |\n|---|---|---:|---:|---:|---:|---:|");
    for case in ["canon-8", "canon-16", "panasonic-8", "panasonic-16"] {
        for op in ["png-encode", "psd-export", "psd-import"] {
            let select = |version: &str| {
                rows.iter().filter(|r| r["case"] == case && r["operation"] == op && r["version"] == version && r["trial"] != 0).collect::<Vec<_>>()
            };
            let a = select("baseline");
            let b = select("candidate");
            if a.len() != 9 || b.len() != 9 {
                return Err("expected nine samples per version".into());
            }
            let mut changes = Vec::new();
            let mut at = Vec::new();
            let mut bt = Vec::new();
            for (a, b) in a.iter().zip(&b) {
                if a["decoded_hash"] != b["decoded_hash"] || a["common_psd_hash"] != b["common_psd_hash"] || a["trial"] != b["trial"] {
                    return Err("mismatched decoded pixels, fixture or trial".into());
                }
                let ams = a["ms"].as_f64().ok_or("invalid time")?;
                let bms = b["ms"].as_f64().ok_or("invalid time")?;
                at.push(ams);
                bt.push(bms);
                changes.push((bms / ams - 1.0) * 100.0);
            }
            changes.sort_by(f64::total_cmp);
            let am = median(&mut at);
            let bm = median(&mut bt);
            println!(
                "| {case} | {op} | {am:.3} | {bm:.3} | {:+.2}% | {:+.2}..{:+.2}% | {} / {} |",
                (bm / am - 1.0) * 100.0,
                changes[0],
                changes[8],
                a[0]["bytes"],
                b[0]["bytes"]
            );
        }
    }
    println!(
        "\nRanges are observed paired changes, not confidence intervals. Hosted-runner timing is variable. Size, import-speed and dependency tradeoffs remain maintainer decisions.\n"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode, photos, output] if mode == "prepare" => prepare(Path::new(photos), Path::new(output)),
        [mode, folder, version, trial] if mode == "measure" => measure(Path::new(folder), version, trial.parse()?),
        [mode, path] if mode == "summarize" => summarize(Path::new(path)),
        _ => Err("use prepare PHOTOS OUTPUT | measure FIXTURES VERSION TRIAL | summarize SAMPLES".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_codecs::{ChannelLayout, Image, SampleType as CodecSample};
    #[test]
    fn pixel_signature_covers_geometry_and_depth() -> Result<()> {
        let a = Document::new("a", Size::new(1, 2), ColorMode::Rgb, SampleType::U8);
        let b = Document::new("b", Size::new(2, 1), ColorMode::Rgb, SampleType::U8);
        let c = Document::new("c", Size::new(1, 2), ColorMode::Rgb, SampleType::U16);
        assert_ne!(pixels(&a)?, pixels(&b)?);
        assert_ne!(pixels(&a)?, pixels(&c)?);
        Ok(())
    }
    #[test]
    fn png_and_psd_preserve_both_sample_depths() -> Result<()> {
        for sample in [CodecSample::U8, CodecSample::U16] {
            let image = Image::from_normalized(2, 1, ChannelLayout::Rgba, sample, &[0.2, 0.4, 0.8, 1.0, 0.9, 0.1, 0.3, 1.0])?
                .with_icc(Some(photocraft_cms::Builtin::ProPhotoCompat.profile().to_bytes().to_vec()));
            let encoded = photocraft_codecs::encode(&image, Format::Png, &EncodeOptions::default())?;
            let doc = photocraft_io::import("photo.png", &encoded)?.document;
            assert_eq!(photocraft_codecs::decode(&encoded)?, image);
            assert_eq!(pixels(&photocraft_io::import("photo.psd", &psd(&doc)?)?.document)?, pixels(&doc)?);
        }
        Ok(())
    }
}
