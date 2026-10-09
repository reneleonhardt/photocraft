/* Create a layered PSD with a saved composite. */
#include "photocraft_psd.h"
#include <stdio.h>

int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: create <output.psd>\n"); return 1; }
    pc_psd_builder *builder = NULL;
    pc_psd *doc = NULL;
    pc_psd_buffer out = {0};
    pc_psd_error error = {{0}};
    const uint8_t pixels[] = {255, 0, 0, 128};
    pc_psd_builder_config config = PC_PSD_BUILDER_CONFIG(1, 1);
    pc_psd_layer_input layer = {{(const uint8_t *)"Red", 3}, {0, 0, 1, 1},
        {pixels, sizeof pixels}, {NULL, 0}, PC_PSD_LAYER_OPTIONS};
    int result = 1;
#define CHECK(call) do { if ((call) != PC_PSD_OK) { fprintf(stderr, "%s\n", error.message); goto cleanup; } } while (0)
    CHECK(pc_psd_builder_new(config, &builder, &error));
    CHECK(pc_psd_builder_add_layer(builder, &layer, &error));
    CHECK(pc_psd_builder_composite(builder, layer.pixels, &error));
    CHECK(pc_psd_builder_resolution(builder, 300, &error));
    CHECK(pc_psd_builder_build(builder, &doc, &error));
    CHECK(pc_psd_serialize(doc, &out, &error));
    FILE *file = fopen(argv[1], "wb");
    if (!file) { perror(argv[1]); goto cleanup; }
    int write_failed = fwrite(out.data, 1, out.len, file) != out.len;
    int close_failed = fclose(file) != 0;
    if (write_failed || close_failed) { fprintf(stderr, "cannot write output\n"); goto cleanup; }
    result = 0;
cleanup:
    pc_psd_buffer_free(&out);
    pc_psd_free(doc);
    pc_psd_builder_free(builder);
    return result;
}
