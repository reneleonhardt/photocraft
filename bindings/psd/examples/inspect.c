/* Inspect native samples and serialize a PSD/PSB. */
#include "photocraft_psd.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

static uint8_t *read_file(const char *path, size_t *len) {
    FILE *file = fopen(path, "rb");
    if (!file) { perror(path); return NULL; }
    long size = -1;
    if (fseek(file, 0, SEEK_END) == 0) size = ftell(file);
    if (size < 0 || (uintmax_t)size > SIZE_MAX || fseek(file, 0, SEEK_SET) != 0) {
        fprintf(stderr, "cannot determine input size\n"); fclose(file); return NULL;
    }
    *len = (size_t)size;
    uint8_t *data = malloc(*len ? *len : 1);
    if (!data || fread(data, 1, *len, file) != *len) {
        fprintf(stderr, "cannot read input\n"); free(data); fclose(file); return NULL;
    }
    if (fclose(file) != 0) { perror(path); free(data); return NULL; }
    return data;
}

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: inspect <input.psd> <output.psd>\n"); return 1; }
    size_t len = 0;
    uint8_t *input = read_file(argv[1], &len);
    if (!input) return 1;
    pc_psd *doc = NULL;
    pc_psd_path *vector = NULL;
    pc_psd_slice_collection *slices = NULL;
    pc_psd_buffer out = {0};
    pc_psd_float_image pixels = {0};
    pc_psd_error error = {{0}};
    pc_psd_header_info header;
    int result = 1;
#define CHECK(call) do { if ((call) != PC_PSD_OK) { fprintf(stderr, "%s\n", error.message); goto cleanup; } } while (0)
    if (pc_psd_abi_version() != 1) { fprintf(stderr, "unsupported ABI\n"); goto cleanup; }
    CHECK(pc_psd_parse_lazy(input, len, &doc, &error));
    CHECK(pc_psd_header(doc, &header, &error));
    pc_psd_document info;
    pc_psd_resolution_info resolution;
    CHECK(pc_psd_document_info(doc, &info, &error));
    CHECK(pc_psd_resolution(doc, &resolution, &error));
    printf("compression %u, merged alpha %u, resolution present %u\n", info.compression, info.merged_has_alpha, resolution.present);
    printf("%" PRIu32 "x%" PRIu32 ", depth %u, mode %u, version %u, channels %u, %zu layers\n",
        header.width, header.height, header.depth, header.color_mode, header.version, header.channels, header.layer_count);
    CHECK(pc_psd_decode_merged(doc, &out, &error));
    printf("merged: %zu bytes\n", out.len);
    pc_psd_buffer_free(&out);
    if (header.color_mode == PC_PSD_BITMAP || header.color_mode == PC_PSD_GRAYSCALE || header.color_mode == PC_PSD_INDEXED ||
        header.color_mode == PC_PSD_RGB || header.color_mode == PC_PSD_CMYK || header.color_mode == PC_PSD_LAB) {
        CHECK(pc_psd_merged_pixels_f32(doc, &pixels, &error));
        printf("source pixels: %zu floats, %u channels, mode %u\n", pixels.data.len, pixels.channels, pixels.color_mode);
        pc_psd_float_buffer_free(&pixels.data);
    }
    CHECK(pc_psd_icc_profile(doc, &out, &error));
    printf("ICC: %zu bytes\n", out.len);
    pc_psd_buffer_free(&out);
    pc_psd_thumbnail thumbnail;
    CHECK(pc_psd_thumbnail_info(doc, &thumbnail, &error));
    printf("thumbnail present %u: %" PRIu32 "x%" PRIu32 " format %" PRIu32 "\n", thumbnail.present, thumbnail.width, thumbnail.height, thumbnail.format);
    size_t resources = 0;
    CHECK(pc_psd_resource_count(doc, &resources, &error));
    for (size_t i = 0; i < resources; ++i) {
        pc_psd_resource_info_value resource;
        CHECK(pc_psd_resource_info(doc, i, &resource, &error));
        printf("resource %u: %zu bytes\n", resource.id, resource.data_len);
    }
    CHECK(pc_psd_slices(doc, &slices, &error));
    if (slices) {
        pc_psd_slices_info_value slices_info;
        CHECK(pc_psd_slices_info(slices, &slices_info, &error));
        printf("slices: %zu\n", slices_info.count);
    }
    for (size_t i = 0; i < header.layer_count; ++i) {
        CHECK(pc_psd_vector_mask(doc, i, &vector, &error));
        if (vector) {
            pc_psd_path_info_value path;
            CHECK(pc_psd_path_info(vector, &path, &error));
            printf("vector mask: %zu subpaths, flags %" PRIu32 "\n", path.subpath_count, path.flags);
            pc_psd_path_free(vector);
            vector = NULL;
        }
        pc_psd_layer layer;
        CHECK(pc_psd_layer_info(doc, i, &layer, &error));
        pc_psd_layer_properties_info properties;
        CHECK(pc_psd_layer_properties(doc, i, &properties, &error));
        printf("visible %u, opacity %u, section %" PRIu32 "\n", properties.visible, properties.opacity, properties.section_type);
        CHECK(pc_psd_layer_name(doc, i, &out, &error));
        printf("layer %zu: ", i);
        if (out.len) fwrite(out.data, 1, out.len, stdout);
        printf(" (%" PRId32 ",%" PRId32 ",%" PRId32 ",%" PRId32 ")\n", layer.top, layer.left, layer.bottom, layer.right);
        pc_psd_buffer_free(&out);
        for (size_t j = 0; j < layer.channel_count; ++j) {
            int16_t id;
            int32_t bounds[4];
            CHECK(pc_psd_channel_id(doc, i, j, &id, &error));
            CHECK(pc_psd_channel_bounds(doc, i, id, bounds, &error));
            CHECK(pc_psd_decode_channel(doc, i, id, &out, &error));
            printf("  channel %d: %zu bytes (%" PRId32 ",%" PRId32 ",%" PRId32 ",%" PRId32 ")\n",
                id, out.len, bounds[0], bounds[1], bounds[2], bounds[3]);
            pc_psd_buffer_free(&out);
        }
    }
    CHECK(pc_psd_serialize(doc, &out, &error));
    FILE *file = fopen(argv[2], "wb");
    if (!file) { perror(argv[2]); goto cleanup; }
    int write_failed = fwrite(out.data, 1, out.len, file) != out.len;
    int close_failed = fclose(file) != 0;
    if (write_failed || close_failed) { fprintf(stderr, "cannot write output\n"); goto cleanup; }
    result = 0;
cleanup:
    pc_psd_float_buffer_free(&pixels.data);
    pc_psd_path_free(vector);
    pc_psd_slices_free(slices);
    pc_psd_buffer_free(&out);
    pc_psd_free(doc);
    free(input);
    return result;
}
