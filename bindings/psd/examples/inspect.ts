// Browser entry point. Run decoding in a worker for large documents.
import init, { Psd, ColorMode, type HeaderInfo, type LayerMetadata } from '../pkg/photocraft_psd.js';

export async function inspect(file: File): Promise<HeaderInfo> {
    await init();
    const doc = new Psd(new Uint8Array(await file.arrayBuffer()), true);
    try {
        const header: HeaderInfo = doc.headerInfo();
        console.log(header.colorMode === ColorMode.Rgb, doc.resolution());
        console.log(doc.thumbnailInfo());
        for (const node of doc.layerTree()) {
            const layer: LayerMetadata = doc.layerMetadata(node.index);
            console.log(layer.name, layer.bounds, layer.opacity, node.parent);
            if (!node.isGroup) {
                const thumbnail = doc.layerPreview(node.index, 256);
                try { console.log(thumbnail.width, thumbnail.height, thumbnail.data.byteLength); }
                finally { thumbnail.free(); }
            }
        }
        return header;
    } finally {
        doc.free();
    }
}
