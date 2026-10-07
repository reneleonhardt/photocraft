// Exercise the browser ES-module loader with explicit WASM bytes in Node.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import init, { Psd, PsdBuilder } from '../pkg/photocraft_psd.js';

await init({ module_or_path: readFileSync(new URL('../pkg/photocraft_psd_bg.wasm', import.meta.url)) });
const input = readFileSync(process.argv[2]);
const doc = new Psd(input);
try {
    assert.equal(doc.headerInfo().width, 2);
    assert.equal(doc.layerMetadata(0).name, 'Red ☺');
    assert.deepEqual(Buffer.from(doc.mergedRgba8()), Buffer.from('ff0000ffff000080', 'hex'));
    assert.deepEqual(Buffer.from(doc.toBytes()), input);
} finally { doc.free(); }
const builder = new PsdBuilder(1, 1);
builder.addLayer('Red', new Int32Array([0, 0, 1, 1]), new Uint8Array([255, 0, 0, 128]), {});
const created = builder.build();
builder.free();
assert.equal(created.layerMetadata(0).name, 'Red');
created.free();
console.log('Web ES-module consumer check passed');
