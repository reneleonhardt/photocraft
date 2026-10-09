#ifndef PHOTOCRAFT_PSD_HPP
#define PHOTOCRAFT_PSD_HPP

#include "photocraft_psd.h"
#include <memory>
#include <stdexcept>
#include <utility>

namespace photocraft::psd {

inline void check(int32_t status, const pc_psd_error &error) {
    if (status != PC_PSD_OK) throw std::runtime_error(error.message);
}

template<class T, auto Free> struct Deleter {
    void operator()(T *value) const noexcept { Free(value); }
};
template<class T, auto Free> using Handle = std::unique_ptr<T, Deleter<T, Free>>;
using Document = Handle<pc_psd, pc_psd_free>;
using Builder = Handle<pc_psd_builder, pc_psd_builder_free>;
using Path = Handle<pc_psd_path, pc_psd_path_free>;
using Slices = Handle<pc_psd_slice_collection, pc_psd_slices_free>;

// Owned C outputs; put() releases the previous allocation before reuse.
template<class T, auto Free> class Output {
    T value_{};
public:
    Output() = default;
    Output(const Output &) = delete;
    Output &operator=(const Output &) = delete;
    Output(Output &&other) noexcept : value_(std::exchange(other.value_, T{})) {}
    Output &operator=(Output &&other) noexcept {
        if (this != &other) {
            Free(&value_);
            value_ = std::exchange(other.value_, T{});
        }
        return *this;
    }
    ~Output() { Free(&value_); }
    const T &get() const noexcept { return value_; }
    T *put() noexcept { Free(&value_); return &value_; }
};
using Buffer = Output<pc_psd_buffer, pc_psd_buffer_free>;
using Tree = Output<pc_psd_tree, pc_psd_tree_free>;
inline void free_preview(pc_psd_preview_image *value) noexcept { pc_psd_buffer_free(&value->data); }
using Preview = Output<pc_psd_preview_image, free_preview>;
inline void free_float_image(pc_psd_float_image *value) noexcept { pc_psd_float_buffer_free(&value->data); }
using FloatImage = Output<pc_psd_float_image, free_float_image>;

// POD outputs and byte outputs share the C API's argument order.
template<class T, class Function, class... Args> T call(Function function, Args... args) {
    T value{};
    pc_psd_error error{};
    check(function(args..., &value, &error), error);
    return value;
}
template<class Function, class... Args> Buffer bytes(Function function, Args... args) {
    Buffer value;
    pc_psd_error error{};
    check(function(args..., value.put(), &error), error);
    return value;
}
inline Document parse(const uint8_t *data, size_t size, bool lazy = false) {
    return Document(call<pc_psd *>(lazy ? pc_psd_parse_lazy : pc_psd_parse, data, size));
}
inline Builder builder(pc_psd_builder_config config) {
    return Builder(call<pc_psd_builder *>(pc_psd_builder_new, config));
}
inline Document build(const Builder &value) {
    return Document(call<pc_psd *>(pc_psd_builder_build, value.get()));
}
inline Document edit_layer(const Document &value, size_t layer, pc_psd_bytes name = {}, int8_t visible = -1) {
    return Document(call<pc_psd *>(pc_psd_edit_layer, value.get(), layer, name, visible));
}
inline Document replace_channel(const Document &value, size_t layer, int16_t id, pc_psd_bytes data) {
    return Document(call<pc_psd *>(pc_psd_replace_channel, value.get(), layer, id, data));
}

} // namespace photocraft::psd
#endif
