// Smoke-test the browser worker in Node with a DedicatedWorkerGlobalScope/file-fetch shim.
import assert from 'node:assert/strict';
import { cpSync, readFileSync } from 'node:fs';
import { Worker, isMainThread, parentPort } from 'node:worker_threads';

if (isMainThread) {
    cpSync(new URL('../pkg/', import.meta.url), new URL('../target/pkg/', import.meta.url), { recursive: true });
    const worker = new Worker(new URL(import.meta.url));
    try {
        await new Promise((resolve, reject) => { worker.once('message', resolve); worker.once('error', reject); });
        const source = readFileSync(process.argv[2]);
        const request = async () => {
            const bytes = source.buffer.slice(source.byteOffset, source.byteOffset + source.byteLength);
            const result = new Promise((resolve, reject) => { worker.once('message', resolve); worker.once('error', reject); });
            worker.postMessage({ bytes }, [bytes]);
            assert.equal(bytes.byteLength, 0);
            return result;
        };
        assert.deepEqual(await request(), { error: 'temporary WASM load failure' });
        const response = await request();
        assert.equal(response.header.width, 2);
        assert.deepEqual([response.width, response.height], [2, 1]);
        assert.deepEqual(Buffer.from(response.rgba), Buffer.from('ff0000ffff000080', 'hex'));
        console.log('Worker load retry/transfer/preview check passed');
    } finally { await worker.terminate(); }
} else {
    globalThis.self = { postMessage: (value, transfers) => parentPort.postMessage(value, transfers) };
    let failFetch = true;
    globalThis.fetch = async url => {
        if (failFetch) { failFetch = false; throw new Error('temporary WASM load failure'); }
        return new Response(readFileSync(url), { headers: { 'Content-Type': 'application/wasm' } });
    };
    await import('../target/worker/worker.js');
    parentPort.on('message', data => self.onmessage({ data }));
    parentPort.postMessage('ready');
}
