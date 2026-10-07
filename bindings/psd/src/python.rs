use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyMemoryView};

use crate::{
    BlockInfo, Builder as AdapterBuilder, BuilderConfig, Document, HeaderInfo, KnotInfo, LayerMetadata, LayerOptions, Path as AdapterPath, PathInfo,
    ResolutionInfo, ResourceInfo, SliceInfo, Slices as AdapterSlices, SlicesInfo, SubpathInfo, ThumbnailInfo, TreeNode,
};

fn error(e: ::photocraft_psd::PsdError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn layer_options(options: Option<Bound<'_, PyDict>>) -> PyResult<LayerOptions> {
    let mut result = LayerOptions::default();
    if let Some(options) = options {
        for key in options.keys() {
            if !matches!(
                key.extract::<&str>()?,
                "blend_mode" | "opacity" | "fill_opacity" | "visible" | "clipping" | "mask_bounds" | "mask_default" | "mask_disabled"
            ) {
                return Err(PyValueError::new_err("unknown layer option"));
            }
        }
        macro_rules! field {
            ($name:ident) => {
                if let Some(value) = options.get_item(stringify!($name))? {
                    result.$name = value.extract()?;
                }
            };
        }
        field!(blend_mode);
        field!(opacity);
        field!(fill_opacity);
        field!(visible);
        field!(clipping);
        field!(mask_bounds);
        field!(mask_default);
        field!(mask_disabled);
    }
    Ok(result)
}

#[pyclass(name = "Path", module = "photocraft_psd", frozen)]
struct PsdPath(AdapterPath);
#[pymethods]
impl PsdPath {
    fn info(&self) -> PathInfo {
        self.0.info()
    }
    fn subpath_info(&self, index: usize) -> PyResult<SubpathInfo> {
        self.0.subpath_info(index).map_err(error)
    }
    fn knot(&self, subpath: usize, index: usize) -> PyResult<KnotInfo> {
        self.0.knot(subpath, index).map_err(error)
    }
}
#[pyclass(name = "Slices", module = "photocraft_psd", frozen)]
struct PsdSlices(AdapterSlices);
#[pymethods]
impl PsdSlices {
    fn info(&self) -> SlicesInfo {
        self.0.info()
    }
    fn group_name(&self) -> &str {
        self.0.group_name()
    }
    fn slice_info(&self, index: usize) -> PyResult<SliceInfo> {
        self.0.slice_info(index).map_err(error)
    }
    fn text(&self, index: usize, field: &str) -> PyResult<&str> {
        self.0.text(index, field).map_err(error)
    }
}
#[pyclass(name = "PsdBuilder", module = "photocraft_psd")]
struct PsdBuilder(AdapterBuilder);
#[pymethods]
impl PsdBuilder {
    #[new]
    #[pyo3(signature=(width,height,*,depth=8,color_mode=3,version=1,compression=1))]
    fn new(width: u32, height: u32, depth: u16, color_mode: u16, version: u16, compression: u16) -> PyResult<Self> {
        AdapterBuilder::new(BuilderConfig { width, height, depth, color_mode, version, compression }).map(Self).map_err(error)
    }
    #[pyo3(signature=(name,bounds,pixels,options=None,mask=None))]
    fn add_layer(
        &mut self,
        py: Python<'_>,
        name: &str,
        bounds: [i32; 4],
        pixels: &[u8],
        options: Option<Bound<'_, PyDict>>,
        mask: Option<&[u8]>,
    ) -> PyResult<()> {
        let options = layer_options(options)?;
        py.detach(|| self.0.add_layer(name, bounds, pixels, options, mask)).map_err(error)
    }
    fn composite(&mut self, py: Python<'_>, pixels: &[u8]) -> PyResult<()> {
        py.detach(|| self.0.composite(pixels)).map_err(error)
    }
    fn begin_group(&mut self, name: &str) -> PyResult<()> {
        self.0.begin_group(name).map_err(error)
    }
    fn end_group(&mut self) -> PyResult<()> {
        self.0.end_group().map_err(error)
    }
    #[pyo3(signature=(id,data,name=None))]
    fn resource(&mut self, py: Python<'_>, id: u16, data: &[u8], name: Option<&[u8]>) -> PyResult<()> {
        py.detach(|| self.0.resource(id, name.unwrap_or_default(), data)).map_err(error)
    }
    fn icc_profile(&mut self, py: Python<'_>, data: &[u8]) -> PyResult<()> {
        py.detach(|| self.0.resource(1039, &[], data)).map_err(error)
    }
    fn resolution(&mut self, dpi: f64) -> PyResult<()> {
        self.0.resolution(dpi).map_err(error)
    }
    fn build(&self, py: Python<'_>) -> PyResult<Psd> {
        py.detach(|| self.0.build()).map(Psd).map_err(error)
    }
}

#[pyclass(name = "Psd", module = "photocraft_psd", frozen)]
struct Psd(Document);

#[pyclass(name = "FloatImage", module = "photocraft_psd", frozen)]
struct FloatImage(crate::FloatImage);
#[pymethods]
impl FloatImage {
    #[getter]
    fn width(&self) -> u32 {
        self.0.width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.0.height
    }
    #[getter]
    fn color_mode(&self) -> u16 {
        self.0.color_mode
    }
    #[getter]
    fn channels(&self) -> u16 {
        self.0.channels
    }
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let bytes = PyBytes::new_with(py, self.0.data.len() * 4, |out| {
            for (out, value) in out.as_chunks_mut::<4>().0.iter_mut().zip(&self.0.data) {
                out.copy_from_slice(&value.to_ne_bytes());
            }
            Ok(())
        })?;
        PyMemoryView::from(&bytes)?.call_method1("cast", ("f",))
    }
}

#[pymethods]
impl Psd {
    #[new]
    #[pyo3(signature = (bytes, *, lazy=false))]
    fn new(py: Python<'_>, bytes: &[u8], lazy: bool) -> PyResult<Self> {
        py.detach(|| if lazy { Document::parse_lazy(bytes) } else { Document::parse(bytes) }).map(Self).map_err(error)
    }

    #[getter]
    fn width(&self) -> u32 {
        self.0.header().width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.0.header().height
    }
    #[getter]
    fn version(&self) -> u16 {
        self.0.header().version
    }
    #[getter]
    fn channels(&self) -> u16 {
        self.0.header().channels
    }
    #[getter]
    fn depth(&self) -> u16 {
        self.0.header().depth
    }
    #[getter]
    fn color_mode(&self) -> u16 {
        self.0.header().color_mode
    }
    #[getter]
    fn layer_count(&self) -> usize {
        self.0.header().layer_count
    }

    fn layer_name(&self, layer: usize) -> PyResult<String> {
        self.0.layer_name(layer).map_err(error)
    }

    fn layer_bounds(&self, layer: usize) -> PyResult<[i32; 4]> {
        let i = self.0.layer_info(layer).map_err(error)?;
        Ok([i.top, i.left, i.bottom, i.right])
    }

    fn layer_channel_count(&self, layer: usize) -> PyResult<usize> {
        Ok(self.0.layer_info(layer).map_err(error)?.channel_count)
    }

    fn channel_id(&self, layer: usize, channel: usize) -> PyResult<i16> {
        self.0.channel_id(layer, channel).map_err(error)
    }

    fn channel_bounds(&self, layer: usize, id: i16) -> PyResult<[i32; 4]> {
        self.0.channel_bounds(layer, id).map_err(error)
    }

    fn decode_merged<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        py.detach(|| self.0.decode_merged()).map(|bytes| PyBytes::new(py, &bytes)).map_err(error)
    }

    fn decode_channel<'py>(&self, py: Python<'py>, layer: usize, id: i16) -> PyResult<Bound<'py, PyBytes>> {
        py.detach(|| self.0.decode_channel(layer, id)).map(|bytes| PyBytes::new(py, &bytes)).map_err(error)
    }

    fn icc_profile<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.0.icc_profile())
    }

    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        py.detach(|| self.0.to_bytes()).map(|bytes| PyBytes::new(py, &bytes)).map_err(error)
    }
    #[pyo3(signature = (layer, *, name=None, visible=None))]
    fn edit_layer(&self, py: Python<'_>, layer: usize, name: Option<&str>, visible: Option<bool>) -> PyResult<Self> {
        py.detach(|| self.0.edit_layer(layer, name, visible)).map(Self).map_err(error)
    }
    fn replace_channel(&self, py: Python<'_>, layer: usize, id: i16, data: &[u8]) -> PyResult<Self> {
        py.detach(|| self.0.replace_channel(layer, id, data)).map(Self).map_err(error)
    }
    fn merged_pixels_f32(&self, py: Python<'_>) -> PyResult<FloatImage> {
        py.detach(|| self.0.merged_pixels_f32()).map(FloatImage).map_err(error)
    }
    fn layer_pixels_f32(&self, py: Python<'_>, layer: usize) -> PyResult<FloatImage> {
        py.detach(|| self.0.layer_pixels_f32(layer)).map(FloatImage).map_err(error)
    }

    fn header_info(&self) -> HeaderInfo {
        self.0.header_info()
    }
    fn layer_metadata(&self, layer: usize) -> PyResult<LayerMetadata> {
        self.0.layer_metadata(layer).map_err(error)
    }
    fn layer_tree(&self, py: Python<'_>) -> PyResult<Vec<TreeNode>> {
        py.detach(|| self.0.layer_tree()).map_err(error)
    }
    fn channel_compression(&self, layer: usize, id: i16) -> PyResult<Option<u16>> {
        self.0.channel_compression(layer, id).map_err(error)
    }
    fn resolution(&self) -> Option<ResolutionInfo> {
        self.0.resolution()
    }
    fn color_mode_data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.0.color_mode_data())
    }
    fn resource<'py>(&self, py: Python<'py>, id: u16) -> Option<Bound<'py, PyBytes>> {
        self.0.resource(id).map(|bytes| PyBytes::new(py, bytes))
    }
    fn resource_path(&self, py: Python<'_>, index: usize) -> PyResult<Option<PsdPath>> {
        py.detach(|| self.0.resource_path(index)).map(|p| p.map(PsdPath)).map_err(error)
    }
    fn vector_mask(&self, py: Python<'_>, layer: usize) -> PyResult<Option<PsdPath>> {
        py.detach(|| self.0.vector_mask(layer)).map(|p| p.map(PsdPath)).map_err(error)
    }
    fn slices(&self, py: Python<'_>) -> PyResult<Option<PsdSlices>> {
        py.detach(|| self.0.slices()).map(|s| s.map(PsdSlices)).map_err(error)
    }
    fn resource_count(&self) -> usize {
        self.0.resource_count()
    }
    fn resource_info(&self, index: usize) -> PyResult<ResourceInfo> {
        self.0.resource_info(index).map_err(error)
    }
    fn resource_name<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyBytes>> {
        self.0.resource_name(index).map(|data| PyBytes::new(py, data)).map_err(error)
    }
    fn resource_data<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyBytes>> {
        self.0.resource_data(index).map(|data| PyBytes::new(py, data)).map_err(error)
    }
    fn layer_block_count(&self, layer: usize) -> PyResult<usize> {
        self.0.layer_block_count(layer).map_err(error)
    }
    fn global_block_count(&self) -> usize {
        self.0.global_block_count()
    }
    fn layer_block_info(&self, layer: usize, index: usize) -> PyResult<BlockInfo> {
        self.0.layer_block_info(layer, index).map_err(error)
    }
    fn global_block_info(&self, index: usize) -> PyResult<BlockInfo> {
        self.0.global_block_info(index).map_err(error)
    }
    fn layer_block_data<'py>(&self, py: Python<'py>, layer: usize, index: usize) -> PyResult<Bound<'py, PyBytes>> {
        self.0.layer_block_data(layer, index).map(|data| PyBytes::new(py, data)).map_err(error)
    }
    fn global_block_data<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyBytes>> {
        self.0.global_block_data(index).map(|data| PyBytes::new(py, data)).map_err(error)
    }
    fn thumbnail_info(&self) -> PyResult<Option<ThumbnailInfo>> {
        self.0.thumbnail_info().map_err(error)
    }
    fn thumbnail_data<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyBytes>>> {
        self.0.thumbnail_data().map(|data| data.map(|data| PyBytes::new(py, data))).map_err(error)
    }
    fn preview<'py>(&self, py: Python<'py>, max_edge: u32) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
        py.detach(|| self.0.preview(max_edge)).map(|p| (p.width, p.height, PyBytes::new(py, &p.data))).map_err(error)
    }
    fn layer_preview<'py>(&self, py: Python<'py>, layer: usize, max_edge: u32) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
        py.detach(|| self.0.layer_preview(layer, max_edge)).map(|p| (p.width, p.height, PyBytes::new(py, &p.data))).map_err(error)
    }
    fn merged_rgba8<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        py.detach(|| self.0.merged_rgba8()).map(|bytes| PyBytes::new(py, &bytes)).map_err(error)
    }
    fn layer_rgba8<'py>(&self, py: Python<'py>, layer: usize) -> PyResult<Bound<'py, PyBytes>> {
        py.detach(|| self.0.layer_rgba8(layer)).map(|bytes| PyBytes::new(py, &bytes)).map_err(error)
    }
}

#[pymodule(gil_used = false)]
fn photocraft_psd(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Psd>()?;
    module.add_class::<FloatImage>()?;
    module.add_class::<PsdPath>()?;
    module.add_class::<PsdSlices>()?;
    module.add_class::<PsdBuilder>()?;
    for (name, value) in [
        ("RAW", 0),
        ("RLE", 1),
        ("ZIP", 2),
        ("ZIP_PREDICTION", 3),
        ("BITMAP", 0),
        ("GRAYSCALE", 1),
        ("INDEXED", 2),
        ("RGB", 3),
        ("CMYK", 4),
        ("MULTICHANNEL", 7),
        ("DUOTONE", 8),
        ("LAB", 9),
    ] {
        module.add(name, value)?;
    }
    Ok(())
}
