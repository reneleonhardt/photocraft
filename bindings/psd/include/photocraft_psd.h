#ifndef PHOTOCRAFT_PSD_H
#define PHOTOCRAFT_PSD_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct pc_psd pc_psd;
typedef struct {
    uint32_t width, height;
    uint16_t version, channels, depth, color_mode;
    size_t layer_count;
} pc_psd_header_info;
typedef struct {
    int32_t top, left, bottom, right;
    size_t channel_count;
} pc_psd_layer;
typedef struct { uint8_t *data; size_t len; } pc_psd_buffer;
typedef struct { char message[256]; } pc_psd_error;

enum { PC_PSD_OK = 0, PC_PSD_INVALID_ARGUMENT = 1, PC_PSD_ERROR = 2, PC_PSD_PANIC = 3 };
enum { PC_PSD_RAW = 0, PC_PSD_RLE = 1, PC_PSD_ZIP = 2, PC_PSD_ZIP_PREDICTION = 3 };
enum { PC_PSD_BITMAP = 0, PC_PSD_GRAYSCALE = 1, PC_PSD_INDEXED = 2, PC_PSD_RGB = 3,
    PC_PSD_CMYK = 4, PC_PSD_MULTICHANNEL = 7, PC_PSD_DUOTONE = 8, PC_PSD_LAB = 9 };
enum { PC_PSD_OTHER = 0, PC_PSD_OPEN_FOLDER = 1, PC_PSD_CLOSED_FOLDER = 2, PC_PSD_DIVIDER = 3 };

typedef struct { uint16_t compression; uint8_t merged_has_alpha; int8_t has_real_merged_data; } pc_psd_document;
typedef struct {
    uint8_t blend_mode[4]; uint32_t section_type, id;
    uint8_t has_id, visible, opacity, fill_opacity, clipping;
} pc_psd_layer_properties_info;
typedef struct {
    int32_t bounds[4], real_bounds[4]; double user_feather, vector_feather;
    uint8_t present, flags, default_color, parameters, user_density, vector_density,
        has_real, real_flags, real_background;
} pc_psd_mask;
typedef struct {
    double horizontal, vertical; uint16_t horizontal_unit, vertical_unit, width_unit, height_unit;
    uint8_t present;
} pc_psd_resolution_info;
typedef struct { size_t index, parent, divider; uint8_t is_group; } pc_psd_tree_node;
typedef struct { pc_psd_tree_node *data; size_t len; } pc_psd_tree;

typedef struct { uint8_t signature[4]; uint16_t id; size_t name_len, data_len; } pc_psd_resource_info_value;
typedef struct { uint8_t signature[4], key[4]; size_t data_len; } pc_psd_block_info;
typedef struct {
    uint32_t format, width, height, row_bytes; uint16_t resource_id; uint8_t present, bgr;
} pc_psd_thumbnail;
typedef struct { uint32_t width, height; pc_psd_buffer data; } pc_psd_preview_image;
typedef struct { float *data; size_t len; } pc_psd_float_buffer;
typedef struct { uint32_t width, height; uint16_t color_mode, channels; pc_psd_float_buffer data; } pc_psd_float_image;

typedef struct pc_psd_builder pc_psd_builder;
typedef struct pc_psd_path pc_psd_path;
typedef struct pc_psd_slice_collection pc_psd_slice_collection;
typedef struct { const uint8_t *data; size_t len; } pc_psd_bytes;
typedef struct { uint32_t width, height; uint16_t color_mode, depth, version, compression; } pc_psd_builder_config;
#define PC_PSD_BUILDER_CONFIG(w, h) { (w), (h), PC_PSD_RGB, 8, 1, PC_PSD_RLE }
typedef struct {
    uint8_t blend_mode[4]; int32_t mask_bounds[4];
    uint8_t opacity, fill_opacity, visible, clipping, mask_default, mask_disabled;
} pc_psd_layer_options;
#define PC_PSD_LAYER_OPTIONS { {'n','o','r','m'}, {0,0,0,0}, 255, 255, 1, 0, 255, 0 }
typedef struct { pc_psd_bytes name; int32_t bounds[4]; pc_psd_bytes pixels, mask; pc_psd_layer_options options; } pc_psd_layer_input;
typedef struct { size_t subpath_count; uint32_t version, flags; uint8_t initial_fill, is_vector_mask; } pc_psd_path_info_value;
typedef struct { size_t knot_count; int16_t operation; uint8_t closed; } pc_psd_subpath;
typedef struct { double pre[2], anchor[2], post[2]; uint8_t linked; } pc_psd_knot_info;
typedef struct { int32_t bounds[4]; size_t count; uint32_t version; } pc_psd_slices_info_value;
typedef struct {
    uint32_t id, group_id, origin, layer_id, kind; int32_t bounds[4], outsets[4];
    uint32_t horizontal_align, vertical_align; uint8_t color[4], has_layer_id, cell_text_is_html;
} pc_psd_slice;

/* ABI v1 contract:
 * Handles must be live values returned by the matching parse/build/geometry call. Inputs point to len readable
 * bytes in one allocation (NULL is allowed only at len == 0). Outputs/error point
 * to aligned writable storage, disjoint from each other and input/document memory.
 * error may be NULL; its message is NUL-terminated UTF-8, truncated to fit.
 * Failure resets valid output storage.
 * Release owned outputs before reusing their storage. Input bytes can be released
 * after parse. Returned bytes are independent of the document; keep the buffer's
 * data/len pair unchanged and release it only with pc_psd_buffer_free.
 * Free each owned handle exactly once; do not free while another call is using it.
 * Read-only calls may run concurrently with separate output/error storage.
 * All free functions accept NULL. buffer_free and tree_free reset their storage.
 * Pointer checks cannot detect dangling pointers or insufficient storage.
 * Layer order is bottom-most first; bounds are top, left, bottom, right.
 * Samples retain the PSD colour mode/depth: planar, big-endian at 16/32 bits;
 * 1-bit samples are MSB-first with each row padded to a whole byte.
 * Native decode functions do not composite or colour-manage the stored data.
 * Transparent RGB composites retain their white matte; layer samples are straight.
 * Stored CMYK samples are inverted ink values. These bytes are not builder inputs.
 */
uint32_t pc_psd_abi_version(void);
int32_t pc_psd_parse(const uint8_t *data, size_t len, pc_psd **out, pc_psd_error *error);
/* Defers saved-composite validation until pixel decoding. */
int32_t pc_psd_parse_lazy(const uint8_t *data, size_t len, pc_psd **out, pc_psd_error *error);
void pc_psd_free(pc_psd *doc);
void pc_psd_buffer_free(pc_psd_buffer *buffer);
int32_t pc_psd_header(const pc_psd *doc, pc_psd_header_info *out, pc_psd_error *error);
int32_t pc_psd_layer_info(const pc_psd *doc, size_t layer, pc_psd_layer *out, pc_psd_error *error);
/* UTF-8 layer-name bytes have no NUL terminator. */
int32_t pc_psd_layer_name(const pc_psd *doc, size_t layer, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_channel_id(const pc_psd *doc, size_t layer, size_t channel, int16_t *out, pc_psd_error *error);
int32_t pc_psd_channel_bounds(const pc_psd *doc, size_t layer, int16_t id, int32_t out[4], pc_psd_error *error);
int32_t pc_psd_decode_channel(const pc_psd *doc, size_t layer, int16_t id, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_decode_merged(const pc_psd *doc, pc_psd_buffer *out, pc_psd_error *error);
/* Packed native-endian floats: source-space colour channels followed by straight alpha.
 * RGB/Gray colour values are normalized, retaining finite 32-bit HDR values without clipping.
 * CMYK is normalized ink amounts; Lab retains normalized PSD encoding (L/100,
 * (a+128)/255, (b+128)/255). Bitmap expands to Gray (0 black); indexed expands to RGB.
 * Supports RGB/Gray at 8/16/32 bits, CMYK/Lab at 8/16, bitmap at 1 and indexed at 8.
 * Removes RGB composite white matte; fully transparent composite RGB becomes zero.
 * Integer matte quantization is irreversible. ICC bytes describe the source; no conversion,
 * gamma change, tone mapping, opacity/fill opacity, masks, effects or blending is applied.
 * Extra composite channels are omitted; absent transparency becomes alpha 1.
 * Nonfinite colours or alpha outside 0..1 return errors. Missing real composites return errors.
 * Rows are width * channels floats; len counts floats, not bytes.
 * Output is capped at 1 GiB, excluding decoding memory. Duotone/multichannel are unsupported.
 * Outputs own their storage independently; release out->data with float_buffer_free.
 * Layer dimensions use its bounds; an empty layer has empty data.
 */
int32_t pc_psd_merged_pixels_f32(const pc_psd *doc, pc_psd_float_image *out, pc_psd_error *error);
int32_t pc_psd_layer_pixels_f32(const pc_psd *doc, size_t layer, pc_psd_float_image *out, pc_psd_error *error);
void pc_psd_float_buffer_free(pc_psd_float_buffer *buffer);
/* A missing ICC profile succeeds with { NULL, 0 }. */
int32_t pc_psd_icc_profile(const pc_psd *doc, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_serialize(const pc_psd *doc, pc_psd_buffer *out, pc_psd_error *error);
/* Edits return an independent document; release it with pc_psd_free.
 * NULL/0 name and visible == -1 keep those fields; visibility otherwise must be 0 or 1.
 * replace_channel takes native decoded bytes at the existing channel's bounds/depth.
 * Visibility changes and channel replacement remove thumbnails and mark the saved composite
 * unavailable (1057); raw composite bytes remain stored. These calls do not render layers.
 * Unknown blocks and untouched channels are preserved; edited layer padding is regenerated.
 * Names are capped at 1 MiB; channel input at 1 GiB, excluding copy/encoding memory.
 */
int32_t pc_psd_edit_layer(const pc_psd *doc, size_t layer, pc_psd_bytes name, int8_t visible, pc_psd **out, pc_psd_error *error);
int32_t pc_psd_replace_channel(const pc_psd *doc, size_t layer, int16_t id, pc_psd_bytes data, pc_psd **out, pc_psd_error *error);

/* has_real_merged_data: -1 unknown, 0 false, 1 true.
 * Optional mask/resolution values have present == 0 when absent. Mask parameter
 * presence bits: 1 user density, 2 user feather, 4 vector density, 8 vector feather.
 * Resolution unit 1 is pixels/inch, 2 pixels/cm; display units retain PSD codes.
 * Missing resources return an empty buffer. color_mode_data includes indexed palettes.
 */
int32_t pc_psd_document_info(const pc_psd *doc, pc_psd_document *out, pc_psd_error *error);
int32_t pc_psd_layer_properties(const pc_psd *doc, size_t layer, pc_psd_layer_properties_info *out, pc_psd_error *error);
int32_t pc_psd_layer_mask(const pc_psd *doc, size_t layer, pc_psd_mask *out, pc_psd_error *error);
int32_t pc_psd_resolution(const pc_psd *doc, pc_psd_resolution_info *out, pc_psd_error *error);
int32_t pc_psd_color_mode_data(const pc_psd *doc, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_resource(const pc_psd *doc, uint16_t id, pc_psd_buffer *out, pc_psd_error *error);
/* Channel compression -1 means an empty channel without a stored compression tag. */
int32_t pc_psd_channel_compression(const pc_psd *doc, size_t layer, int16_t id, int32_t *out, pc_psd_error *error);
/* Preorder nodes; parent/divider are layer-record indices, SIZE_MAX means absent.
 * Owned tree storage follows the buffer contract; release with tree_free.
 * Groups deeper than 256 levels return an error.
 */
int32_t pc_psd_layer_tree(const pc_psd *doc, pc_psd_tree *out, pc_psd_error *error);
void pc_psd_tree_free(pc_psd_tree *tree);
/* Enumeration preserves file order, duplicate IDs/keys and raw Pascal name bytes. */
int32_t pc_psd_resource_count(const pc_psd *doc, size_t *out, pc_psd_error *error);
int32_t pc_psd_resource_info(const pc_psd *doc, size_t index, pc_psd_resource_info_value *out, pc_psd_error *error);
int32_t pc_psd_resource_name(const pc_psd *doc, size_t index, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_resource_data(const pc_psd *doc, size_t index, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_layer_block_count(const pc_psd *doc, size_t layer, size_t *out, pc_psd_error *error);
int32_t pc_psd_global_block_count(const pc_psd *doc, size_t *out, pc_psd_error *error);
int32_t pc_psd_layer_block_info(const pc_psd *doc, size_t layer, size_t index, pc_psd_block_info *out, pc_psd_error *error);
int32_t pc_psd_global_block_info(const pc_psd *doc, size_t index, pc_psd_block_info *out, pc_psd_error *error);
int32_t pc_psd_layer_block_data(const pc_psd *doc, size_t layer, size_t index, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_global_block_data(const pc_psd *doc, size_t index, pc_psd_buffer *out, pc_psd_error *error);
/* Embedded thumbnail: format 0 is padded RGB/BGR24, 1 is JPEG. bgr marks legacy 1033.
 * Missing thumbnails have present == 0 and an empty payload. No JPEG decoding or ICC conversion.
 */
int32_t pc_psd_thumbnail_info(const pc_psd *doc, pc_psd_thumbnail *out, pc_psd_error *error);
int32_t pc_psd_thumbnail_data(const pc_psd *doc, pc_psd_buffer *out, pc_psd_error *error);
/* Interleaved sRGB RGBA8, straight alpha; embedded ICC conversion precedes quantization.
 * Unprofiled integer RGB/Gray assume sRGB/sGray; float RGB/Gray assume linear sRGB.
 * Unprofiled Lab assumes D50. CMYK requires an ICC profile. Invalid profiles return errors.
 * No HDR tone mapping; output is clipped after conversion. Layer masks/effects/opacity/fill opacity are not applied.
 * Merged dimensions are the header's; layer dimensions/position are its bounds.
 * Missing real composites return an error. Full-resolution output is capped at 256 MiB.
 */
int32_t pc_psd_merged_rgba8(const pc_psd *doc, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_layer_rgba8(const pc_psd *doc, size_t layer, pc_psd_buffer *out, pc_psd_error *error);
/* Fit within a positive max_edge without upscaling. Free out->data with buffer_free.
 * Alpha-aware bilinear resizing operates in sRGB; the full composite or layer is decoded first.
 */
int32_t pc_psd_preview(const pc_psd *doc, uint32_t max_edge, pc_psd_preview_image *out, pc_psd_error *error);
/* Uses the layer's bounds; an empty layer returns a 0 x 0 preview. */
int32_t pc_psd_layer_preview(const pc_psd *doc, size_t layer, uint32_t max_edge, pc_psd_preview_image *out, pc_psd_error *error);

/* Typed geometry handles own their data independently of the document. Missing data returns NULL.
 * Resource paths use indices, matching resource enumeration; other resource types return NULL.
 * Knot x/y coordinates are fractions of canvas width/height. Slice bounds are left/top/right/bottom;
 * slices_info bounds and slice outsets are top/left/bottom/right. Colour is alpha/red/green/blue.
 * Typed paths reject partial records and allocations above 256 MiB.
 */
int32_t pc_psd_resource_path(const pc_psd *doc, size_t index, pc_psd_path **out, pc_psd_error *error);
int32_t pc_psd_vector_mask(const pc_psd *doc, size_t layer, pc_psd_path **out, pc_psd_error *error);
int32_t pc_psd_path_info(const pc_psd_path *path, pc_psd_path_info_value *out, pc_psd_error *error);
int32_t pc_psd_subpath_info(const pc_psd_path *path, size_t index, pc_psd_subpath *out, pc_psd_error *error);
int32_t pc_psd_knot(const pc_psd_path *path, size_t subpath, size_t index, pc_psd_knot_info *out, pc_psd_error *error);
void pc_psd_path_free(pc_psd_path *path);
int32_t pc_psd_slices(const pc_psd *doc, pc_psd_slice_collection **out, pc_psd_error *error);
int32_t pc_psd_slices_info(const pc_psd_slice_collection *slices, pc_psd_slices_info_value *out, pc_psd_error *error);
int32_t pc_psd_slices_group_name(const pc_psd_slice_collection *slices, pc_psd_buffer *out, pc_psd_error *error);
int32_t pc_psd_slice_info(const pc_psd_slice_collection *slices, size_t index, pc_psd_slice *out, pc_psd_error *error);
/* UTF-8 text; field is name, url, target, message, alt or cell_text. */
int32_t pc_psd_slice_text(const pc_psd_slice_collection *slices, size_t index, pc_psd_bytes field, pc_psd_buffer *out, pc_psd_error *error);
void pc_psd_slices_free(pc_psd_slice_collection *slices);

/* Builder: RGB/Gray/CMYK at 8/16 bits; interleaved colour + alpha, big-endian at 16 bits.
 * Samples are straight; RGB composites are encoded against white at the document depth.
 * CMYK inputs are ink values; stored decoded channels are inverted. User masks are 8-bit.
 * Bounds are top/left/bottom/right. Inputs are borrowed only for the call; the 1 GiB input budget includes per-item overhead, excluding build/encode working memory.
 * Layer input points to aligned readable storage, disjoint from the builder and outputs.
 * A NULL/0 mask means absent. Resource names retain raw Pascal bytes (at most 255).
 * Layer options use nonzero values for true. Supply the composite to produce a real saved preview;
 * build does not render layers. RLE defaults support readers without ZIP composite decoding.
 * Layer records are capped at 32767; nesting at 256. Builder mutators require exclusive access; build returns an independent document and can be repeated.
 */
int32_t pc_psd_builder_new(pc_psd_builder_config config, pc_psd_builder **out, pc_psd_error *error);
int32_t pc_psd_builder_add_layer(pc_psd_builder *builder, const pc_psd_layer_input *layer, pc_psd_error *error);
int32_t pc_psd_builder_composite(pc_psd_builder *builder, pc_psd_bytes pixels, pc_psd_error *error);
int32_t pc_psd_builder_begin_group(pc_psd_builder *builder, pc_psd_bytes name, pc_psd_error *error);
int32_t pc_psd_builder_end_group(pc_psd_builder *builder, pc_psd_error *error);
int32_t pc_psd_builder_resource(pc_psd_builder *builder, uint16_t id, pc_psd_bytes name, pc_psd_bytes data, pc_psd_error *error);
int32_t pc_psd_builder_icc_profile(pc_psd_builder *builder, pc_psd_bytes data, pc_psd_error *error);
int32_t pc_psd_builder_resolution(pc_psd_builder *builder, double dpi, pc_psd_error *error);
int32_t pc_psd_builder_build(const pc_psd_builder *builder, pc_psd **out, pc_psd_error *error);
void pc_psd_builder_free(pc_psd_builder *builder);

#ifdef __cplusplus
}
#endif
#endif
