#include "photocraft_psd.hpp"
#include <fstream>
#include <iostream>

int main(int argc, char **argv) {
    if (argc != 2) { std::cerr << "usage: create <output.psd>\n"; return 1; }
    namespace psd = photocraft::psd;
    try {
        auto builder = psd::builder(pc_psd_builder_config PC_PSD_BUILDER_CONFIG(1, 1));
        const uint8_t pixels[] = {255, 0, 0, 128};
        pc_psd_layer_input layer{{reinterpret_cast<const uint8_t *>("Red"), 3}, {0, 0, 1, 1},
            {pixels, sizeof pixels}, {nullptr, 0}, PC_PSD_LAYER_OPTIONS};
        pc_psd_error error{};
        psd::check(pc_psd_builder_add_layer(builder.get(), &layer, &error), error);
        psd::check(pc_psd_builder_composite(builder.get(), layer.pixels, &error), error);
        psd::check(pc_psd_builder_resolution(builder.get(), 300, &error), error);
        auto doc = psd::build(builder);
        auto bytes = psd::bytes(pc_psd_serialize, doc.get());
        std::ofstream output(argv[1], std::ios::binary);
        output.write(reinterpret_cast<const char *>(bytes.get().data), static_cast<std::streamsize>(bytes.get().len));
        output.close();
        if (!output) throw std::runtime_error("Cannot write output.");
    } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
