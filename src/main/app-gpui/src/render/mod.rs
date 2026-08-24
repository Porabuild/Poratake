//! The shared rasterizer. The image editor's export and the video editor's
//! composition are both ports of renderer code written against a 2D canvas, so
//! they draw through one surface (`canvas`) and one annotation renderer
//! (`annotations`) rather than each growing its own.

pub mod annotations;
pub mod blur;
pub mod canvas;
pub mod color;
pub mod color_detection;
pub mod freehand;
pub mod gradient;
pub mod text;
pub mod window_frame;
