import assert from 'node:assert/strict';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { Psd, PsdBuilder } = require('../pkg-node/photocraft_psd.js');
const doc = new Psd(readFileSync(process.argv[2]));
try {
    assert.deepEqual([doc.width, doc.height, doc.version, doc.channels, doc.depth, doc.colorMode, doc.layerCount], [2, 1, 1, 4, 16, 3, 1]);
    assert.equal(doc.layerName(0), 'Red ☺');
    assert.deepEqual([...doc.layerBounds(0)], [2, -1, 3, 1]);
    assert.equal(doc.layerChannelCount(0), 4);
    assert.deepEqual([0, 1, 2, 3].map(i => doc.channelId(0, i)), [-1, 0, 1, 2]);
    assert.deepEqual([...doc.channelBounds(0, 0)], [2, -1, 3, 1]);
    assert.deepEqual(Buffer.from(doc.decodeChannel(0, 0)), Buffer.from('ffffffff', 'hex'));
    assert.deepEqual(Buffer.from(doc.decodeMerged()), Buffer.from('ffffffff00007fff00007fffffff8000', 'hex'));
    assert.equal(Buffer.from(doc.iccProfile()).subarray(36, 40).toString(), 'acsp');
    assert.equal(doc.headerInfo().compression, 1);
    assert.equal(doc.headerInfo().hasRealMergedData, true);
    assert.equal(doc.layerMetadata(0).mask, null);
    assert.equal(doc.resolution().horizontal, 300);
    assert.equal(Buffer.from(doc.resource(1060)).toString(), '<xmp/>');
    assert.equal(doc.resource(65535), undefined);
    assert.equal(doc.colorModeData().length, 0);
    assert.deepEqual(Buffer.from(doc.mergedRgba8()), Buffer.from('ff0000ffff000080', 'hex'));
    const hidden = doc.editLayer(0, 'Black', false), black = hidden.replaceChannel(0, 0, new Uint8Array(4));
    try { writeFileSync(join(dirname(process.argv[2]), 'edited-javascript.psd'), black.toBytes()); }
    finally { hidden.free(); black.free(); }
    for (let i = 0; i < doc.resourceCount(); ++i) {
        const info = doc.resourceInfo(i);
        assert.equal(info.dataLen, doc.resourceData(i).length);
        assert.equal(info.nameLen, doc.resourceName(i).length);
        if (info.id === 1060) assert.equal(Buffer.from(doc.resourceName(i)).toString(), 'XMP');
    }
    for (let i = 0; i < doc.layerBlockCount(0); ++i) assert.equal(doc.layerBlockInfo(0, i).dataLen, doc.layerBlockData(0, i).length);
    assert.equal(doc.globalBlockCount(), 1);
    assert.deepEqual(doc.globalBlockInfo(0).key, [...Buffer.from('TEST')]);
    assert.equal(Buffer.from(doc.globalBlockData(0)).toString(), 'global');
    assert.deepEqual(doc.thumbnailInfo(), { resourceId: 1036, format: 0, width: 2, height: 1, rowBytes: 8, bgr: false });
    assert.deepEqual(Buffer.from(doc.thumbnailData()), Buffer.from('ff0000ff00000000', 'hex'));
    const preview = doc.preview(1);
    const data = preview.data;
    assert.deepEqual([preview.width, preview.height, ...data], [1, 1, 255, 0, 0, 192]);
    preview.free();
    assert.deepEqual([...data], [255, 0, 0, 192]);
    const layerPreview = doc.layerPreview(0, 1);
    try { assert.deepEqual([layerPreview.width, layerPreview.height, ...layerPreview.data], [1, 1, 255, 0, 0, 192]); }
    finally { layerPreview.free(); }
    const pathIndex = Array.from({ length: doc.resourceCount() }, (_, i) => i).find(i => doc.resourceInfo(i).id === 2000);
    const owner = new Psd(doc.toBytes());
    const outline = owner.resourcePath(pathIndex), vector = owner.vectorMask(0), slices = owner.slices();
    owner.free();
    try {
        assert.deepEqual(outline.info(), { subpathCount: 1, version: 0, flags: 0, initialFill: false, isVectorMask: false });
        assert.deepEqual(outline.subpathInfo(0), { knotCount: 3, operation: 1, closed: true });
        assert.deepEqual(outline.knot(0, 1), { pre: [1, 0], anchor: [1, 0], post: [1, 0], linked: true });
        assert.equal(vector.info().flags, 4);
        assert.deepEqual(slices.info(), { bounds: [0, 0, 1, 2], count: 1, version: 6 });
        assert.equal(slices.groupName(), 'assets');
        assert.equal(slices.text(0, 'name'), 'red');
        assert.equal(slices.text(0, 'alt'), 'Red');
        assert.deepEqual(slices.sliceInfo(0).bounds, [0, 0, 2, 1]);
        assert.equal(slices.sliceInfo(0).hasLayerId, true);
        assert.equal(doc.resourcePath(0), undefined);
        for (const call of [() => outline.subpathInfo(99), () => outline.knot(0, 99), () => slices.sliceInfo(99), () => slices.text(0, 'invalid')]) assert.throws(call, Error);
    } finally { outline.free(); vector.free(); slices.free(); }
    const builder = new PsdBuilder(1, 1, 16, undefined, undefined, 3);
    try {
        const pixels = Buffer.from('ffff000000008000', 'hex');
        builder.beginGroup('Folder');
        builder.addLayer('Red', new Int32Array([0, 0, 1, 1]), pixels,
            { blendMode: [110, 111, 114, 109], opacity: 127, fillOpacity: 128, visible: false, clipping: true,
              maskBounds: [0, 0, 1, 1], maskDefault: 255, maskDisabled: true }, new Uint8Array([128]));
        builder.endGroup();
        builder.composite(pixels);
        builder.resolution(300);
        builder.iccProfile(doc.iccProfile());
        builder.resource(1060, Buffer.from('<xmp/>'), Buffer.from('XMP'));
        const built = builder.build();
        const created = new Psd(built.toBytes());
        built.free();
        try {
            assert.deepEqual([...created.decodeChannel(1, 0)], [255, 255]);
            assert.deepEqual([...created.decodeChannel(1, -2)], [128, 128]);
            assert.equal(created.layerMetadata(1).fillOpacity, 128);
            assert.equal(created.resolution().horizontal, 300);
            assert.equal(Buffer.from(created.resource(1060)).toString(), '<xmp/>');
            assert.deepEqual([...created.mergedRgba8()], [255, 0, 0, 128]);
            assert.equal(created.slices(), undefined);
            assert.equal(created.vectorMask(1), undefined);
        } finally { created.free(); }
        assert.throws(() => builder.addLayer('', new Int32Array(3), pixels), Error);
        assert.throws(() => builder.addLayer('', new Int32Array([0, 0, 1, 1]), pixels, { opacity: 256 }), Error);
        assert.throws(() => builder.addLayer('', new Int32Array([0, 0, 1, 1]), pixels, { opacitty: 128 }), Error);
    } finally { builder.free(); }
    assert.throws(() => new PsdBuilder(0, 1), Error);
    const fixtures = join(dirname(process.argv[2]), 'fixtures');
    const files = readdirSync(fixtures).filter(name => name.endsWith('.psd')).sort();
    assert.equal(files.length, 24, 'generate the full codec/depth/version matrix first');
    for (const file of files) {
        const [version, depth, compression] = file.slice(0, -4).split('-').map(part => Number(part.slice(1)));
        const raw = readFileSync(join(fixtures, file));
        const matrix = new Psd(raw);
        const lazy = new Psd(raw, true);
        try { assert.deepEqual(lazy.decodeMerged(), matrix.decodeMerged()); } finally { lazy.free(); }
        if (compression === 2 || compression === 3) {
            const corrupt = Buffer.from(raw);
            corrupt[corrupt.length - 1] ^= 1;
            assert.throws(() => new Psd(corrupt), Error);
            const deferred = new Psd(corrupt, true);
            try { assert.throws(() => deferred.decodeMerged(), Error); } finally { deferred.free(); }
        }
        try {
            assert.deepEqual([matrix.version, matrix.depth, matrix.headerInfo().compression], [version, depth, compression]);
            assert.deepEqual(Buffer.from(matrix.toBytes()), raw);
            const layer = matrix.layerMetadata(1);
            assert.deepEqual([layer.name, layer.opacity, layer.fillOpacity, layer.clipping, layer.visible, layer.blendMode], ['Red ☺', 127, 128, 1, false, 'norm']);
            assert.deepEqual(layer.blendModeKey, [110, 111, 114, 109]);
            assert.notEqual(layer.id, null);
            assert.ok(layer.mask.flags & 2);
            assert.deepEqual(layer.mask.bounds, { top: 2, left: 0, bottom: 3, right: 1 });
            assert.deepEqual(matrix.layerTree(), [{ index: 2, parent: null, divider: 0, isGroup: true }, { index: 1, parent: 2, divider: null, isGroup: false }]);
            for (const id of [-2, -1, 0, 1, 2]) assert.equal(matrix.channelCompression(1, id), compression);
            const red = { 8: 'ffff', 16: 'ffffffff', 32: '3f8000003f800000' }[depth];
            assert.deepEqual(Buffer.from(matrix.decodeChannel(1, 0)), Buffer.from(red, 'hex'));
            const merged = { 8: 'ffff007f007fff80', 16: 'ffffffff00007fff00007fffffff8000',
                32: '3f8000003f800000000000003f000000000000003f0000003f8000003f000000' }[depth];
            assert.deepEqual(Buffer.from(matrix.decodeMerged()), Buffer.from(merged, 'hex'));
            assert.deepEqual(Buffer.from(matrix.layerRgba8(1)), Buffer.from('ff0000ffff000080', 'hex'));
            const image = matrix.layerPixelsF32(1), mergedImage = matrix.mergedPixelsF32();
            const floats = image.data;
            assert.ok(floats instanceof Float32Array);
            assert.deepEqual([image.width, image.height, image.colorMode, image.channels], [2, 1, 3, 4]);
            assert.deepEqual([...floats.slice(0, 7)], [1, 0, 0, 1, 1, 0, 0]);
            assert.ok(Math.abs(floats[7] - 0.5) < 0.002);
            assert.equal(mergedImage.data[0], 1);
            image.free(); mergedImage.free();
            assert.equal(floats[0], 1);
            const renamed = matrix.editLayer(1, 'Ink 🎨');
            const changed = renamed.replaceChannel(1, 0, Buffer.from(depth === 32 ? '4000000040000000' : red, 'hex'));
            const shown = changed.editLayer(1, undefined, true);
            const reopened = new Psd(shown.toBytes());
            try {
                assert.equal(reopened.layerName(1), 'Ink 🎨');
                assert.equal(reopened.layerMetadata(1).visible, true);
                assert.equal(reopened.headerInfo().hasRealMergedData, false);
                assert.equal(reopened.thumbnailInfo(), null);
                assert.throws(() => reopened.mergedPixelsF32(), Error);
                const hdr = reopened.layerPixelsF32(1);
                try { assert.equal(hdr.data[0], depth === 32 ? 2 : 1); } finally { hdr.free(); }
                assert.equal(matrix.layerName(1), 'Red ☺');
            } finally { renamed.free(); changed.free(); shown.free(); reopened.free(); }
        } finally { matrix.free(); }
    }
    for (const call of [() => doc.editLayer(999), () => doc.replaceChannel(0, 123, new Uint8Array()),
        () => doc.replaceChannel(0, 0, new Uint8Array()), () => doc.layerPixelsF32(999),
        () => new Psd(new Uint8Array()), () => doc.layerName(999), () => doc.layerBounds(999),
        () => doc.layerChannelCount(999), () => doc.channelId(0, 999), () => doc.channelBounds(0, 99), () => doc.decodeChannel(0, 99),
        () => doc.layerMetadata(999), () => doc.layerRgba8(999), () => doc.channelCompression(0, 99), () => doc.resource(65536),
        () => doc.resourceInfo(999), () => doc.resourceName(999), () => doc.resourceData(999),
        () => doc.layerBlockCount(999), () => doc.layerBlockInfo(0, 999), () => doc.layerBlockData(0, 999),
        () => doc.resourcePath(999), () => doc.vectorMask(999), () => doc.globalBlockInfo(999), () => doc.globalBlockData(999), () => doc.preview(0), () => doc.layerPreview(0, 0)]) {
        assert.throws(call, Error);
    }
    for (const bad of [-1, 0.5, NaN, Infinity, 2 ** 32]) {
        assert.throws(() => doc.layerName(bad), Error);
        assert.throws(() => doc.resource(bad), Error);
        assert.throws(() => new PsdBuilder(bad, 1), Error);
        assert.throws(() => doc.preview(bad), Error);
        assert.throws(() => doc.layerPreview(bad, 1), Error);
        assert.throws(() => doc.layerPreview(0, bad), Error);
        assert.throws(() => doc.channelId(0, bad), Error);
    }
    for (const bad of [0.5, NaN, Infinity, -32769, 65536]) {
        assert.throws(() => doc.channelBounds(0, bad), Error);
        assert.throws(() => doc.decodeChannel(0, bad), Error);
    }
} finally {
    doc.free();
}
console.log('JavaScript consumer checks passed');
