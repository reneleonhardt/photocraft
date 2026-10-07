import { Psd, PsdBuilder, type LayerOptions, type PathInfo, type SubpathInfo, type KnotInfo, type SlicesInfo, type SliceInfo, Compression, ColorMode, SectionType, type Bounds, type HeaderInfo, type LayerMetadata, type MaskInfo, type ResolutionInfo, type TreeNode, type ResourceInfo, type BlockInfo, type ThumbnailInfo, type Preview } from '../pkg/photocraft_psd.js';
import { Psd as NodePsd } from '../pkg-node/photocraft_psd.js';

// This is a compile-time contract test; runtime conversion is checked by javascript.mjs.
export function consumer(doc: Psd, node: NodePsd): void {
    const header: HeaderInfo = doc.headerInfo();
    const layer: LayerMetadata = doc.layerMetadata(0);
    const bounds: Bounds = layer.bounds;
    const mask: MaskInfo | null = layer.mask;
    const resolution: ResolutionInfo | null = doc.resolution();
    const tree: TreeNode[] = doc.layerTree();
    const resource: Uint8Array | undefined = doc.resource(1060);
    const pixels: Uint8Array = doc.mergedRgba8();
    const info: ResourceInfo = doc.resourceInfo(0);
    const block: BlockInfo = doc.globalBlockInfo(0);
    const thumbnail: ThumbnailInfo | null = doc.thumbnailInfo();
    const payload: Uint8Array | undefined = doc.thumbnailData();
    const preview: Preview = doc.preview(1024);
    const rgba: Uint8Array = preview.data;
    const floatImage = doc.layerPixelsF32(0);
    const floats: Float32Array = floatImage.data;
    const edited: Psd = doc.editLayer(0, 'Renamed', false);
    const replaced: Psd = edited.replaceChannel(0, 0, doc.decodeChannel(0, 0));
    console.log(floatImage.colorMode, floatImage.channels, floats);
    floatImage.free(); edited.free(); replaced.free();
    console.log(info, block, thumbnail, payload, preview.width, preview.height, rgba);
    preview.free();
    const path = doc.resourcePath(0);
    if (path) {
        const info: PathInfo = path.info(), subpath: SubpathInfo = path.subpathInfo(0), knot: KnotInfo = path.knot(0, 0);
        console.log(info, subpath, knot);
        path.free();
    }
    const slices = doc.slices();
    if (slices) {
        const info: SlicesInfo = slices.info(), slice: SliceInfo = slices.sliceInfo(0);
        console.log(info, slice, slices.groupName(), slices.text(0, 'name'));
        slices.free();
    }
    const builder = new PsdBuilder(1, 1);
    const options: LayerOptions = { opacity: 128 };
    builder.addLayer('Red', new Int32Array([0, 0, 1, 1]), new Uint8Array([255, 0, 0, 128]), options);
    // @ts-expect-error Layer options have typed keys.
    builder.addLayer('Red', new Int32Array(4), new Uint8Array(4), { opacitty: 128 });
    const created: Psd = builder.build();
    created.free(); builder.free();
    const compression: number | undefined = doc.channelCompression(0, 0);
    console.log(header.compression === Compression.ZipPrediction, header.colorMode === ColorMode.Rgb,
        layer.sectionType === SectionType.OpenFolder, bounds, mask, resolution, tree, resource, pixels, compression, node.headerInfo());
    // @ts-expect-error Header widths are numeric, never any.
    const width: string = header.width;
    // @ts-expect-error Metadata accepts a numeric index.
    doc.layerMetadata('0');
    // @ts-expect-error Missing masks must be checked.
    const density: number = layer.mask.userDensity;
    console.log(width, density);
}
