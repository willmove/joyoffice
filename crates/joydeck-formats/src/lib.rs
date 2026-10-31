#![forbid(unsafe_code)]
pub use joydeck_model::{
    chart, design, diagram, geometry, identity, interop, media, motion, playback, review, show,
    table, text,
};
pub mod chart_package;
pub mod diagram_package;
pub mod document;
pub mod motion_package;
pub mod native;
pub mod pptx;
pub mod pptx_write;
pub mod table_pptx;
