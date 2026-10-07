// Create a layered PSD with a saved composite.
import { writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { PsdBuilder } = createRequire(import.meta.url)('../pkg-node/photocraft_psd.js');
if (process.argv.length !== 3) throw new Error('usage: create.mjs <output.psd>');
const builder = new PsdBuilder(1, 1);
try {
    const pixels = new Uint8Array([255, 0, 0, 128]);
    builder.addLayer('Red', new Int32Array([0, 0, 1, 1]), pixels);
    builder.composite(pixels);
    builder.resolution(300);
    const doc = builder.build();
    try { writeFileSync(process.argv[2], doc.toBytes()); }
    finally { doc.free(); }
} finally { builder.free(); }
