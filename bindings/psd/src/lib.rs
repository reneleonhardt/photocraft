//! Language adapters for PSD/PSB inspection, decoding and creation.
#![deny(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod builder;
mod document;
mod edit;
mod geometry;
mod metadata;
mod pixels;
mod preview;
mod resources;
pub use builder::{Builder, BuilderConfig, LayerOptions};
pub use document::{Document, Header, LayerInfo};
pub use geometry::{Path, Slices};
pub use metadata::*;
pub use pixels::FloatImage;
