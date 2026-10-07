import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';

test('HTTP loading, owned pixels, and independent recoverable workers', async ({ page }) => {
    const files = {
        '/pkg/photocraft_psd.js': '../pkg/photocraft_psd.js',
        '/pkg/photocraft_psd_bg.wasm': '../pkg/photocraft_psd_bg.wasm',
        '/worker/worker.js': '../target/worker/worker.js',
        '/fixture.psd': '../target/fixture.psd',
    };
    const server = createServer(async (request, response) => {
        if (request.url === '/') return response.setHeader('Content-Type', 'text/html').end('<!doctype html><title>PSD bindings test</title>');
        const file = files[request.url];
        if (!file) return response.writeHead(404).end();
        try {
            const bytes = await readFile(new URL(file, import.meta.url));
            const type = file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.js') ? 'application/javascript' : 'application/octet-stream';
            response.setHeader('Content-Type', type).end(bytes);
        } catch { response.writeHead(500).end(); }
    });
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
    try {
        await page.goto(`http://127.0.0.1:${server.address().port}/`);
        const result = await page.evaluate(async () => {
            const { default: init, Psd, PsdBuilder } = await import('/pkg/photocraft_psd.js');
            const wasm = await init();
            const input = new Uint8Array(await (await fetch('/fixture.psd')).arrayBuffer());
            const original = input.slice();
            const doc = new Psd(input);
            input.fill(0);
            const preview = doc.preview(1);
            const rgba = preview.data, native = doc.decodeMerged(), saved = doc.toBytes();
            const image = doc.layerPixelsF32(0), floats = image.data;
            const edited = doc.editLayer(0, 'Renamed', false);
            const renamed = edited.layerName(0), available = edited.headerInfo().hasRealMergedData;
            image.free(); edited.free();
            preview.free();
            doc.free();
            wasm.memory.grow(1);
            const retained = { rgba: [...rgba], native: [...native], float: floats[0], renamed, available,
                roundtrip: saved.every((value, i) => value === original[i]) && saved.length === original.length };
            const builder = new PsdBuilder(1, 1, 8, undefined, undefined, 3);
            builder.composite(new Uint8Array([0, 0, 255, 255]));
            const blue = builder.build();
            const second = blue.toBytes();
            blue.free();
            builder.free();
            const workers = [new Worker('/worker/worker.js', { type: 'module' }), new Worker('/worker/worker.js', { type: 'module' })];
            const request = (worker, data) => new Promise((resolve, reject) => {
                const timeout = setTimeout(() => reject(new Error('worker response timeout')), 10_000);
                worker.onmessage = ({ data }) => { clearTimeout(timeout); resolve(data); };
                worker.onerror = event => { clearTimeout(timeout); reject(new Error(event.message)); };
                const bytes = data.slice().buffer;
                worker.postMessage({ bytes }, [bytes]);
                if (bytes.byteLength !== 0) reject(new Error('input buffer was not transferred'));
            });
            try {
                const invalid = await request(workers[0], new Uint8Array([0]));
                const responses = await Promise.all([request(workers[0], original), request(workers[1], second)]);
                return { retained, rejected: typeof invalid.error === 'string' && invalid.error.length > 0,
                    responses: responses.map(value => ({ width: value.width, height: value.height, rgba: [...value.rgba] })) };
            } finally { workers.forEach(worker => worker.terminate()); }
        });
        expect(result).toEqual({
            retained: { rgba: [255, 0, 0, 192], native: [...Buffer.from('ffffffff00007fff00007fffffff8000', 'hex')],
                float: 1, renamed: 'Renamed', available: false, roundtrip: true },
            rejected: true,
            responses: [
                { width: 2, height: 1, rgba: [255, 0, 0, 255, 255, 0, 0, 128] },
                { width: 1, height: 1, rgba: [0, 0, 255, 255] },
            ],
        });
    } finally { await new Promise(resolve => server.close(resolve)); }
});
