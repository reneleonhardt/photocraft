package psd

import (
	"bytes"
	"encoding/binary"
	"errors"
	"image"
	"image/png"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"testing"
)

func ok(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}
func wantError(t *testing.T, err error) {
	t.Helper()
	if err == nil {
		t.Fatal("expected error")
	}
}
func TestFixtures(t *testing.T) {
	paths, err := filepath.Glob("../target/fixtures/*.psd")
	ok(t, err)
	if len(paths) == 0 {
		t.Fatal("run check.sh c to generate fixtures")
	}
	paths = append(paths, "../target/fixture.psd")
	for _, path := range paths {
		t.Run(filepath.Base(path), func(t *testing.T) {
			input, err := os.ReadFile(path)
			ok(t, err)
			doc, err := Parse(input, true)
			ok(t, err)
			defer func() {
				if doc != nil {
					doc.Close()
				}
			}()
			header, err := doc.Header()
			ok(t, err)
			output, err := doc.ToBytes()
			ok(t, err)
			if !bytes.Equal(input, output) {
				t.Fatal("serialization")
			}
			pixels, err := doc.DecodeMerged()
			ok(t, err)
			if len(pixels) == 0 {
				t.Fatal("empty merged data")
			}
			profile, err := doc.ICCProfile()
			ok(t, err)
			resource, err := doc.Resource(1039)
			ok(t, err)
			if !bytes.Equal(profile, resource) {
				t.Fatal("ICC resource")
			}
			_, err = doc.ColorModeData()
			ok(t, err)
			resource, err = doc.Resource(65535)
			ok(t, err)
			if len(resource) != 0 {
				t.Fatal("missing resource")
			}
			info, err := doc.Info()
			ok(t, err)
			dpi, err := doc.Resolution()
			ok(t, err)
			thumb, err := doc.ThumbnailInfo()
			ok(t, err)
			thumbnail, err := doc.ThumbnailData()
			ok(t, err)
			floats, err := doc.MergedPixelsF32()
			ok(t, err)
			if !info.MergedHasAlpha || info.HasRealMergedData == nil || !*info.HasRealMergedData || dpi == nil || *dpi != (ResolutionInfo{300, 300, 1, 1, 1, 1}) || thumb == nil || *thumb != (ThumbnailInfo{0, 2, 1, 8, 1036, false}) || !bytes.Equal(thumbnail, []byte{255, 0, 0, 255, 0, 0, 0, 0}) || floats.Width != 2 || floats.Height != 1 || floats.ColorMode != RGB || floats.Channels != 4 || len(floats.Data) != 8 || floats.Data[0] != 1 || math.Abs(float64(floats.Data[5])) > 0.0001 || math.Abs(float64(floats.Data[7])-.5) > .003 {
				t.Fatal("extended metadata/pixels", info, dpi, thumb, floats)
			}
			raster := 0
			if header.LayerCount == 3 {
				raster = 1
			}
			compression, err := doc.ChannelCompression(raster, 0)
			ok(t, err)
			if compression == nil || *compression != info.Compression {
				t.Fatal("compression")
			}
			name := "Ink 🎨"
			renamed, err := doc.EditLayer(raster, &name, nil)
			ok(t, err)
			original, err := doc.DecodeChannel(raster, 0)
			ok(t, err)
			replacement := make([]byte, len(original))
			if header.Depth == 32 {
				for i := 0; i < len(replacement); i += 4 {
					binary.BigEndian.PutUint32(replacement[i:], math.Float32bits(2))
				}
			}
			changed, err := renamed.ReplaceChannel(raster, 0, replacement)
			ok(t, err)
			renamed.Close()
			if header.LayerCount == 1 {
				black, hidden := "Black", false
				edited, err := changed.EditLayer(raster, &black, &hidden)
				ok(t, err)
				output, err := edited.ToBytes()
				ok(t, err)
				edited.Close()
				ok(t, os.WriteFile(filepath.Join(filepath.Dir(path), "edited-go.psd"), output, 0600))
			}
			empty, hidden := "", false
			cleared, err := changed.EditLayer(raster, &empty, &hidden)
			ok(t, err)
			changed.Close()
			data, err := cleared.ToBytes()
			ok(t, err)
			cleared.Close()
			reopened, err := Parse(data, false)
			ok(t, err)
			l, err := reopened.Layer(raster)
			ok(t, err)
			properties, err := reopened.LayerProperties(raster)
			ok(t, err)
			updated, err := reopened.DecodeChannel(raster, 0)
			ok(t, err)
			saved, err := reopened.Info()
			ok(t, err)
			thumb, err = reopened.ThumbnailInfo()
			ok(t, err)
			thumbnail, err = reopened.ThumbnailData()
			ok(t, err)
			layerPixels, err := reopened.LayerPixelsF32(raster)
			ok(t, err)
			want := float32(0)
			if header.Depth == 32 {
				want = 2
			}
			if l.Name != "" || properties.Visible || saved.HasRealMergedData == nil || *saved.HasRealMergedData || thumb != nil || len(thumbnail) != 0 || !bytes.Equal(updated, replacement) || layerPixels.Data[0] != want {
				t.Fatal("edits")
			}
			_, err = reopened.MergedPixelsF32()
			wantError(t, err)
			reopened.Close()
			originalLayer, err := doc.Layer(raster)
			ok(t, err)
			if originalLayer.Name != "Red ☺" {
				t.Fatal("source changed")
			}
			_, err = doc.EditLayer(-1, nil, nil)
			wantError(t, err)
			_, err = doc.ReplaceChannel(raster, 123, nil)
			wantError(t, err)
			_, err = doc.ReplaceChannel(raster, 0, nil)
			wantError(t, err)
			_, err = doc.LayerPixelsF32(-1)
			wantError(t, err)
			_, err = doc.ChannelCompression(raster, 123)
			wantError(t, err)
			for i := 0; i < header.LayerCount; i++ {
				layer, err := doc.Layer(i)
				ok(t, err)
				for j := 0; j < layer.ChannelCount; j++ {
					id, err := doc.ChannelID(i, j)
					ok(t, err)
					channelBounds, err := doc.ChannelBounds(i, id)
					ok(t, err)
					if id >= -1 && channelBounds != layer.Bounds {
						t.Fatal("channel bounds")
					}
					_, err = doc.DecodeChannel(i, id)
					ok(t, err)
				}
			}
			if header.ColorMode == RGB {
				tree, err := doc.LayerTree()
				ok(t, err)
				if header.LayerCount == 3 {
					if len(tree) != 2 || tree[0] != (TreeNode{2, -1, 0, true}) || tree[1] != (TreeNode{1, 2, -1, false}) {
						t.Fatal("layer tree")
					}
					properties, err := doc.LayerProperties(1)
					ok(t, err)
					mask, err := doc.LayerMask(1)
					ok(t, err)
					if properties.Visible || properties.Opacity != 127 || properties.FillOpacity != 128 || properties.Clipping != 1 || properties.ID == nil || properties.BlendMode != ([4]byte{'n', 'o', 'r', 'm'}) || mask == nil || mask.Bounds != image.Rect(0, 2, 1, 3) || mask.Flags != 2 {
						t.Fatal("layer metadata")
					}
				} else {
					mask, err := doc.LayerMask(0)
					ok(t, err)
					if mask != nil || len(tree) != 1 {
						t.Fatal("absent mask")
					}
				}
				_, err = doc.LayerProperties(-1)
				wantError(t, err)
				_, err = doc.LayerMask(header.LayerCount)
				wantError(t, err)
				preview, err := doc.Preview(1)
				ok(t, err)
				var encoded bytes.Buffer
				ok(t, png.Encode(&encoded, preview))
				decoded, err := png.Decode(&encoded)
				ok(t, err)
				if decoded.Bounds() != preview.Bounds() {
					t.Fatal("Go image bridge")
				}
				if header.LayerCount > 0 {
					_, err = doc.LayerPreview(0, 1)
					ok(t, err)
				}
			}
			var wg sync.WaitGroup
			for i := 0; i < 8; i++ {
				wg.Go(func() {
					copy, err := doc.EditLayer(raster, nil, nil)
					if err != nil {
						t.Error(err)
						return
					}
					defer copy.Close()
					pixels, err := copy.MergedPixelsF32()
					if err != nil {
						t.Error(err)
					} else if pixels.Data[0] != 1 {
						t.Error("concurrent float/edit")
					}
				})
			}
			wg.Wait()
			_, err = doc.Layer(-1)
			wantError(t, err)
			_, err = doc.ChannelID(0, -1)
			wantError(t, err)
			copy := *doc
			doc = nil
			runtime.GC()
			_, err = copy.Header()
			ok(t, err)
			for i := 0; i < 8; i++ {
				wg.Go(func() {
					_, err := copy.Header()
					if err != nil && !errors.Is(err, ErrClosed) {
						t.Error(err)
					}
				})
			}
			copy.Close()
			wg.Wait()
			copy.Close()
			_, err = copy.DecodeMerged()
			if !errors.Is(err, ErrClosed) {
				t.Fatal(err)
			}
			if len(pixels) == 0 || floats.Data[0] != 1 {
				t.Fatal("output lifetime")
			}
		})
	}
}
func TestBuilder(t *testing.T) {
	input, err := os.ReadFile("../target/fixture.psd")
	ok(t, err)
	fixture, err := Parse(input, false)
	ok(t, err)
	defer fixture.Close()
	profile, err := fixture.ICCProfile()
	ok(t, err)
	for compression := Raw; compression <= ZIPPrediction; compression++ {
		t.Run(string(rune('0'+compression)), func(t *testing.T) {
			config := DefaultConfig(1, 1)
			config.Compression = compression
			b, err := NewBuilder(config)
			ok(t, err)
			defer b.Close()
			pixel := []byte{255, 0, 0, 128}
			o := DefaultLayerOptions()
			o.MaskBounds = image.Rect(0, 0, 1, 1)
			ok(t, b.BeginGroup("Folder"))
			ok(t, b.AddLayer("Ink 🎨", image.Rect(2, -1, 3, 0), pixel, &o, []byte{128}))
			_, err = b.Build()
			wantError(t, err)
			ok(t, b.EndGroup())
			ok(t, b.Composite(pixel))
			ok(t, b.Resource(1060, []byte{1, 2}, []byte{255}))
			ok(t, b.Resolution(300))
			ok(t, b.ICCProfile(profile))
			built, err := b.Build()
			ok(t, err)
			data, err := built.ToBytes()
			ok(t, err)
			built.Close()
			doc, err := Parse(data, false)
			ok(t, err)
			defer doc.Close()
			header, err := doc.Header()
			ok(t, err)
			layer, err := doc.Layer(1)
			ok(t, err)
			if header.LayerCount != 3 || layer.Name != "Ink 🎨" || layer.Bounds != image.Rect(2, -1, 3, 0) {
				t.Fatal(header, layer)
			}
			emptyPixels, err := doc.LayerPixelsF32(0)
			ok(t, err)
			if len(emptyPixels.Data) != 0 {
				t.Fatal("empty float layer")
			}
			maskBounds, err := doc.ChannelBounds(1, -2)
			ok(t, err)
			if maskBounds != o.MaskBounds {
				t.Fatal("mask bounds")
			}
			thumb, err := doc.ThumbnailInfo()
			ok(t, err)
			if thumb != nil {
				t.Fatal("unexpected thumbnail")
			}
			preview, err := doc.LayerPreview(1, 1)
			ok(t, err)
			if !bytes.Equal(preview.Pix, pixel) {
				t.Fatal(preview.Pix)
			}
			preview, err = doc.Preview(1)
			ok(t, err)
			if !bytes.Equal(preview.Pix, pixel) {
				t.Fatal(preview.Pix)
			}
			channel, err := doc.DecodeChannel(1, -2)
			ok(t, err)
			if !bytes.Equal(channel, []byte{128}) {
				t.Fatal(channel)
			}
			resource, err := doc.Resource(1060)
			ok(t, err)
			if !bytes.Equal(resource, []byte{1, 2}) {
				t.Fatal(resource)
			}
			resource, err = doc.Resource(1005)
			ok(t, err)
			if len(resource) != 16 {
				t.Fatal("DPI resource")
			}
			resource, err = doc.ICCProfile()
			ok(t, err)
			if !bytes.Equal(resource, profile) {
				t.Fatal("ICC resource")
			}
			_, err = doc.Preview(0)
			wantError(t, err)
			wantError(t, b.AddLayer("bad", image.Rect(0, 0, 1, 1), nil, nil, nil))
			wantError(t, b.AddLayer("bad", image.Rect(0, 0, 1<<40, 1), pixel, nil, nil))
			b.Close()
			_, err = b.Build()
			if !errors.Is(err, ErrClosed) {
				t.Fatal(err)
			}
		})
	}
	config := DefaultConfig(1, 1)
	config.Depth = 16
	config.ColorMode = CMYK
	config.Version = 2
	b, err := NewBuilder(config)
	ok(t, err)
	defer b.Close()
	pixel := []byte{0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0xff, 0xff}
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Go(func() {
			if err := b.AddLayer("Ink", image.Rect(0, 0, 1, 1), pixel, nil, nil); err != nil {
				t.Error(err)
			}
		})
	}
	wg.Wait()
	ok(t, b.Composite(pixel))
	doc, err := b.Build()
	ok(t, err)
	defer doc.Close()
	header, err := doc.Header()
	ok(t, err)
	decoded, err := doc.DecodeMerged()
	ok(t, err)
	if header.Version != 2 || header.Depth != 16 || header.LayerCount != 8 || !bytes.Equal(decoded, []byte{0xed, 0xcb, 0xa9, 0x87, 0x65, 0x43, 0x21, 0x0f}) {
		t.Fatal(header, decoded)
	}
	ink, err := doc.MergedPixelsF32()
	ok(t, err)
	info, err := doc.Info()
	ok(t, err)
	if info.MergedHasAlpha || ink.ColorMode != CMYK || ink.Channels != 5 || math.Abs(float64(ink.Data[0])-0x1234/65535.0) > 0.000001 || ink.Data[4] != 1 {
		t.Fatal("CMYK float pixels")
	}
	channel, err := doc.DecodeChannel(0, 0)
	ok(t, err)
	if !bytes.Equal(channel, []byte{0xed, 0xcb}) {
		t.Fatal(channel)
	}
}
func TestErrors(t *testing.T) {
	_, err := Parse(nil, false)
	wantError(t, err)
	_, err = NewBuilder(DefaultConfig(0, 1))
	wantError(t, err)
	var doc Document
	_, err = doc.Header()
	if !errors.Is(err, ErrClosed) {
		t.Fatal(err)
	}
	doc.Close()
	var b Builder
	wantError(t, b.EndGroup())
	b.Close()
	config := DefaultConfig(1, 1)
	bptr, err := NewBuilder(config)
	ok(t, err)
	defer bptr.Close()
	empty, err := bptr.Build()
	ok(t, err)
	dpi, err := empty.Resolution()
	ok(t, err)
	empty.Close()
	if dpi != nil {
		t.Fatal("unexpected resolution")
	}
	_, err = doc.MergedPixelsF32()
	wantError(t, err)
	_, err = doc.EditLayer(0, nil, nil)
	wantError(t, err)
	wantError(t, bptr.Resolution(0))
	wantError(t, bptr.Composite(nil))
	o := DefaultLayerOptions()
	o.BlendMode = [4]byte{}
	// Unknown four-byte blend keys are retained by the core, not rejected by the wrapper.
	wantError(t, bptr.AddLayer("", image.Rect(0, 0, 1, 1), []byte{1}, &o, nil))
}

func TestMaskBoundsArePreserved(t *testing.T) {
	input, err := os.ReadFile("../target/fixtures/v1-d8-c0.psd")
	ok(t, err)
	at := bytes.Index(input, []byte{0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 1, 255, 2})
	if at < 0 {
		t.Fatal("missing fixture mask")
	}
	input[at+3] = 4
	doc, err := Parse(input, true)
	ok(t, err)
	defer doc.Close()
	mask, err := doc.LayerMask(1)
	ok(t, err)
	if mask == nil || mask.Bounds != (image.Rectangle{Min: image.Pt(0, 4), Max: image.Pt(1, 3)}) {
		t.Fatal("mask bounds were reordered", mask)
	}
}
