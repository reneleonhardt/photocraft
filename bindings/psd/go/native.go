// Package psd reads and writes PSD/PSB through the PhotoCraft C ABI.
// Close documents and builders promptly; returned pixels and bytes are Go-owned.
package psd

/*
#cgo CFLAGS: -I${SRCDIR}/../include
#cgo LDFLAGS: -lphotocraft_psd_bindings
#include "photocraft_psd.h"
*/
import "C"

import (
	"errors"
	"image"
	"runtime"
	"sync"
	"unsafe"
)

var ErrClosed = errors.New("PSD handle is closed")

type Header struct {
	Width, Height                       uint32
	Version, Channels, Depth, ColorMode uint16
	LayerCount                          int
}
type DocumentInfo struct {
	Compression       uint16
	MergedHasAlpha    bool
	HasRealMergedData *bool
}
type ResolutionInfo struct {
	Horizontal, Vertical                                float64
	HorizontalUnit, VerticalUnit, WidthUnit, HeightUnit uint16
}
type ThumbnailInfo struct {
	Format, Width, Height, RowBytes uint32
	ResourceID                      uint16
	BGR                             bool
}
type FloatImage struct {
	Width, Height       uint32
	ColorMode, Channels uint16
	Data                []float32
}
type Layer struct {
	Name         string
	Bounds       image.Rectangle
	ChannelCount int
}
type LayerProperties struct {
	BlendMode                      [4]byte
	SectionType                    uint32
	ID                             *uint32
	Visible                        bool
	Opacity, FillOpacity, Clipping uint8
}
type Mask struct {
	Bounds, RealBounds                                                                     image.Rectangle
	Flags, DefaultColor, Parameters, UserDensity, VectorDensity, RealFlags, RealBackground uint8
	UserFeather, VectorFeather                                                             float64
	HasReal                                                                                bool
}
type TreeNode struct {
	Index, Parent, Divider int // Parent/Divider == -1 when absent.
	IsGroup                bool
}
type Config struct {
	Width, Height                          uint32
	ColorMode, Depth, Version, Compression uint16
}
type LayerOptions struct {
	BlendMode                         [4]byte
	MaskBounds                        image.Rectangle
	Opacity, FillOpacity, MaskDefault uint8
	Visible, Clipping, MaskDisabled   bool
}

const (
	Grayscale     uint16 = C.PC_PSD_GRAYSCALE
	RGB           uint16 = C.PC_PSD_RGB
	CMYK          uint16 = C.PC_PSD_CMYK
	Raw           uint16 = C.PC_PSD_RAW
	RLE           uint16 = C.PC_PSD_RLE
	ZIP           uint16 = C.PC_PSD_ZIP
	ZIPPrediction uint16 = C.PC_PSD_ZIP_PREDICTION
)

func DefaultConfig(width, height uint32) Config { return Config{width, height, RGB, 8, 1, RLE} }
func DefaultLayerOptions() LayerOptions {
	return LayerOptions{BlendMode: [4]byte{'n', 'o', 'r', 'm'}, Opacity: 255, FillOpacity: 255, MaskDefault: 255, Visible: true}
}

type Document struct{ state *handle }
type Builder struct{ state *handle }
type allocation struct {
	ptr     unsafe.Pointer
	builder bool
}
type handle struct {
	sync.RWMutex
	allocation
	cleanup runtime.Cleanup
}

// SAFETY: pointers originate in C; the cleanup argument contains no Go owner reference.
func release(a allocation) {
	if a.builder {
		C.pc_psd_builder_free((*C.pc_psd_builder)(a.ptr))
	} else {
		C.pc_psd_free((*C.pc_psd)(a.ptr))
	}
}
func own(a allocation) *handle {
	h := &handle{allocation: a}
	h.cleanup = runtime.AddCleanup(h, release, a)
	return h
}
func (h *handle) close() {
	if h == nil {
		return
	}
	h.Lock()
	defer h.Unlock()
	if h.ptr != nil {
		h.cleanup.Stop()
		release(h.allocation)
		h.ptr = nil
	}
}
func (h *handle) call(fn func(unsafe.Pointer) error) error {
	if h == nil {
		return ErrClosed
	}
	if h.builder {
		h.Lock()
		defer h.Unlock()
	} else {
		h.RLock()
		defer h.RUnlock()
	}
	defer runtime.KeepAlive(h)
	if h.ptr == nil {
		return ErrClosed
	}
	return fn(h.ptr)
}
func query[T any](h *handle, fn func(unsafe.Pointer, *T, *C.pc_psd_error) C.int32_t) (out T, err error) {
	err = h.call(func(ptr unsafe.Pointer) error {
		var e C.pc_psd_error
		return check(fn(ptr, &out, &e), &e)
	})
	return
}
func (d *Document) Close() { d.state.close() }
func (b *Builder) Close()  { b.state.close() }
func check(status C.int32_t, e *C.pc_psd_error) error {
	if status == C.PC_PSD_OK {
		return nil
	}
	return errors.New(C.GoString(&e.message[0]))
}
func abi() error {
	if C.pc_psd_abi_version() != 1 {
		return errors.New("unsupported PSD C ABI")
	}
	return nil
}

// SAFETY: pin byte backing arrays while C structs containing their pointers are passed to C.
// The ABI borrows inputs only for the call and returns independently owned allocations.
func borrow(data []byte, pins *runtime.Pinner) C.pc_psd_bytes {
	if len(data) == 0 {
		return C.pc_psd_bytes{}
	}
	ptr := unsafe.SliceData(data)
	pins.Pin(ptr)
	return C.pc_psd_bytes{data: (*C.uint8_t)(ptr), len: C.size_t(len(data))}
}
func copyBuffer(out *C.pc_psd_buffer) ([]byte, error) {
	defer C.pc_psd_buffer_free(out)
	if out.len > 1<<31-1 {
		return nil, errors.New("PSD output exceeds Go copy limit")
	}
	// SAFETY: C guarantees out.len readable bytes, retained until the deferred free.
	return C.GoBytes(unsafe.Pointer(out.data), C.int(out.len)), nil
}
func Parse(data []byte, lazy bool) (*Document, error) {
	if err := abi(); err != nil {
		return nil, err
	}
	var pins runtime.Pinner
	defer pins.Unpin()
	input := borrow(data, &pins)
	var ptr *C.pc_psd
	var e C.pc_psd_error
	var status C.int32_t
	if lazy {
		status = C.pc_psd_parse_lazy(input.data, input.len, &ptr, &e)
	} else {
		status = C.pc_psd_parse(input.data, input.len, &ptr, &e)
	}
	if err := check(status, &e); err != nil {
		return nil, err
	}
	return &Document{own(allocation{ptr: unsafe.Pointer(ptr)})}, nil
}
func (d *Document) Header() (Header, error) {
	out, err := query(d.state, func(ptr unsafe.Pointer, out *C.pc_psd_header_info, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_header((*C.pc_psd)(ptr), out, e)
	})
	return Header{uint32(out.width), uint32(out.height), uint16(out.version), uint16(out.channels), uint16(out.depth), uint16(out.color_mode), int(out.layer_count)}, err
}
func (d *Document) Info() (result DocumentInfo, err error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.pc_psd_document, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_document_info((*C.pc_psd)(p), o, e)
	})
	result.Compression, result.MergedHasAlpha = uint16(o.compression), o.merged_has_alpha != 0
	if err == nil && o.has_real_merged_data >= 0 {
		value := o.has_real_merged_data != 0
		result.HasRealMergedData = &value
	}
	return result, err
}
func (d *Document) ChannelBounds(layer int, id int16) (image.Rectangle, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *[4]C.int32_t, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_channel_bounds((*C.pc_psd)(p), C.size_t(layer), C.int16_t(id), &o[0], e)
	})
	return bounds(o), err
}
func bounds(b [4]C.int32_t) image.Rectangle {
	return image.Rectangle{Min: image.Pt(int(b[1]), int(b[0])), Max: image.Pt(int(b[3]), int(b[2]))}
}
func (d *Document) ChannelCompression(layer int, id int16) (*uint16, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.int32_t, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_channel_compression((*C.pc_psd)(p), C.size_t(layer), C.int16_t(id), o, e)
	})
	if err != nil || o < 0 {
		return nil, err
	}
	value := uint16(o)
	return &value, nil
}
func (d *Document) Resolution() (*ResolutionInfo, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.pc_psd_resolution_info, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_resolution((*C.pc_psd)(p), o, e)
	})
	if err != nil || o.present == 0 {
		return nil, err
	}
	return &ResolutionInfo{float64(o.horizontal), float64(o.vertical), uint16(o.horizontal_unit), uint16(o.vertical_unit), uint16(o.width_unit), uint16(o.height_unit)}, nil
}
func (d *Document) ThumbnailInfo() (*ThumbnailInfo, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.pc_psd_thumbnail, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_thumbnail_info((*C.pc_psd)(p), o, e)
	})
	if err != nil || o.present == 0 {
		return nil, err
	}
	return &ThumbnailInfo{uint32(o.format), uint32(o.width), uint32(o.height), uint32(o.row_bytes), uint16(o.resource_id), o.bgr != 0}, nil
}
func (d *Document) ThumbnailData() ([]byte, error) {
	return d.bytes(func(p *C.pc_psd, o *C.pc_psd_buffer, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_thumbnail_data(p, o, e)
	})
}
func (d *Document) floats(fn func(*C.pc_psd, *C.pc_psd_float_image, *C.pc_psd_error) C.int32_t) (FloatImage, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.pc_psd_float_image, e *C.pc_psd_error) C.int32_t {
		return fn((*C.pc_psd)(p), o, e)
	})
	if err != nil {
		return FloatImage{}, err
	}
	defer C.pc_psd_float_buffer_free(&o.data)
	if o.data.len > 1<<31-1 {
		return FloatImage{}, errors.New("PSD output exceeds Go copy limit")
	}
	// SAFETY: C owns o.data.len floats until the deferred free; only an independent copy escapes.
	data := append([]float32(nil), unsafe.Slice((*float32)(unsafe.Pointer(o.data.data)), int(o.data.len))...)
	return FloatImage{uint32(o.width), uint32(o.height), uint16(o.color_mode), uint16(o.channels), data}, nil
}
func (d *Document) MergedPixelsF32() (FloatImage, error) {
	return d.floats(func(p *C.pc_psd, o *C.pc_psd_float_image, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_merged_pixels_f32(p, o, e)
	})
}
func (d *Document) LayerPixelsF32(layer int) (FloatImage, error) {
	return d.floats(func(p *C.pc_psd, o *C.pc_psd_float_image, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_layer_pixels_f32(p, C.size_t(layer), o, e)
	})
}
func (d *Document) edited(fn func(*C.pc_psd, **C.pc_psd, *C.pc_psd_error) C.int32_t) (*Document, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o **C.pc_psd, e *C.pc_psd_error) C.int32_t { return fn((*C.pc_psd)(p), o, e) })
	if err != nil {
		return nil, err
	}
	return &Document{own(allocation{ptr: unsafe.Pointer(o)})}, nil
}

// Nil name/visible keeps the field; an empty name clears it. The source is unchanged.
func (d *Document) EditLayer(layer int, name *string, visible *bool) (*Document, error) {
	var pins runtime.Pinner
	defer pins.Unpin()
	var input C.pc_psd_bytes
	if name != nil {
		input = borrow(append([]byte(*name), 0), &pins)
		input.len-- // Keep a non-null pointer for an explicitly empty name.
	}
	value := C.int8_t(-1)
	if visible != nil {
		value = C.int8_t(flag(*visible))
	}
	return d.edited(func(p *C.pc_psd, o **C.pc_psd, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_edit_layer(p, C.size_t(layer), input, value, o, e)
	})
}
func (d *Document) ReplaceChannel(layer int, id int16, data []byte) (*Document, error) {
	var pins runtime.Pinner
	defer pins.Unpin()
	input := borrow(data, &pins)
	return d.edited(func(p *C.pc_psd, o **C.pc_psd, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_replace_channel(p, C.size_t(layer), C.int16_t(id), input, o, e)
	})
}
func (d *Document) bytes(fn func(*C.pc_psd, *C.pc_psd_buffer, *C.pc_psd_error) C.int32_t) ([]byte, error) {
	out, err := query(d.state, func(ptr unsafe.Pointer, out *C.pc_psd_buffer, e *C.pc_psd_error) C.int32_t {
		return fn((*C.pc_psd)(ptr), out, e)
	})
	if err != nil {
		return nil, err
	}
	return copyBuffer(&out)
}
func (d *Document) ToBytes() ([]byte, error) {
	return d.bytes(func(p *C.pc_psd, o *C.pc_psd_buffer, e *C.pc_psd_error) C.int32_t { return C.pc_psd_serialize(p, o, e) })
}
func (d *Document) DecodeMerged() ([]byte, error) {
	return d.bytes(func(p *C.pc_psd, o *C.pc_psd_buffer, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_decode_merged(p, o, e)
	})
}
func (d *Document) DecodeChannel(layer int, id int16) ([]byte, error) {
	return d.bytes(func(p *C.pc_psd, o *C.pc_psd_buffer, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_decode_channel(p, C.size_t(layer), C.int16_t(id), o, e)
	})
}
func (d *Document) ICCProfile() ([]byte, error) { return d.Resource(1039) }
func (d *Document) Resource(id uint16) ([]byte, error) {
	return d.bytes(func(p *C.pc_psd, o *C.pc_psd_buffer, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_resource(p, C.uint16_t(id), o, e)
	})
}
func (d *Document) ColorModeData() ([]byte, error) {
	return d.bytes(func(p *C.pc_psd, o *C.pc_psd_buffer, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_color_mode_data(p, o, e)
	})
}
func (d *Document) Layer(index int) (result Layer, err error) {
	err = d.state.call(func(ptr unsafe.Pointer) error {
		var info C.pc_psd_layer
		var out C.pc_psd_buffer
		var e C.pc_psd_error
		p := (*C.pc_psd)(ptr)
		if err := check(C.pc_psd_layer_info(p, C.size_t(index), &info, &e), &e); err != nil {
			return err
		}
		if err := check(C.pc_psd_layer_name(p, C.size_t(index), &out, &e), &e); err != nil {
			return err
		}
		name, err := copyBuffer(&out)
		if err != nil {
			return err
		}
		result = Layer{string(name), image.Rectangle{Min: image.Pt(int(info.left), int(info.top)), Max: image.Pt(int(info.right), int(info.bottom))}, int(info.channel_count)}
		return nil
	})
	return
}
func (d *Document) ChannelID(layer, channel int) (int16, error) {
	out, err := query(d.state, func(ptr unsafe.Pointer, out *C.int16_t, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_channel_id((*C.pc_psd)(ptr), C.size_t(layer), C.size_t(channel), out, e)
	})
	return int16(out), err
}
func (d *Document) LayerProperties(layer int) (result LayerProperties, err error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.pc_psd_layer_properties_info, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_layer_properties((*C.pc_psd)(p), C.size_t(layer), o, e)
	})
	for i, v := range o.blend_mode {
		result.BlendMode[i] = byte(v)
	}
	result.SectionType, result.Visible = uint32(o.section_type), o.visible != 0
	result.Opacity, result.FillOpacity, result.Clipping = uint8(o.opacity), uint8(o.fill_opacity), uint8(o.clipping)
	if o.has_id != 0 {
		id := uint32(o.id)
		result.ID = &id
	}
	return result, err
}
func (d *Document) LayerMask(layer int) (*Mask, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.pc_psd_mask, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_layer_mask((*C.pc_psd)(p), C.size_t(layer), o, e)
	})
	if err != nil || o.present == 0 {
		return nil, err
	}
	return &Mask{Bounds: bounds(o.bounds), RealBounds: bounds(o.real_bounds), Flags: uint8(o.flags), DefaultColor: uint8(o.default_color),
		Parameters: uint8(o.parameters), UserDensity: uint8(o.user_density), VectorDensity: uint8(o.vector_density),
		UserFeather: float64(o.user_feather), VectorFeather: float64(o.vector_feather), HasReal: o.has_real != 0,
		RealFlags: uint8(o.real_flags), RealBackground: uint8(o.real_background)}, nil
}
func (d *Document) LayerTree() ([]TreeNode, error) {
	o, err := query(d.state, func(p unsafe.Pointer, o *C.pc_psd_tree, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_layer_tree((*C.pc_psd)(p), o, e)
	})
	if err != nil {
		return nil, err
	}
	defer C.pc_psd_tree_free(&o)
	result := make([]TreeNode, int(o.len))
	// SAFETY: the C tree owns o.len initialized nodes until the deferred free; no pointers escape.
	for i, n := range unsafe.Slice(o.data, int(o.len)) {
		result[i] = TreeNode{int(n.index), int(n.parent), int(n.divider), n.is_group != 0}
	}
	return result, nil
}
func (d *Document) preview(layer int, maxEdge uint32, merged bool) (*image.NRGBA, error) {
	out, err := query(d.state, func(ptr unsafe.Pointer, out *C.pc_psd_preview_image, e *C.pc_psd_error) C.int32_t {
		if merged {
			return C.pc_psd_preview((*C.pc_psd)(ptr), C.uint32_t(maxEdge), out, e)
		}
		return C.pc_psd_layer_preview((*C.pc_psd)(ptr), C.size_t(layer), C.uint32_t(maxEdge), out, e)
	})
	if err != nil {
		return nil, err
	}
	pixels, err := copyBuffer(&out.data)
	if err != nil {
		return nil, err
	}
	return &image.NRGBA{Pix: pixels, Stride: int(out.width) * 4, Rect: image.Rect(0, 0, int(out.width), int(out.height))}, nil
}

// Preview and LayerPreview return owned, straight-alpha sRGB RGBA8 images.
func (d *Document) Preview(maxEdge uint32) (*image.NRGBA, error) { return d.preview(0, maxEdge, true) }
func (d *Document) LayerPreview(layer int, maxEdge uint32) (*image.NRGBA, error) {
	return d.preview(layer, maxEdge, false)
}

func NewBuilder(config Config) (*Builder, error) {
	if err := abi(); err != nil {
		return nil, err
	}
	c := C.pc_psd_builder_config{width: C.uint32_t(config.Width), height: C.uint32_t(config.Height), color_mode: C.uint16_t(config.ColorMode), depth: C.uint16_t(config.Depth), version: C.uint16_t(config.Version), compression: C.uint16_t(config.Compression)}
	var ptr *C.pc_psd_builder
	var e C.pc_psd_error
	if err := check(C.pc_psd_builder_new(c, &ptr, &e), &e); err != nil {
		return nil, err
	}
	return &Builder{own(allocation{unsafe.Pointer(ptr), true})}, nil
}
func (b *Builder) mutate(fn func(*C.pc_psd_builder, *C.pc_psd_error) C.int32_t) error {
	return b.state.call(func(ptr unsafe.Pointer) error {
		var e C.pc_psd_error
		return check(fn((*C.pc_psd_builder)(ptr), &e), &e)
	})
}
func rectangle(rect image.Rectangle) ([4]C.int32_t, error) {
	values := [4]int{rect.Min.Y, rect.Min.X, rect.Max.Y, rect.Max.X}
	var out [4]C.int32_t
	for i, v := range values {
		if int64(v) != int64(int32(v)) {
			return out, errors.New("PSD bounds exceed int32")
		}
		out[i] = C.int32_t(v)
	}
	return out, nil
}
func flag(value bool) C.uint8_t {
	if value {
		return 1
	}
	return 0
}

// A nil options pointer uses defaults. A nil or empty mask omits the raster mask.
func (b *Builder) AddLayer(name string, bounds image.Rectangle, pixels []byte, options *LayerOptions, mask []byte) error {
	rect, err := rectangle(bounds)
	if err != nil {
		return err
	}
	o := DefaultLayerOptions()
	if options != nil {
		o = *options
	}
	maskRect, err := rectangle(o.MaskBounds)
	if err != nil {
		return err
	}
	var pins runtime.Pinner
	defer pins.Unpin()
	input := C.pc_psd_layer_input{name: borrow([]byte(name), &pins), bounds: rect, pixels: borrow(pixels, &pins), mask: borrow(mask, &pins)}
	for i, v := range o.BlendMode {
		input.options.blend_mode[i] = C.uint8_t(v)
	}
	input.options.mask_bounds = maskRect
	input.options.opacity = C.uint8_t(o.Opacity)
	input.options.fill_opacity = C.uint8_t(o.FillOpacity)
	input.options.mask_default = C.uint8_t(o.MaskDefault)
	input.options.visible = flag(o.Visible)
	input.options.clipping = flag(o.Clipping)
	input.options.mask_disabled = flag(o.MaskDisabled)
	return b.mutate(func(p *C.pc_psd_builder, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_builder_add_layer(p, &input, e)
	})
}
func (b *Builder) Composite(pixels []byte) error {
	var pins runtime.Pinner
	defer pins.Unpin()
	data := borrow(pixels, &pins)
	return b.mutate(func(p *C.pc_psd_builder, e *C.pc_psd_error) C.int32_t { return C.pc_psd_builder_composite(p, data, e) })
}
func (b *Builder) BeginGroup(name string) error {
	var pins runtime.Pinner
	defer pins.Unpin()
	data := borrow([]byte(name), &pins)
	return b.mutate(func(p *C.pc_psd_builder, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_builder_begin_group(p, data, e)
	})
}
func (b *Builder) EndGroup() error {
	return b.mutate(func(p *C.pc_psd_builder, e *C.pc_psd_error) C.int32_t { return C.pc_psd_builder_end_group(p, e) })
}
func (b *Builder) Resource(id uint16, data, name []byte) error {
	var pins runtime.Pinner
	defer pins.Unpin()
	n, d := borrow(name, &pins), borrow(data, &pins)
	return b.mutate(func(p *C.pc_psd_builder, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_builder_resource(p, C.uint16_t(id), n, d, e)
	})
}
func (b *Builder) ICCProfile(data []byte) error { return b.Resource(1039, data, nil) }
func (b *Builder) Resolution(dpi float64) error {
	return b.mutate(func(p *C.pc_psd_builder, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_builder_resolution(p, C.double(dpi), e)
	})
}
func (b *Builder) Build() (*Document, error) {
	out, err := query(b.state, func(ptr unsafe.Pointer, out **C.pc_psd, e *C.pc_psd_error) C.int32_t {
		return C.pc_psd_builder_build((*C.pc_psd_builder)(ptr), out, e)
	})
	if err != nil {
		return nil, err
	}
	return &Document{own(allocation{ptr: unsafe.Pointer(out)})}, nil
}
