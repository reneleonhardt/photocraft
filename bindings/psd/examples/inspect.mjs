// Node consumer of the generated CommonJS wasm-bindgen package.
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { Psd } = require('../pkg-node/photocraft_psd.js');
if (process.argv.length !== 4) throw new Error('usage: inspect.mjs <input.psd> <output.psd>');
const doc = new Psd(readFileSync(process.argv[2]), true);
try {
    console.log(doc.headerInfo(), doc.resolution(), doc.layerTree());
    console.log(`merged: ${doc.decodeMerged().length} bytes; ICC: ${doc.iccProfile().length} bytes`);
    console.log(doc.thumbnailInfo(), Array.from({ length: doc.resourceCount() }, (_, i) => doc.resourceInfo(i)));
    const slices = doc.slices();
    if (slices) {
        try { console.log(slices.info(), slices.groupName()); }
        finally { slices.free(); }
    }
    for (let i = 0; i < doc.layerCount; ++i) {
        console.log(doc.layerMetadata(i));
        const vector = doc.vectorMask(i);
        if (vector) {
            try { console.log('vector mask:', vector.info()); }
            finally { vector.free(); }
        }
        for (let j = 0; j < doc.layerChannelCount(i); ++j) {
            const id = doc.channelId(i, j);
            console.log(`  channel ${id}: ${doc.decodeChannel(i, id).length} bytes ${doc.channelBounds(i, id)}`);
        }
    }
    writeFileSync(process.argv[3], doc.toBytes());
} finally {
    doc.free();
}
