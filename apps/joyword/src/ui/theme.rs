//! Word's brand binding to the shared JoyOffice visual system.
pub use joyoffice_ui::metrics;
pub const PALETTE: joyoffice_ui::Palette = joyoffice_ui::Palette::light(joyoffice_ui::Brand::Word);
pub const ACCENT: u32 = PALETTE.accent;
pub const ACCENT_DARK: u32 = PALETTE.accent_hover;
pub const TEXT: u32 = PALETTE.text;
pub const TEXT_MUTED: u32 = PALETTE.muted;
pub const TEXT_DISABLED: u32 = PALETTE.disabled;
pub const WHITE: u32 = PALETTE.surface;
pub const HEADER: u32 = PALETTE.header;
pub const RIBBON: u32 = PALETTE.chrome;
pub const CANVAS: u32 = PALETTE.canvas;
pub const BORDER: u32 = PALETTE.border;
pub const BORDER_LIGHT: u32 = PALETTE.separator;
pub const HOVER: u32 = PALETTE.hover;
pub const PRESSED: u32 = PALETTE.pressed;
pub const SELECTED: u32 = PALETTE.selected;
pub const NOTICE: u32 = 0xfff4ce;

pub const CONTROL_HEIGHT: f32 = metrics::CONTROL;
pub const TAB_HEIGHT: f32 = metrics::TAB;
pub const STATUS_HEIGHT: f32 = metrics::STATUS;
pub const RULER_HEIGHT: f32 = 28.0;
