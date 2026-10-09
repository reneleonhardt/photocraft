#include "photocraft_psd.hpp"
#include <fstream>
#include <iostream>
#include <iterator>
#include <vector>

int main(int argc, char **argv) {
    if (argc != 3) { std::cerr << "usage: inspect <input.psd> <output.psd>\n"; return 1; }
    try {
        namespace psd = photocraft::psd;
        std::ifstream input(argv[1], std::ios::binary);
        if (!input) throw std::runtime_error("Cannot open input.");
        std::vector<uint8_t> data{std::istreambuf_iterator<char>(input), {}};
        auto doc = psd::parse(data.data(), data.size(), true);
        auto header = psd::call<pc_psd_header_info>(pc_psd_header, doc.get());
        std::cout << header.width << " x " << header.height << ", " << header.depth << " bits, " << header.layer_count << " layers\n";
        auto bytes = psd::bytes(pc_psd_serialize, doc.get());
        std::ofstream output(argv[2], std::ios::binary);
        output.write(reinterpret_cast<const char *>(bytes.get().data), static_cast<std::streamsize>(bytes.get().len));
        output.close();
        if (!output) throw std::runtime_error("Cannot write output.");
    } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
