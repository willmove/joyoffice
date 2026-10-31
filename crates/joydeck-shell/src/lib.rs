#![forbid(unsafe_code)]
pub mod automation;
pub mod control;
pub mod diagnostics;
pub mod document;
pub mod edit;
pub mod export;
pub mod interop;
pub mod media;
pub mod output;
pub mod preferences;
pub mod storage;
pub mod video;
pub mod workspace;
pub use joydeck_engine::{authoring_demo, compare, design_commands};
pub use joydeck_formats::{
    chart_package, diagram_package, motion_package, pptx, pptx_write, table_pptx,
};
pub use joydeck_model::{design, diagram, geometry, review, show, table, text};
pub mod preview;
pub mod playback {
    pub use joydeck_platform::playback::*;
}
pub mod chart {
    pub use joydeck_model::chart::*;
    pub use joydeck_scene::chart::svg;
}
pub mod motion {
    pub use joydeck_scene::motion::*;
}
pub mod scene;
