// Serve worker/worker.js and the sibling pkg/ directory over HTTP.
import init, { Psd, type HeaderInfo, type Preview } from '../pkg/photocraft_psd.js';

export type PreviewRequest = { bytes: ArrayBuffer };
export type PreviewResponse = { header: HeaderInfo; width: number; height: number; rgba: Uint8Array } | { error: string };
const worker = self as unknown as DedicatedWorkerGlobalScope;
let ready: ReturnType<typeof init> | undefined;
worker.onmessage = async ({ data }: MessageEvent<PreviewRequest>) => {
    let doc: Psd | undefined;
    let preview: Preview | undefined;
    try {
        await (ready ??= init().catch(error => {
            ready = undefined;
            throw error;
        }));
        doc = new Psd(new Uint8Array(data.bytes), true);
        preview = doc.preview(1024);
        const response: PreviewResponse = { header: doc.headerInfo(), width: preview.width, height: preview.height, rgba: preview.data };
        worker.postMessage(response, [response.rgba.buffer as ArrayBuffer]);
    } catch (error) {
        worker.postMessage({ error: error instanceof Error ? error.message : String(error) } satisfies PreviewResponse);
    } finally {
        preview?.free();
        doc?.free();
    }
};
