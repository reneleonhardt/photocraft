#include "photocraft_psd.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    assert(argc == 2);
    FILE *file = fopen(argv[1], "rb");
    assert(file && fseek(file, 0, SEEK_END) == 0);
    long size = ftell(file);
    assert(size > 0 && fseek(file, 0, SEEK_SET) == 0);
    uint8_t *input = malloc((size_t)size);
    assert(input && fread(input, 1, (size_t)size, file) == (size_t)size);
    assert(fclose(file) == 0);
    pc_psd *doc = NULL;
    pc_psd_buffer out = {0};
    pc_psd_error error = {{0}};
    pc_psd_header_info h;
    pc_psd_document info;
    pc_psd_layer_properties_info properties;
    pc_psd_mask mask;
    pc_psd_resolution_info resolution;
    pc_psd_tree tree = {0};
    pc_psd_float_image pixels = {0};
#define CHECK(call) assert((call) == PC_PSD_OK)
    CHECK(pc_psd_parse(input, (size_t)size, &doc, &error));
    pc_psd *lazy = NULL;
    CHECK(pc_psd_parse_lazy(input, (size_t)size, &lazy, &error));
    CHECK(pc_psd_decode_merged(lazy, &out, &error));
    pc_psd_buffer_free(&out);
    CHECK(pc_psd_merged_pixels_f32(lazy, &pixels, &error));
    pc_psd_float_buffer_free(&pixels.data);
    pc_psd_free(lazy);
    CHECK(pc_psd_header(doc, &h, &error));
    pc_psd *edited = NULL;
    assert(pc_psd_edit_layer(doc, 0, (pc_psd_bytes){NULL, 0}, 2, &edited, &error) == PC_PSD_INVALID_ARGUMENT && edited == NULL);
    assert(pc_psd_edit_layer(doc, SIZE_MAX, (pc_psd_bytes){NULL, 0}, -1, &edited, &error) == PC_PSD_ERROR && edited == NULL);
    assert(pc_psd_replace_channel(doc, 0, INT16_MAX, (pc_psd_bytes){NULL, 0}, &edited, &error) == PC_PSD_ERROR && edited == NULL);
    CHECK(pc_psd_document_info(doc, &info, &error));
    if (info.compression == 2 || info.compression == 3) {
        input[size - 1] ^= 1;
        assert(pc_psd_parse(input, (size_t)size, &lazy, &error) == PC_PSD_ERROR && lazy == NULL);
        CHECK(pc_psd_parse_lazy(input, (size_t)size, &lazy, &error));
        assert(pc_psd_decode_merged(lazy, &out, &error) == PC_PSD_ERROR && out.data == NULL);
        assert(pc_psd_merged_pixels_f32(lazy, &pixels, &error) == PC_PSD_ERROR && pixels.data.data == NULL);
        pc_psd_free(lazy);
        input[size - 1] ^= 1;
    }
    assert(h.width == 2 && h.height == 1 && info.merged_has_alpha == 1 && info.has_real_merged_data == 1);
    size_t leaf = h.layer_count == 3 ? 1 : 0;
    CHECK(pc_psd_layer_properties(doc, leaf, &properties, &error));
    assert(memcmp(properties.blend_mode, "norm", 4) == 0 && properties.has_id);
    assert(properties.opacity == (leaf ? 127 : 255) && properties.fill_opacity == (leaf ? 128 : 255));
    assert(properties.visible == !leaf && properties.clipping == (uint8_t)leaf);
    CHECK(pc_psd_layer_mask(doc, leaf, &mask, &error));
    assert(mask.present == !!leaf);
    if (leaf) assert(mask.flags & 2 && mask.bounds[0] == 2 && mask.bounds[1] == 0 && mask.default_color == 255);
    CHECK(pc_psd_resolution(doc, &resolution, &error));
    assert(resolution.present && resolution.horizontal == 300 && resolution.vertical == 300 && resolution.horizontal_unit == 1);
    CHECK(pc_psd_resource(doc, 1060, &out, &error));
    assert(out.len == 6 && memcmp(out.data, "<xmp/>", 6) == 0);
    pc_psd_buffer_free(&out);
    CHECK(pc_psd_resource(doc, 65535, &out, &error));
    assert(out.data == NULL && out.len == 0);
    CHECK(pc_psd_color_mode_data(doc, &out, &error));
    assert(out.len == 0);
    int32_t compression = -1;
    CHECK(pc_psd_channel_compression(doc, leaf, 0, &compression, &error));
    assert(compression == info.compression);
    CHECK(pc_psd_decode_channel(doc, leaf, 0, &out, &error));
    const uint8_t f32[] = {0x3f, 0x80, 0, 0, 0x3f, 0x80, 0, 0};
    if (h.depth == 32) assert(out.len == 8 && memcmp(out.data, f32, 8) == 0);
    else { assert(out.len == (size_t)h.depth / 4); for (size_t i = 0; i < out.len; ++i) assert(out.data[i] == 255); }
    pc_psd_buffer_free(&out);
    const uint8_t rgba[] = {255, 0, 0, 255, 255, 0, 0, 128};
    CHECK(pc_psd_layer_pixels_f32(doc, leaf, &pixels, &error));
    assert(pixels.width == 2 && pixels.height == 1 && pixels.color_mode == PC_PSD_RGB && pixels.channels == 4 && pixels.data.len == 8);
    const float alpha = h.depth == 8 ? 128.0f / 255.0f : h.depth == 16 ? 32768.0f / 65535.0f : 0.5f;
    const float expected[] = {1, 0, 0, 1, 1, 0, 0, alpha};
    for (size_t i = 0; i < 8; ++i) assert(pixels.data.data[i] == expected[i]);
    pc_psd_float_buffer_free(&pixels.data);
    assert(pc_psd_layer_pixels_f32(doc, SIZE_MAX, &pixels, &error) == PC_PSD_ERROR && pixels.data.data == NULL);
    CHECK(pc_psd_merged_pixels_f32(doc, &pixels, &error));
    for (size_t i = 0; i < 8; ++i) {
        float difference = pixels.data.data[i] - expected[i];
        assert(difference > -0.000001f && difference < 0.000001f);
    }
    CHECK(pc_psd_merged_rgba8(doc, &out, &error));
    assert(out.len == 8 && memcmp(out.data, rgba, 8) == 0);
    pc_psd_buffer_free(&out);
    CHECK(pc_psd_layer_rgba8(doc, leaf, &out, &error));
    assert(out.len == 8 && memcmp(out.data, rgba, 8) == 0);
    pc_psd_buffer_free(&out);
    size_t count = 0;
    pc_psd_path *outline = NULL, *vector = NULL;
    CHECK(pc_psd_resource_count(doc, &count, &error));
    for (size_t i = 0; i < count; ++i) {
        pc_psd_resource_info_value resource;
        CHECK(pc_psd_resource_info(doc, i, &resource, &error));
        if (resource.id == 2000) CHECK(pc_psd_resource_path(doc, i, &outline, &error));
        CHECK(pc_psd_resource_name(doc, i, &out, &error));
        assert(out.len == resource.name_len);
        if (resource.id == 1060) assert(out.len == 3 && memcmp(out.data, "XMP", 3) == 0);
        pc_psd_buffer_free(&out);
        CHECK(pc_psd_resource_data(doc, i, &out, &error));
        assert(out.len == resource.data_len);
        pc_psd_buffer_free(&out);
    }
    CHECK(pc_psd_layer_block_count(doc, leaf, &count, &error));
    for (size_t i = 0; i < count; ++i) {
        pc_psd_block_info block;
        CHECK(pc_psd_layer_block_info(doc, leaf, i, &block, &error));
        CHECK(pc_psd_layer_block_data(doc, leaf, i, &out, &error));
        assert(out.len == block.data_len);
        pc_psd_buffer_free(&out);
    }
    pc_psd_block_info block;
    CHECK(pc_psd_global_block_count(doc, &count, &error));
    CHECK(pc_psd_global_block_info(doc, 0, &block, &error));
    CHECK(pc_psd_global_block_data(doc, 0, &out, &error));
    assert(count == 1 && memcmp(block.key, "TEST", 4) == 0 && out.len == 6 && memcmp(out.data, "global", 6) == 0);
    pc_psd_buffer_free(&out);
    pc_psd_thumbnail thumbnail;
    CHECK(pc_psd_thumbnail_info(doc, &thumbnail, &error));
    CHECK(pc_psd_thumbnail_data(doc, &out, &error));
    assert(thumbnail.present && !thumbnail.bgr && thumbnail.format == 0 && thumbnail.width == 2 && out.len == 8);
    pc_psd_buffer_free(&out);
    pc_psd_preview_image preview;
    CHECK(pc_psd_preview(doc, 1, &preview, &error));
    assert(preview.width == 1 && preview.height == 1 && preview.data.len == 4 && preview.data.data[3] == 192);
    pc_psd_preview_image layer_preview;
    CHECK(pc_psd_layer_preview(doc, leaf, 1, &layer_preview, &error));
    assert(layer_preview.width == 1 && layer_preview.height == 1 && layer_preview.data.len == 4);
    assert(memcmp(layer_preview.data.data, preview.data.data, 4) == 0);
    pc_psd_buffer_free(&layer_preview.data);
    assert(pc_psd_layer_preview(doc, leaf, 0, &layer_preview, &error) == PC_PSD_ERROR);
    assert(layer_preview.data.data == NULL && layer_preview.data.len == 0);
    CHECK(pc_psd_vector_mask(doc, leaf, &vector, &error));
    pc_psd_path_info_value path_info;
    pc_psd_subpath subpath;
    pc_psd_knot_info knot;
    CHECK(pc_psd_path_info(vector, &path_info, &error));
    assert(path_info.is_vector_mask && path_info.flags == 4);
    pc_psd_path_free(vector);
    pc_psd_slice_collection *slices = NULL;
    pc_psd_slices_info_value slices_info;
    pc_psd_slice slice;
    CHECK(pc_psd_slices(doc, &slices, &error));
    CHECK(pc_psd_slices_info(slices, &slices_info, &error));
    assert(slices_info.count == 1 && slices_info.bounds[2] == 1 && slices_info.bounds[3] == 2);
    CHECK(pc_psd_slices_group_name(slices, &out, &error));
    assert(out.len == 6 && memcmp(out.data, "assets", 6) == 0);
    pc_psd_buffer_free(&out);
    CHECK(pc_psd_slice_info(slices, 0, &slice, &error));
    assert(slice.id == 7 && slice.has_layer_id && slice.bounds[2] == 2 && slice.bounds[3] == 1);
    CHECK(pc_psd_slice_text(slices, 0, (pc_psd_bytes){(const uint8_t *)"alt", 3}, &out, &error));
    assert(out.len == 3 && memcmp(out.data, "Red", 3) == 0);
    pc_psd_buffer_free(&out);
    assert(pc_psd_slice_text(slices, 0, (pc_psd_bytes){NULL, 1}, &out, &error) == PC_PSD_INVALID_ARGUMENT);
    assert(out.data == NULL && out.len == 0);
    if (!leaf) {
        pc_psd_builder *builder = NULL;
        pc_psd_builder_config config = PC_PSD_BUILDER_CONFIG(1, 1);
        config.depth = 16; config.compression = PC_PSD_ZIP_PREDICTION;
        CHECK(pc_psd_builder_new(config, &builder, &error));
        const uint8_t pixels[] = {255, 255, 0, 0, 0, 0, 128, 0}, mask_data[] = {128};
        pc_psd_layer_input layer = {{(const uint8_t *)"Red", 3}, {0, 0, 1, 1},
            {pixels, sizeof pixels}, {mask_data, 1}, PC_PSD_LAYER_OPTIONS};
        layer.options.mask_bounds[2] = 1; layer.options.mask_bounds[3] = 1;
        layer.options.fill_opacity = 128;
        assert(pc_psd_builder_begin_group(builder, (pc_psd_bytes){(const uint8_t *)"\xff", 1}, &error) == PC_PSD_INVALID_ARGUMENT);
        CHECK(pc_psd_builder_begin_group(builder, (pc_psd_bytes){(const uint8_t *)"Folder", 6}, &error));
        assert(pc_psd_builder_add_layer(builder, NULL, &error) == PC_PSD_INVALID_ARGUMENT);
        CHECK(pc_psd_builder_add_layer(builder, &layer, &error));
        CHECK(pc_psd_builder_end_group(builder, &error));
        CHECK(pc_psd_builder_composite(builder, layer.pixels, &error));
        CHECK(pc_psd_builder_resolution(builder, 300, &error));
        CHECK(pc_psd_icc_profile(doc, &out, &error));
        CHECK(pc_psd_builder_icc_profile(builder, (pc_psd_bytes){out.data, out.len}, &error));
        pc_psd_buffer_free(&out);
        CHECK(pc_psd_builder_resource(builder, 1060, (pc_psd_bytes){NULL, 0}, (pc_psd_bytes){(const uint8_t *)"<xmp/>", 6}, &error));
        pc_psd *created = NULL;
        CHECK(pc_psd_builder_build(builder, &created, &error));
        pc_psd_builder_free(builder);
        CHECK(pc_psd_decode_channel(created, 1, -2, &out, &error));
        assert(out.len == 2 && out.data[0] == 128 && out.data[1] == 128);
        pc_psd_buffer_free(&out);
        CHECK(pc_psd_merged_rgba8(created, &out, &error));
        assert(out.len == 4 && memcmp(out.data, rgba, 3) == 0 && out.data[3] == 128);
        pc_psd_buffer_free(&out);
        pc_psd_free(created);
        assert(pc_psd_builder_end_group(NULL, &error) == PC_PSD_INVALID_ARGUMENT);
        config.width = 0;
        builder = NULL;
        assert(pc_psd_builder_new(config, &builder, &error) == PC_PSD_ERROR && builder == NULL);
        pc_psd_builder_free(NULL);
    }
    CHECK(pc_psd_layer_tree(doc, &tree, &error));
    assert(tree.len == (size_t)(leaf ? 2 : 1));
    assert(tree.data[0].parent == SIZE_MAX);
    if (leaf) assert(tree.data[0].index == 2 && tree.data[0].divider == 0 && tree.data[0].is_group && tree.data[1].parent == 2);
    CHECK(pc_psd_serialize(doc, &out, &error));
    pc_psd_free(doc);
    assert(pixels.data.data[0] == 1); /* Float output owns its storage independently. */
    pc_psd_float_buffer_free(&pixels.data);
    pc_psd_float_buffer_free(&pixels.data);
    pc_psd_float_buffer_free(NULL);
    assert(pixels.data.data == NULL && pixels.data.len == 0);
    CHECK(pc_psd_path_info(outline, &path_info, &error));
    assert(path_info.subpath_count == 1 && !path_info.is_vector_mask);
    CHECK(pc_psd_subpath_info(outline, 0, &subpath, &error));
    assert(subpath.knot_count == 3 && subpath.closed && subpath.operation == 1);
    CHECK(pc_psd_knot(outline, 0, 1, &knot, &error));
    assert(knot.anchor[0] == 1 && knot.anchor[1] == 0 && knot.linked);
    assert(pc_psd_knot(outline, 0, 99, &knot, &error) == PC_PSD_ERROR);
    pc_psd_path_free(outline);
    pc_psd_path_free(NULL);
    CHECK(pc_psd_slice_info(slices, 0, &slice, &error));
    assert(slice.id == 7);
    pc_psd_slices_free(slices);
    pc_psd_slices_free(NULL);
    assert(out.len == (size_t)size && memcmp(out.data, input, out.len) == 0);
    assert(preview.data.data[0] == 255);
    pc_psd_buffer_free(&preview.data);
    assert(tree.data[0].parent == SIZE_MAX); /* Tree owns its storage independently. */
    pc_psd_buffer_free(&out);
    pc_psd_buffer_free(&out);
    assert(out.data == NULL && out.len == 0);
    pc_psd_tree_free(&tree);
    pc_psd_tree_free(&tree);
    pc_psd_tree_free(NULL);
    assert(tree.data == NULL && tree.len == 0);
    free(input);
    return 0;
}
