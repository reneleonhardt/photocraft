#include "photocraft_psd.hpp"
#include <cassert>
#include <fstream>
#include <iterator>
#include <type_traits>
#include <vector>

int main(int argc, char **argv) {
    namespace psd = photocraft::psd;
    static_assert(!std::is_copy_constructible_v<psd::Buffer>);
    static_assert(std::is_nothrow_move_constructible_v<psd::Buffer>);
    assert(argc == 2);
    std::ifstream input(argv[1], std::ios::binary);
    std::vector<uint8_t> data{std::istreambuf_iterator<char>(input), {}};
    auto doc = psd::parse(data.data(), data.size(), true);
    auto header = psd::call<pc_psd_header_info>(pc_psd_header, doc.get());
    assert(header.width && header.height);
    auto decoded = psd::bytes(pc_psd_decode_merged, doc.get());
    auto moved = std::move(decoded);
    assert(!decoded.get().data && moved.get().len);
    decoded = std::move(moved);
    assert(!moved.get().data && decoded.get().len);
    pc_psd_error error{};
    psd::FloatImage pixels;
    psd::check(pc_psd_merged_pixels_f32(doc.get(), pixels.put(), &error), error);
    auto floats = std::move(pixels);
    assert(!pixels.get().data.data && floats.get().data.len == 8);
    psd::check(pc_psd_serialize(doc.get(), decoded.put(), &error), error);
    assert(decoded.get().len == data.size());
    auto edited = psd::edit_layer(doc, 0, {}, 0);
    assert(psd::call<pc_psd_document>(pc_psd_document_info, edited.get()).has_real_merged_data == 0);
    auto channel = psd::bytes(pc_psd_decode_channel, doc.get(), size_t{0}, int16_t{0});
    auto changed = psd::replace_channel(edited, 0, 0, {channel.get().data, channel.get().len});
    assert(psd::call<pc_psd_layer_properties_info>(pc_psd_layer_properties, changed.get(), size_t{0}).visible == 0);
    psd::Tree tree;
    psd::check(pc_psd_layer_tree(doc.get(), tree.put(), &error), error);
    psd::Preview preview;
    if (header.color_mode == PC_PSD_RGB) psd::check(pc_psd_preview(doc.get(), 1, preview.put(), &error), error);
    psd::Path path;
    for (size_t i = 0; i < psd::call<size_t>(pc_psd_resource_count, doc.get()); ++i) {
        if (psd::call<pc_psd_resource_info_value>(pc_psd_resource_info, doc.get(), i).id == 2000)
            path.reset(psd::call<pc_psd_path *>(pc_psd_resource_path, doc.get(), i));
    }
    psd::Slices slices(psd::call<pc_psd_slice_collection *>(pc_psd_slices, doc.get()));
    doc.reset();
    assert(floats.get().data.data[0] == 1);
    assert(decoded.get().data && decoded.get().len);
    assert(path && psd::call<pc_psd_path_info_value>(pc_psd_path_info, path.get()).subpath_count == 1);
    assert(slices && psd::call<pc_psd_slices_info_value>(pc_psd_slices_info, slices.get()).count == 1);
    try { psd::parse(nullptr, 0); assert(false); } catch (const std::runtime_error &e) { assert(*e.what()); }
    auto config = pc_psd_builder_config PC_PSD_BUILDER_CONFIG(1, 1);
    auto builder = psd::builder(config);
    uint8_t pixel[] = {255, 0, 0, 128};
    pc_psd_layer_input layer{{nullptr, 0}, {0, 0, 1, 1}, {pixel, 4}, {nullptr, 0}, PC_PSD_LAYER_OPTIONS};
    psd::check(pc_psd_builder_add_layer(builder.get(), &layer, &error), error);
    psd::check(pc_psd_builder_composite(builder.get(), {pixel, 4}, &error), error);
    auto created = psd::build(builder);
    builder.reset();
    assert(psd::bytes(pc_psd_merged_rgba8, created.get()).get().len == 4);
}
