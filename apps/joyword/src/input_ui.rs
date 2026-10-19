//! GPUI adapter for the platform-neutral input state. Geometry is derived
//! only from the current layout and the live scroll offset, never an old
//! scene; it does not wait for that layout's pixels.
use crate::*;
use gpui::{
    ElementInputHandler, EntityInputHandler, MouseButton, Pixels, Point, UTF16Selection, canvas,
    fill, point, rgba,
};
use joyword_engine::Selection;
use joyword_foundation::Emu;
use joyword_model::{Bias, Position};
use joyword_shell::docx::BoundDocx;
use joyword_shell::input::{Direction, InputError, InputSource, InputView, TextInput};
use joyword_shell::storage::SaveCopyReceipt;
use std::ops::Range;

pub const DOC_PAD: f32 = 32.0;
pub const DOC_GAP: f32 = 32.0;

fn table_axis(
    args: &std::collections::BTreeMap<&'static str, joyword_shell::command::ArgValue>,
    name: &'static str,
) -> Result<u32, String> {
    match args.get(name) {
        None => Ok(2),
        Some(joyword_shell::command::ArgValue::Text(text)) => {
            text.parse().map_err(|_| name.to_string())
        }
        Some(_) => Err(name.to_string()),
    }
}

/// The last geometry handed to the platform. Comparing this avoids an
/// endless next-frame loop while refreshing IME coordinates after async layout
/// and scrolling, including scrolls whose offscreen pages are not repainted.
#[derive(Clone, PartialEq)]
pub(crate) struct ImeGeometry {
    key: LayoutRequestKey,
    zoom: u16,
    viewport: Bounds<Pixels>,
    scroll: Point<Pixels>,
    selection: Selection,
}

/// A prompt response applies only to the document version the user saw.
#[derive(Debug, PartialEq, Eq)]
enum CloseDecision {
    Cancel,
    Discard,
    AskAgain,
}
pub(crate) trait DocEditor {
    fn has_unsaved_changes(&self) -> bool;
    fn is_composing(&self) -> bool;
    fn version(&self) -> u64;
    fn view(&self) -> &joyword_shell::input::InputView;
    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;
    fn read_only(&self) -> bool;
    fn undo(&mut self) -> Result<(), InputError>;
    fn redo(&mut self) -> Result<(), InputError>;
    fn select_all(&mut self) -> Result<(), InputError>;
    fn delete(&mut self, direction: Direction) -> Result<bool, InputError>;
    /// Keyboard text from the platform input client.
    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Result<bool, InputError> {
        self.replace_from(InputSource::Keyboard, range, text)
    }
    fn replace_from(
        &mut self,
        source: InputSource,
        range: Option<Range<usize>>,
        text: &str,
    ) -> Result<bool, InputError>;
    fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Result<(), InputError>;
    fn cancel_composition(&mut self) -> Result<bool, InputError>;
    fn unmark(&mut self) -> Result<bool, InputError>;
    fn move_grapheme(&mut self, direction: Direction, extend: bool) -> Result<(), InputError>;
    fn select_positions(&mut self, selection: Selection) -> Result<(), InputError>;
    fn select_word_at(&mut self, offset: usize) -> Result<(), InputError>;
    fn select_paragraph_at(&mut self, offset: usize) -> Result<(), InputError>;
    fn source_path(&self) -> Option<&std::path::Path>;
    /// Save As is possible whenever the editor can produce a verified export,
    /// including package sessions never bound to a path.
    fn can_save_copy(&self) -> bool {
        self.source_path().is_some()
    }
    fn save_copy(&self, destination: &std::path::Path) -> Result<SaveCopyReceipt, String>;
    fn save_copy_job(&self) -> Option<joyword_shell::docx::SaveCopyJob> {
        None
    }
    fn save_original_job(&self) -> Option<joyword_shell::docx::SaveOriginalJob> {
        None
    }
    fn apply_original_save(
        &mut self,
        _output: joyword_shell::docx::SaveOriginalJobOutput,
    ) -> Result<joyword_shell::storage::SaveOriginalReceipt, String> {
        Err("file.save_original.unavailable".into())
    }
    fn format_runs(&mut self, change: joyword_engine::RunChange) -> Result<bool, InputError>;
    fn list_styles(&self) -> Vec<String>;
    fn apply_style(&mut self, style: &str) -> Result<bool, InputError>;
    fn apply_list(&mut self, num_id: u32) -> Result<bool, InputError>;
    fn set_list_level(&mut self, level: u8) -> Result<bool, InputError>;
    fn remove_list(&mut self) -> Result<bool, InputError>;
    fn insert_page_break(&mut self) -> Result<bool, InputError>;
    fn set_page_size(&mut self, width: i64, height: i64) -> Result<bool, InputError>;
    fn set_page_margins(
        &mut self,
        top: i64,
        right: i64,
        bottom: i64,
        left: i64,
    ) -> Result<bool, InputError>;
    fn set_page_orientation(&mut self, landscape: bool) -> Result<bool, InputError>;
    fn insert_table(&mut self, rows: u32, columns: u32) -> Result<bool, InputError>;
    fn insert_image_bytes(&mut self, bytes: &[u8]) -> Result<bool, InputError>;
    fn insert_image_file(&mut self, path: &std::path::Path) -> Result<bool, InputError>;
    fn paste_rich(
        &mut self,
        document: &joyword_shell::rich_paste::RichDocument,
    ) -> Result<bool, InputError>;
    fn selected_image_bytes(&self) -> Option<std::sync::Arc<[u8]>>;
    fn openable_link(&self, position: &Position) -> Option<&str>;
    fn insert_table_row(&mut self) -> Result<bool, InputError>;
    fn delete_table_row(&mut self) -> Result<bool, InputError>;
    fn insert_table_column(&mut self) -> Result<bool, InputError>;
    fn delete_table_column(&mut self) -> Result<bool, InputError>;
    fn select_table_cell(&mut self) -> Result<(), InputError>;
    fn format_paragraph(
        &mut self,
        change: joyword_engine::ParagraphChange,
    ) -> Result<bool, InputError>;
    fn editing(
        &mut self,
        request: joyword_shell::input::EditRequest,
    ) -> Result<joyword_shell::input::EditReport, InputError>;
    fn style_rows(&self) -> Vec<joyword_shell::input::StyleRow>;
    fn caret_styles(&self) -> (Option<String>, Option<String>);
    fn acknowledge_layout(&mut self, version: u64);
    fn selected_text(&self) -> Result<String, InputError>;
    fn recovery_baseline(&self) -> Option<&[u8]> {
        None
    }
    fn into_bound(self: Box<Self>) -> Option<TextInput<BoundDocx>> {
        None
    }
}

impl DocEditor for TextInput<joyword_engine::Session> {
    fn has_unsaved_changes(&self) -> bool {
        TextInput::has_unsaved_changes(self)
    }
    fn is_composing(&self) -> bool {
        TextInput::is_composing(self)
    }
    fn version(&self) -> u64 {
        self.backend().version().get()
    }
    fn view(&self) -> &joyword_shell::input::InputView {
        TextInput::view(self)
    }
    fn can_undo(&self) -> bool {
        TextInput::can_undo(self)
    }
    fn can_redo(&self) -> bool {
        TextInput::can_redo(self)
    }
    fn read_only(&self) -> bool {
        TextInput::read_only(self)
    }
    fn undo(&mut self) -> Result<(), InputError> {
        TextInput::undo(self)
    }
    fn redo(&mut self) -> Result<(), InputError> {
        TextInput::redo(self)
    }
    fn select_all(&mut self) -> Result<(), InputError> {
        TextInput::select_all(self)
    }
    fn delete(&mut self, direction: Direction) -> Result<bool, InputError> {
        TextInput::delete(self, direction)
    }
    fn replace_from(
        &mut self,
        source: InputSource,
        range: Option<Range<usize>>,
        text: &str,
    ) -> Result<bool, InputError> {
        TextInput::replace_from(self, source, range, text)
    }
    fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Result<(), InputError> {
        TextInput::mark(self, range, text, selected)
    }
    fn cancel_composition(&mut self) -> Result<bool, InputError> {
        TextInput::cancel_composition(self)
    }
    fn unmark(&mut self) -> Result<bool, InputError> {
        TextInput::unmark(self)
    }
    fn move_grapheme(&mut self, direction: Direction, extend: bool) -> Result<(), InputError> {
        TextInput::move_grapheme(self, direction, extend)
    }
    fn select_positions(&mut self, selection: Selection) -> Result<(), InputError> {
        TextInput::select_positions(self, selection)
    }
    fn select_word_at(&mut self, offset: usize) -> Result<(), InputError> {
        TextInput::select_word_at(self, offset)
    }
    fn select_paragraph_at(&mut self, offset: usize) -> Result<(), InputError> {
        TextInput::select_paragraph_at(self, offset)
    }
    fn source_path(&self) -> Option<&std::path::Path> {
        None
    }
    fn save_copy(&self, _destination: &std::path::Path) -> Result<SaveCopyReceipt, String> {
        Err("file.save_copy.unavailable".into())
    }
    fn format_runs(&mut self, change: joyword_engine::RunChange) -> Result<bool, InputError> {
        TextInput::format_runs(self, change)
    }
    fn list_styles(&self) -> Vec<String> {
        TextInput::list_styles(self)
    }
    fn apply_style(&mut self, style: &str) -> Result<bool, InputError> {
        TextInput::apply_style(self, style)
    }
    fn apply_list(&mut self, num_id: u32) -> Result<bool, InputError> {
        TextInput::apply_list(self, num_id)
    }
    fn set_list_level(&mut self, level: u8) -> Result<bool, InputError> {
        TextInput::set_list_level(self, level)
    }
    fn remove_list(&mut self) -> Result<bool, InputError> {
        TextInput::remove_list(self)
    }
    fn insert_page_break(&mut self) -> Result<bool, InputError> {
        TextInput::insert_page_break(self)
    }
    fn set_page_size(&mut self, width: i64, height: i64) -> Result<bool, InputError> {
        TextInput::set_page_size(self, width, height)
    }
    fn set_page_margins(
        &mut self,
        top: i64,
        right: i64,
        bottom: i64,
        left: i64,
    ) -> Result<bool, InputError> {
        TextInput::set_page_margins(self, top, right, bottom, left)
    }
    fn set_page_orientation(&mut self, landscape: bool) -> Result<bool, InputError> {
        TextInput::set_page_orientation(self, landscape)
    }
    fn insert_table(&mut self, rows: u32, columns: u32) -> Result<bool, InputError> {
        TextInput::insert_table(self, rows, columns)
    }
    fn insert_image_bytes(&mut self, bytes: &[u8]) -> Result<bool, InputError> {
        TextInput::insert_image_bytes(self, bytes)
    }
    fn insert_image_file(&mut self, path: &std::path::Path) -> Result<bool, InputError> {
        TextInput::insert_image_file(self, path)
    }
    fn paste_rich(
        &mut self,
        document: &joyword_shell::rich_paste::RichDocument,
    ) -> Result<bool, InputError> {
        TextInput::paste_rich(self, document)
    }
    fn selected_image_bytes(&self) -> Option<std::sync::Arc<[u8]>> {
        TextInput::selected_image_bytes(self)
    }
    fn openable_link(&self, position: &Position) -> Option<&str> {
        TextInput::openable_link(self, position)
    }
    fn insert_table_row(&mut self) -> Result<bool, InputError> {
        TextInput::insert_table_row(self)
    }
    fn delete_table_row(&mut self) -> Result<bool, InputError> {
        TextInput::delete_table_row(self)
    }
    fn insert_table_column(&mut self) -> Result<bool, InputError> {
        TextInput::insert_table_column(self)
    }
    fn delete_table_column(&mut self) -> Result<bool, InputError> {
        TextInput::delete_table_column(self)
    }
    fn select_table_cell(&mut self) -> Result<(), InputError> {
        TextInput::select_table_cell(self)
    }
    fn format_paragraph(
        &mut self,
        change: joyword_engine::ParagraphChange,
    ) -> Result<bool, InputError> {
        TextInput::format_paragraph(self, change)
    }
    fn editing(
        &mut self,
        request: joyword_shell::input::EditRequest,
    ) -> Result<joyword_shell::input::EditReport, InputError> {
        TextInput::editing(self, request)
    }
    fn style_rows(&self) -> Vec<joyword_shell::input::StyleRow> {
        TextInput::style_rows(self)
    }
    fn caret_styles(&self) -> (Option<String>, Option<String>) {
        TextInput::caret_styles(self)
    }
    fn acknowledge_layout(&mut self, version: u64) {
        TextInput::acknowledge_layout(self, version);
    }
    fn selected_text(&self) -> Result<String, InputError> {
        TextInput::selected_text(self)
    }
}

impl DocEditor for TextInput<BoundDocx> {
    fn has_unsaved_changes(&self) -> bool {
        TextInput::has_unsaved_changes(self)
    }
    fn is_composing(&self) -> bool {
        TextInput::is_composing(self)
    }
    fn version(&self) -> u64 {
        self.backend().session().session().version().get()
    }
    fn view(&self) -> &joyword_shell::input::InputView {
        TextInput::view(self)
    }
    fn can_undo(&self) -> bool {
        TextInput::can_undo(self)
    }
    fn can_redo(&self) -> bool {
        TextInput::can_redo(self)
    }
    fn read_only(&self) -> bool {
        TextInput::read_only(self)
    }
    fn undo(&mut self) -> Result<(), InputError> {
        TextInput::undo(self)
    }
    fn redo(&mut self) -> Result<(), InputError> {
        TextInput::redo(self)
    }
    fn select_all(&mut self) -> Result<(), InputError> {
        TextInput::select_all(self)
    }
    fn delete(&mut self, direction: Direction) -> Result<bool, InputError> {
        TextInput::delete(self, direction)
    }
    fn replace_from(
        &mut self,
        source: InputSource,
        range: Option<Range<usize>>,
        text: &str,
    ) -> Result<bool, InputError> {
        TextInput::replace_from(self, source, range, text)
    }
    fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Result<(), InputError> {
        TextInput::mark(self, range, text, selected)
    }
    fn cancel_composition(&mut self) -> Result<bool, InputError> {
        TextInput::cancel_composition(self)
    }
    fn unmark(&mut self) -> Result<bool, InputError> {
        TextInput::unmark(self)
    }
    fn move_grapheme(&mut self, direction: Direction, extend: bool) -> Result<(), InputError> {
        TextInput::move_grapheme(self, direction, extend)
    }
    fn select_positions(&mut self, selection: Selection) -> Result<(), InputError> {
        TextInput::select_positions(self, selection)
    }
    fn select_word_at(&mut self, offset: usize) -> Result<(), InputError> {
        TextInput::select_word_at(self, offset)
    }
    fn select_paragraph_at(&mut self, offset: usize) -> Result<(), InputError> {
        TextInput::select_paragraph_at(self, offset)
    }
    fn source_path(&self) -> Option<&std::path::Path> {
        Some(self.backend().source_path())
    }
    fn save_copy(&self, destination: &std::path::Path) -> Result<SaveCopyReceipt, String> {
        self.backend()
            .save_copy(destination)
            .map_err(|error| error.to_string())
    }
    fn save_copy_job(&self) -> Option<joyword_shell::docx::SaveCopyJob> {
        Some(self.backend().save_copy_job())
    }
    fn save_original_job(&self) -> Option<joyword_shell::docx::SaveOriginalJob> {
        Some(TextInput::save_original_job(self))
    }
    fn apply_original_save(
        &mut self,
        output: joyword_shell::docx::SaveOriginalJobOutput,
    ) -> Result<joyword_shell::storage::SaveOriginalReceipt, String> {
        TextInput::apply_original_save(self, output).map_err(|error| error.to_string())
    }
    fn format_runs(&mut self, change: joyword_engine::RunChange) -> Result<bool, InputError> {
        TextInput::format_runs(self, change)
    }
    fn list_styles(&self) -> Vec<String> {
        TextInput::list_styles(self)
    }
    fn apply_style(&mut self, style: &str) -> Result<bool, InputError> {
        TextInput::apply_style(self, style)
    }
    fn apply_list(&mut self, num_id: u32) -> Result<bool, InputError> {
        TextInput::apply_list(self, num_id)
    }
    fn set_list_level(&mut self, level: u8) -> Result<bool, InputError> {
        TextInput::set_list_level(self, level)
    }
    fn remove_list(&mut self) -> Result<bool, InputError> {
        TextInput::remove_list(self)
    }
    fn insert_page_break(&mut self) -> Result<bool, InputError> {
        TextInput::insert_page_break(self)
    }
    fn set_page_size(&mut self, width: i64, height: i64) -> Result<bool, InputError> {
        TextInput::set_page_size(self, width, height)
    }
    fn set_page_margins(
        &mut self,
        top: i64,
        right: i64,
        bottom: i64,
        left: i64,
    ) -> Result<bool, InputError> {
        TextInput::set_page_margins(self, top, right, bottom, left)
    }
    fn set_page_orientation(&mut self, landscape: bool) -> Result<bool, InputError> {
        TextInput::set_page_orientation(self, landscape)
    }
    fn insert_table(&mut self, rows: u32, columns: u32) -> Result<bool, InputError> {
        TextInput::insert_table(self, rows, columns)
    }
    fn insert_image_bytes(&mut self, bytes: &[u8]) -> Result<bool, InputError> {
        TextInput::insert_image_bytes(self, bytes)
    }
    fn insert_image_file(&mut self, path: &std::path::Path) -> Result<bool, InputError> {
        TextInput::insert_image_file(self, path)
    }
    fn paste_rich(
        &mut self,
        document: &joyword_shell::rich_paste::RichDocument,
    ) -> Result<bool, InputError> {
        TextInput::paste_rich(self, document)
    }
    fn selected_image_bytes(&self) -> Option<std::sync::Arc<[u8]>> {
        TextInput::selected_image_bytes(self)
    }
    fn openable_link(&self, position: &Position) -> Option<&str> {
        TextInput::openable_link(self, position)
    }
    fn insert_table_row(&mut self) -> Result<bool, InputError> {
        TextInput::insert_table_row(self)
    }
    fn delete_table_row(&mut self) -> Result<bool, InputError> {
        TextInput::delete_table_row(self)
    }
    fn insert_table_column(&mut self) -> Result<bool, InputError> {
        TextInput::insert_table_column(self)
    }
    fn delete_table_column(&mut self) -> Result<bool, InputError> {
        TextInput::delete_table_column(self)
    }
    fn select_table_cell(&mut self) -> Result<(), InputError> {
        TextInput::select_table_cell(self)
    }
    fn format_paragraph(
        &mut self,
        change: joyword_engine::ParagraphChange,
    ) -> Result<bool, InputError> {
        TextInput::format_paragraph(self, change)
    }
    fn editing(
        &mut self,
        request: joyword_shell::input::EditRequest,
    ) -> Result<joyword_shell::input::EditReport, InputError> {
        TextInput::editing(self, request)
    }
    fn style_rows(&self) -> Vec<joyword_shell::input::StyleRow> {
        TextInput::style_rows(self)
    }
    fn caret_styles(&self) -> (Option<String>, Option<String>) {
        TextInput::caret_styles(self)
    }
    fn acknowledge_layout(&mut self, version: u64) {
        TextInput::acknowledge_layout(self, version);
    }
    fn selected_text(&self) -> Result<String, InputError> {
        TextInput::selected_text(self)
    }
    fn recovery_baseline(&self) -> Option<&[u8]> {
        Some(self.backend().session().backing().original_bytes())
    }
    fn into_bound(self: Box<Self>) -> Option<TextInput<BoundDocx>> {
        Some(*self)
    }
}

/// A package session without a bound path (the startup sample): fully editable
/// and exportable, but it has no source file to suggest or return to.
impl DocEditor for TextInput<joyword_shell::docx::DocxSession> {
    fn has_unsaved_changes(&self) -> bool {
        TextInput::has_unsaved_changes(self)
    }
    fn is_composing(&self) -> bool {
        TextInput::is_composing(self)
    }
    fn version(&self) -> u64 {
        self.backend().session().version().get()
    }
    fn view(&self) -> &joyword_shell::input::InputView {
        TextInput::view(self)
    }
    fn can_undo(&self) -> bool {
        TextInput::can_undo(self)
    }
    fn can_redo(&self) -> bool {
        TextInput::can_redo(self)
    }
    fn read_only(&self) -> bool {
        TextInput::read_only(self)
    }
    fn undo(&mut self) -> Result<(), InputError> {
        TextInput::undo(self)
    }
    fn redo(&mut self) -> Result<(), InputError> {
        TextInput::redo(self)
    }
    fn select_all(&mut self) -> Result<(), InputError> {
        TextInput::select_all(self)
    }
    fn delete(&mut self, direction: Direction) -> Result<bool, InputError> {
        TextInput::delete(self, direction)
    }
    fn replace_from(
        &mut self,
        source: InputSource,
        range: Option<Range<usize>>,
        text: &str,
    ) -> Result<bool, InputError> {
        TextInput::replace_from(self, source, range, text)
    }
    fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Result<(), InputError> {
        TextInput::mark(self, range, text, selected)
    }
    fn cancel_composition(&mut self) -> Result<bool, InputError> {
        TextInput::cancel_composition(self)
    }
    fn unmark(&mut self) -> Result<bool, InputError> {
        TextInput::unmark(self)
    }
    fn move_grapheme(&mut self, direction: Direction, extend: bool) -> Result<(), InputError> {
        TextInput::move_grapheme(self, direction, extend)
    }
    fn select_positions(&mut self, selection: Selection) -> Result<(), InputError> {
        TextInput::select_positions(self, selection)
    }
    fn select_word_at(&mut self, offset: usize) -> Result<(), InputError> {
        TextInput::select_word_at(self, offset)
    }
    fn select_paragraph_at(&mut self, offset: usize) -> Result<(), InputError> {
        TextInput::select_paragraph_at(self, offset)
    }
    fn source_path(&self) -> Option<&std::path::Path> {
        None
    }
    fn can_save_copy(&self) -> bool {
        true
    }
    fn save_copy(&self, destination: &std::path::Path) -> Result<SaveCopyReceipt, String> {
        self.backend()
            .save_copy_job()
            .run(destination)
            .map_err(|error| error.to_string())
    }
    fn save_copy_job(&self) -> Option<joyword_shell::docx::SaveCopyJob> {
        Some(self.backend().save_copy_job())
    }
    fn format_runs(&mut self, change: joyword_engine::RunChange) -> Result<bool, InputError> {
        TextInput::format_runs(self, change)
    }
    fn list_styles(&self) -> Vec<String> {
        TextInput::list_styles(self)
    }
    fn apply_style(&mut self, style: &str) -> Result<bool, InputError> {
        TextInput::apply_style(self, style)
    }
    fn apply_list(&mut self, num_id: u32) -> Result<bool, InputError> {
        TextInput::apply_list(self, num_id)
    }
    fn set_list_level(&mut self, level: u8) -> Result<bool, InputError> {
        TextInput::set_list_level(self, level)
    }
    fn remove_list(&mut self) -> Result<bool, InputError> {
        TextInput::remove_list(self)
    }
    fn insert_page_break(&mut self) -> Result<bool, InputError> {
        TextInput::insert_page_break(self)
    }
    fn set_page_size(&mut self, width: i64, height: i64) -> Result<bool, InputError> {
        TextInput::set_page_size(self, width, height)
    }
    fn set_page_margins(
        &mut self,
        top: i64,
        right: i64,
        bottom: i64,
        left: i64,
    ) -> Result<bool, InputError> {
        TextInput::set_page_margins(self, top, right, bottom, left)
    }
    fn set_page_orientation(&mut self, landscape: bool) -> Result<bool, InputError> {
        TextInput::set_page_orientation(self, landscape)
    }
    fn insert_table(&mut self, rows: u32, columns: u32) -> Result<bool, InputError> {
        TextInput::insert_table(self, rows, columns)
    }
    fn insert_image_bytes(&mut self, bytes: &[u8]) -> Result<bool, InputError> {
        TextInput::insert_image_bytes(self, bytes)
    }
    fn insert_image_file(&mut self, path: &std::path::Path) -> Result<bool, InputError> {
        TextInput::insert_image_file(self, path)
    }
    fn paste_rich(
        &mut self,
        document: &joyword_shell::rich_paste::RichDocument,
    ) -> Result<bool, InputError> {
        TextInput::paste_rich(self, document)
    }
    fn selected_image_bytes(&self) -> Option<std::sync::Arc<[u8]>> {
        TextInput::selected_image_bytes(self)
    }
    fn openable_link(&self, position: &Position) -> Option<&str> {
        TextInput::openable_link(self, position)
    }
    fn insert_table_row(&mut self) -> Result<bool, InputError> {
        TextInput::insert_table_row(self)
    }
    fn delete_table_row(&mut self) -> Result<bool, InputError> {
        TextInput::delete_table_row(self)
    }
    fn insert_table_column(&mut self) -> Result<bool, InputError> {
        TextInput::insert_table_column(self)
    }
    fn delete_table_column(&mut self) -> Result<bool, InputError> {
        TextInput::delete_table_column(self)
    }
    fn select_table_cell(&mut self) -> Result<(), InputError> {
        TextInput::select_table_cell(self)
    }
    fn format_paragraph(
        &mut self,
        change: joyword_engine::ParagraphChange,
    ) -> Result<bool, InputError> {
        TextInput::format_paragraph(self, change)
    }
    fn editing(
        &mut self,
        request: joyword_shell::input::EditRequest,
    ) -> Result<joyword_shell::input::EditReport, InputError> {
        TextInput::editing(self, request)
    }
    fn style_rows(&self) -> Vec<joyword_shell::input::StyleRow> {
        TextInput::style_rows(self)
    }
    fn caret_styles(&self) -> (Option<String>, Option<String>) {
        TextInput::caret_styles(self)
    }
    fn acknowledge_layout(&mut self, version: u64) {
        TextInput::acknowledge_layout(self, version);
    }
    fn selected_text(&self) -> Result<String, InputError> {
        TextInput::selected_text(self)
    }
    fn recovery_baseline(&self) -> Option<&[u8]> {
        Some(self.backend().backing().original_bytes())
    }
}

/// A Save As copy in flight: the editor epoch it was taken from and the
/// committed version it writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CopyRequest {
    epoch: u64,
    version: u64,
}

/// One Save As copy per editor at a time: choosing the path, then writing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CopyPhase {
    Picking(u64),
    Writing(CopyRequest),
}

impl CopyPhase {
    fn epoch(self) -> u64 {
        match self {
            Self::Picking(epoch) => epoch,
            Self::Writing(request) => request.epoch,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OriginalRequest {
    epoch: u64,
    version: u64,
    document: Option<joyword_shell::host::DocumentId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OriginalSavePhase {
    Probing(OriginalRequest),
    Prompting(OriginalRequest),
    Writing(OriginalRequest),
}

impl OriginalSavePhase {
    fn epoch(self) -> u64 {
        match self {
            Self::Probing(request) | Self::Prompting(request) | Self::Writing(request) => {
                request.epoch
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OriginalDecision {
    Cancel,
    SaveCopy,
    Confirm,
    Reprobe,
}

fn original_decision(answer: Option<usize>, expected: u64, current: u64) -> OriginalDecision {
    match answer {
        Some(1) => OriginalDecision::SaveCopy,
        Some(2) if expected == current => OriginalDecision::Confirm,
        Some(2) => OriginalDecision::Reprobe,
        _ => OriginalDecision::Cancel,
    }
}

fn original_metadata_key(item: joyword_shell::storage::MetadataItem) -> &'static str {
    use joyword_shell::storage::MetadataItem;

    match item {
        MetadataItem::Permissions => "metadata.permissions",
        MetadataItem::Ownership => "metadata.ownership",
        MetadataItem::Acl => "metadata.acl",
        MetadataItem::ExtendedAttributes => "metadata.extended_attributes",
        MetadataItem::CreationTime => "metadata.creation_time",
        MetadataItem::Links => "metadata.links",
        MetadataItem::ProvenanceMarks => "metadata.provenance_marks",
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CopyLanding {
    /// Another document is installed, or none: nothing of it changes.
    Stale,
    Current,
    /// Edits made after the request are not in the copy and stay unsaved.
    Earlier,
}

fn copy_landing(request: CopyRequest, current: Option<(u64, u64)>) -> CopyLanding {
    match current {
        Some((epoch, version)) if epoch == request.epoch => {
            if version == request.version {
                CopyLanding::Current
            } else {
                CopyLanding::Earlier
            }
        }
        _ => CopyLanding::Stale,
    }
}

/// What to do after the user discards unsaved edits. Cancel keeps the session.
#[derive(Clone, Copy)]
enum AfterDiscard {
    Close { quit: bool },
    PickOpen,
}

fn close_decision(answer: Option<usize>, expected: u64, current: u64) -> CloseDecision {
    if answer != Some(1) {
        CloseDecision::Cancel
    } else if expected != current {
        CloseDecision::AskAgain
    } else {
        CloseDecision::Discard
    }
}

/// How the scroll content is shown: the overlay viewport in window logical
/// pixels, the live scroll offset and the zoom. Device pixels never enter, as
/// GPUI and AppKit both convert window logical pixels themselves.
#[derive(Clone, Copy)]
struct Placement {
    viewport: Bounds<Pixels>,
    offset: Point<Pixels>,
    scale: ScreenScale,
}
impl Placement {
    fn page_origin(&self, page: usize, above: Emu) -> Point<Pixels> {
        point(
            self.viewport.origin.x + px(DOC_PAD) + self.offset.x,
            self.viewport.origin.y
                + px(DOC_PAD + self.scale.logical_pixels(above) as f32 + page as f32 * DOC_GAP)
                + self.offset.y,
        )
    }
    fn project(&self, b: SceneBox) -> Bounds<Pixels> {
        let page = self.page_origin(b.page, b.above);
        Bounds::new(
            point(
                page.x + px(self.scale.logical_pixels(b.x) as f32),
                page.y + px(self.scale.logical_pixels(b.top) as f32),
            ),
            size(
                px((self.scale.logical_pixels(b.width) as f32).max(1.0)),
                px(self.scale.logical_pixels(b.height) as f32),
            ),
        )
    }
}
/// A zoom-independent box on one page, with the total height of the pages
/// above it, so it can be placed without the layout that produced it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SceneBox {
    page: usize,
    above: Emu,
    x: Emu,
    top: Emu,
    width: Emu,
    height: Emu,
}
impl SceneBox {
    fn on(
        layout: &joyword_layout::LayoutSnapshot,
        page: usize,
        x: Emu,
        top: Emu,
        width: Emu,
        height: Emu,
    ) -> Option<Self> {
        Some(Self {
            page,
            above: pages_above(layout, page)?,
            x,
            top,
            width,
            height,
        })
    }
    fn caret(layout: &joyword_layout::LayoutSnapshot, at: Position) -> Option<Self> {
        let c = layout.caret(at).ok()?;
        Self::on(layout, c.page, c.x, c.top, Emu::new(1), c.height)
    }
}
fn pages_above(layout: &joyword_layout::LayoutSnapshot, page: usize) -> Option<Emu> {
    let above = layout.pages().get(..page)?.iter().map(|p| p.height.raw());
    Some(Emu::new(above.sum()))
}
fn project_page(
    layout: &joyword_layout::LayoutSnapshot,
    placement: Placement,
    page: usize,
) -> Option<Bounds<Pixels>> {
    let scene = layout.pages().get(page)?;
    Some(Bounds::new(
        placement.page_origin(page, pages_above(layout, page)?),
        size(
            px(placement.scale.logical_pixels(scene.width) as f32),
            px(placement.scale.logical_pixels(scene.height) as f32),
        ),
    ))
}
/// Pages whose boxes meet the viewport at the live scroll offset. Never
/// empty while there are pages: with no page in view (a tiny viewport, a gap
/// between pages) the nearest page above the viewport's bottom stands in.
fn visible_pages(layout: &joyword_layout::LayoutSnapshot, placement: Placement) -> Range<usize> {
    let viewport = placement.viewport;
    let (mut above, mut started, mut shown) = (Emu::new(0), 0, None::<Range<usize>>);
    for (i, page) in layout.pages().iter().enumerate() {
        let top = placement.page_origin(i, above).y;
        if top > viewport.bottom() {
            break;
        }
        started = i;
        let bottom = top + px(placement.scale.logical_pixels(page.height) as f32);
        if bottom >= viewport.top() {
            shown = Some(shown.map_or(i..i + 1, |run| run.start..i + 1));
        }
        above = Emu::new(above.raw() + page.height.raw());
    }
    shown.unwrap_or(started..(started + 1).min(layout.pages().len()))
}
/// UTF-16 offset where the composition starts, or where the next one would
/// replace the selection.
fn ime_origin(view: &InputView) -> usize {
    view.marked_range()
        .map_or_else(|| view.selected_range().0.start, |marked| marked.start)
}
/// Caret box at the IME origin, taken from the last current layout of a
/// committed document version. AppKit asks for the candidate position right
/// after each preedit change, before that preedit's background layout exists;
/// the origin of the composition does not move while the committed version is
/// unchanged, so this answers until the exact layout arrives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImeAnchor {
    version: u64,
    origin: usize,
    caret: SceneBox,
}
impl ImeAnchor {
    fn capture(
        view: &InputView,
        version: u64,
        layout: &joyword_layout::LayoutSnapshot,
    ) -> Option<Self> {
        let origin = ime_origin(view);
        let at = view.index().position(origin, Bias::After).ok()?;
        Some(Self {
            version,
            origin,
            caret: SceneBox::caret(layout, at)?,
        })
    }
    /// Only ranges inside the current composition, or the selection it will
    /// replace, of the same committed version with the same origin.
    fn covers(&self, view: &InputView, version: u64, range: &Range<usize>) -> bool {
        let span = view
            .marked_range()
            .unwrap_or_else(|| view.selected_range().0);
        self.version == version
            && self.origin == ime_origin(view)
            && span.start <= range.start
            && range.end <= span.end
    }
}
/// Window logical-pixel box for an IME range. The current layout answers
/// exactly; without one only a matching origin anchor may answer. Ranges past
/// the end are clamped, as AppKit may ask for the character after the caret.
fn ime_bounds(
    view: &InputView,
    version: u64,
    layout: Option<&joyword_layout::LayoutSnapshot>,
    anchor: Option<&ImeAnchor>,
    placement: Placement,
    range: Range<usize>,
) -> Option<Bounds<Pixels>> {
    let index = view.index();
    let range = range.start.min(index.len_utf16())..range.end.min(index.len_utf16());
    let exact = |layout: &joyword_layout::LayoutSnapshot| {
        index.text_for_range(range.clone()).ok()?;
        let head = view.selection().head;
        if range.is_empty() {
            let at = if index.offset(head).ok() == Some(range.start) {
                head
            } else {
                index.position(range.start, Bias::After).ok()?
            };
            SceneBox::caret(layout, at)
        } else {
            let start = index.position(range.start, Bias::After).ok()?;
            let end = index.position(range.end, Bias::Before).ok()?;
            let r = layout
                .selection_rects(start, end)
                .ok()?
                .into_iter()
                .next()?;
            SceneBox::on(layout, r.page, r.x, r.top, r.width, r.height)
        }
    };
    layout
        .and_then(exact)
        .or_else(|| {
            anchor
                .filter(|a| a.covers(view, version, &range))
                .map(|a| a.caret)
        })
        .map(|b| placement.project(b))
}
/// What to scroll into view after input: the caret of the current layout, or,
/// while a preedit's layout is pending, its origin from the anchor, so the box
/// AppKit asks for next is already on screen. The flag means the exact reveal
/// is still owed once the layout lands.
fn reveal_target(
    view: &InputView,
    version: u64,
    layout: Option<&joyword_layout::LayoutSnapshot>,
    anchor: Option<&ImeAnchor>,
) -> Option<(SceneBox, bool)> {
    if let Some(layout) = layout {
        return SceneBox::caret(layout, view.selection().head).map(|caret| (caret, false));
    }
    let origin = ime_origin(view);
    anchor
        .filter(|a| a.covers(view, version, &(origin..origin)))
        .map(|a| (a.caret, true))
}
fn content_size(layout: &joyword_layout::LayoutSnapshot, scale: ScreenScale) -> gpui::Size<Pixels> {
    let pages = layout.pages();
    let width = pages
        .iter()
        .map(|p| scale.logical_pixels(p.width) as f32)
        .fold(0.0f32, f32::max);
    let height = pages
        .iter()
        .map(|p| scale.logical_pixels(p.height) as f32)
        .sum::<f32>()
        + pages.len().saturating_sub(1) as f32 * DOC_GAP;
    size(px(width + 2.0 * DOC_PAD), px(height + 2.0 * DOC_PAD))
}
/// Document position under a window point. `clamp` picks the nearest page
/// for drags that leave the pages or the viewport.
fn hit_at(
    layout: &joyword_layout::LayoutSnapshot,
    placement: Placement,
    zoom: u16,
    point: Point<Pixels>,
    clamp: bool,
) -> Option<Position> {
    if !clamp && !placement.viewport.contains(&point) {
        return None;
    }
    let (page, bounds) = (0..layout.pages().len())
        .filter_map(|i| project_page(layout, placement, i).map(|b| (i, b)))
        .filter(|(_, b)| clamp || b.contains(&point))
        .min_by(|(_, a), (_, b)| {
            let distance = |r: &Bounds<Pixels>| {
                (f32::from(point.y - r.origin.y) - f32::from(r.size.height) / 2.0).abs()
            };
            distance(a).total_cmp(&distance(b))
        })?;
    let (x, y) = local_emu(point, bounds, zoom);
    layout.hit_test(page, x, y).ok()
}
fn local_emu(point: Point<Pixels>, page: Bounds<Pixels>, zoom: u16) -> (Emu, Emu) {
    let emu_per_px = Emu::PER_INCH as f64 / 96.0 / (f64::from(zoom) / 100.0);
    (
        Emu::new((f64::from(f32::from(point.x - page.origin.x)) * emu_per_px).round() as i64),
        Emu::new((f64::from(f32::from(point.y - page.origin.y)) * emu_per_px).round() as i64),
    )
}
fn reveal_offset(
    viewport: Bounds<Pixels>,
    caret: Bounds<Pixels>,
    mut offset: Point<Pixels>,
    content: gpui::Size<Pixels>,
) -> Point<Pixels> {
    let margin = px(8.0);
    if caret.top() < viewport.top() + margin {
        offset.y += viewport.top() + margin - caret.top();
    } else if caret.bottom() > viewport.bottom() - margin {
        offset.y -= caret.bottom() - viewport.bottom() + margin;
    }
    if caret.left() < viewport.left() + margin {
        offset.x += viewport.left() + margin - caret.left();
    } else if caret.right() > viewport.right() - margin {
        offset.x -= caret.right() - viewport.right() + margin;
    }
    offset.x = offset
        .x
        .max(-(content.width - viewport.size.width).max(px(0.0)))
        .min(px(0.0));
    offset.y = offset
        .y
        .max(-(content.height - viewport.size.height).max(px(0.0)))
        .min(px(0.0));
    offset
}

fn clipboard_image_format(bytes: &[u8]) -> Option<gpui::ImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(gpui::ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(gpui::ImageFormat::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(gpui::ImageFormat::Gif)
    } else if bytes.starts_with(b"BM") {
        Some(gpui::ImageFormat::Bmp)
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        Some(gpui::ImageFormat::Tiff)
    } else {
        None
    }
}

fn link_modifier(modifiers: &gpui::Modifiers) -> bool {
    #[cfg(target_os = "macos")]
    {
        modifiers.control || modifiers.platform
    }
    #[cfg(not(target_os = "macos"))]
    {
        modifiers.control
    }
}

/// Hands `url` to the platform opener as one argument. No shell is involved.
fn open_external(url: &str) -> std::io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut child = std::process::Command::new(program).arg(url).spawn()?;
    let _ = std::thread::Builder::new()
        .name("joyword-open".into())
        .spawn(move || {
            let _ = child.wait();
        });
    Ok(())
}

fn rich_as_text(document: &joyword_shell::rich_paste::RichDocument) -> String {
    let mut out = String::new();
    for (index, paragraph) in document.paragraphs.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        for run in &paragraph.runs {
            out.push_str(&run.text);
        }
    }
    out
}

fn input_error_label(error: &InputError) -> &'static str {
    match error {
        InputError::ReadOnly | InputError::Transaction(joyword_engine::TxError::ReadOnly) => {
            "input.read_only"
        }
        InputError::InvalidRange | InputError::InsideSurrogate | InputError::UnknownPosition(_) => {
            "input.invalid_range"
        }
        InputError::InvalidDocument | InputError::UnsupportedBody => "input.unsupported_document",
        InputError::Budget(_) | InputError::GenerationOverflow => "input.limit",
        InputError::StaleComposition => "input.stale",
        InputError::InvalidText(_) => "input.unsupported_text",
        InputError::Image(_) => "input.image",
        InputError::Undo(_) | InputError::Transaction(_) | InputError::Rejected(_) => {
            "input.rejected"
        }
    }
}

impl JoyWord {
    pub(crate) fn install_input_lifecycle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |view, cx| {
                view.cancel_preedit(cx);
                if !view
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.has_unsaved_changes())
                {
                    return true;
                }
                view.request_close(false, window, cx);
                false
            })
            .unwrap_or(false)
        });
        let focus = self.focus_handle.clone();
        self.subscriptions
            .push(cx.on_blur(&focus, window, |view, _, cx| view.cancel_preedit(cx)));
        self.subscriptions
            .push(cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() {
                    view.cancel_preedit(cx);
                }
            }));
    }
    pub(crate) fn request_close(
        &mut self,
        quit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_if_dirty(AfterDiscard::Close { quit }, window, cx);
    }
    pub(crate) fn begin_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_if_dirty(AfterDiscard::PickOpen, window, cx);
    }
    pub(crate) fn install_editor(&mut self, editor: Box<dyn DocEditor>) {
        self.editor = Some(editor);
        self.copy_epoch = self.copy_epoch.wrapping_add(1);
    }
    pub(crate) fn begin_save_original(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(job) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.save_original_job())
        else {
            self.begin_save_copy(window, cx);
            return;
        };
        if self
            .original_save
            .is_some_and(|phase| phase.epoch() == self.copy_epoch)
        {
            return;
        }
        let request = OriginalRequest {
            epoch: self.copy_epoch,
            version: job.version().get(),
            document: self.held_document,
        };
        self.original_save = Some(OriginalSavePhase::Probing(request));
        self.copy_note = None;
        self.input_error = None;
        cx.spawn_in(window, async move |view, cx| {
            let output = cx
                .background_spawn(async move {
                    job.run(joyword_shell::storage::OriginalChoice::Decline)
                })
                .await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.original_probe_landed(request, output, window, cx)
            });
        })
        .detach();
    }

    fn original_probe_landed(
        &mut self,
        request: OriginalRequest,
        output: joyword_shell::docx::SaveOriginalJobOutput,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.original_save == Some(OriginalSavePhase::Probing(request)) {
            self.original_save = None;
        }
        if request.epoch != self.copy_epoch {
            return;
        }
        let Some(error) = output.storage_error() else {
            let message = match output.result() {
                Err(error) => error.to_string(),
                Ok(_) => self.label("file.original.unexpected"),
            };
            self.report_input_error(message);
            cx.notify();
            return;
        };
        let Some(blockers) = error.blockers().filter(|blockers| {
            blockers.contains(&joyword_shell::storage::ReplacementBlocker::ConfirmationRequired)
        }) else {
            self.report_input_error(error.to_string());
            cx.notify();
            return;
        };
        let detail = self.original_loss_detail(error.path(), blockers);
        self.original_save = Some(OriginalSavePhase::Prompting(request));
        let catalog = Catalog::shipped(self.locale);
        let receiver = window.prompt(
            gpui::PromptLevel::Warning,
            &catalog.text("file.original.prompt"),
            Some(&detail),
            &[
                gpui::PromptButton::cancel(catalog.text("close.cancel")),
                gpui::PromptButton::ok(catalog.text("file.original.save_copy")),
                gpui::PromptButton::new(catalog.text("file.original.confirm")),
            ],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let answer = receiver.await.ok();
            let _ = view.update_in(cx, |view, window, cx| {
                if view.original_save == Some(OriginalSavePhase::Prompting(request)) {
                    view.original_save = None;
                }
                let Some(editor) = view.editor.as_ref() else {
                    return;
                };
                match original_decision(answer, request.version, editor.version()) {
                    OriginalDecision::Cancel => {
                        window.focus(&view.focus_handle, cx);
                        cx.notify();
                    }
                    OriginalDecision::SaveCopy => view.begin_save_copy(window, cx),
                    OriginalDecision::Reprobe => view.begin_save_original(window, cx),
                    OriginalDecision::Confirm => view.start_original_write(request, window, cx),
                }
            });
        })
        .detach();
    }

    fn original_loss_detail(
        &self,
        path: &std::path::Path,
        blockers: &[joyword_shell::storage::ReplacementBlocker],
    ) -> String {
        use joyword_shell::storage::ReplacementBlocker as Blocker;

        let catalog = Catalog::shipped(self.locale);
        let losses = blockers
            .iter()
            .filter_map(|blocker| {
                let (key, item) = match blocker {
                    Blocker::UnobservableMetadata(item) => ("file.original.unobservable", item),
                    Blocker::UnpreservableMetadata(item) => ("file.original.unpreservable", item),
                    _ => return None,
                };
                let item = catalog.text(original_metadata_key(*item));
                Some(catalog.format(key, &[("item", &item)]))
            })
            .collect::<Vec<_>>();
        let losses = if losses.is_empty() {
            catalog.text("file.original.no_losses")
        } else {
            losses.join(", ")
        };
        let shown = path.display().to_string();
        catalog.format(
            "file.original.detail",
            &[("path", &shown), ("losses", &losses)],
        )
    }

    fn start_original_write(
        &mut self,
        request: OriginalRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.epoch != self.copy_epoch {
            return;
        }
        let Some(job) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.save_original_job())
        else {
            self.report_input_error(self.label("file.save_original.unavailable"));
            cx.notify();
            return;
        };
        if job.version().get() != request.version {
            self.begin_save_original(window, cx);
            return;
        }
        self.original_save = Some(OriginalSavePhase::Writing(request));
        self.copy_note = None;
        self.input_error = None;
        cx.spawn_in(window, async move |view, cx| {
            let output = cx
                .background_spawn(async move {
                    job.run(joyword_shell::storage::OriginalChoice::Confirm)
                })
                .await;
            let _ = view.update(cx, |view, cx| {
                view.original_write_landed(request, output, cx)
            });
        })
        .detach();
    }

    fn original_write_landed(
        &mut self,
        request: OriginalRequest,
        output: joyword_shell::docx::SaveOriginalJobOutput,
        cx: &mut Context<Self>,
    ) {
        if self.original_save == Some(OriginalSavePhase::Writing(request)) {
            self.original_save = None;
        }
        let published = output.published_receipt().cloned();
        let current = self
            .editor
            .as_ref()
            .map(|editor| (self.copy_epoch, editor.version()));
        let landing = copy_landing(
            CopyRequest {
                epoch: request.epoch,
                version: request.version,
            },
            current,
        );
        let catalog = Catalog::shipped(self.locale);
        if landing != CopyLanding::Stale {
            let result = self
                .editor
                .as_mut()
                .map(|editor| editor.apply_original_save(output));
            match result {
                Some(Ok(receipt)) => {
                    let key = if landing == CopyLanding::Current {
                        "file.original.saved"
                    } else {
                        "file.original.saved_earlier"
                    };
                    let shown = receipt.path().display().to_string();
                    self.copy_note = Some(catalog.format(key, &[("path", &shown)]));
                    self.input_error = None;
                }
                Some(Err(error)) => {
                    self.report_input_error(
                        catalog.format("file.original.error", &[("reason", &error)]),
                    );
                    if let Some(receipt) = published {
                        let key = if landing == CopyLanding::Current {
                            "file.original.saved"
                        } else {
                            "file.original.saved_earlier"
                        };
                        let shown = receipt.path().display().to_string();
                        self.copy_note = Some(catalog.format(key, &[("path", &shown)]));
                    }
                }
                None => {}
            }
        } else {
            let routed = request
                .document
                .and_then(|id| self.session.apply_original_save(id, output));
            match routed {
                Some(Ok(receipt)) => {
                    let shown = receipt.path().display().to_string();
                    self.copy_note =
                        Some(catalog.format("file.original.saved_background", &[("path", &shown)]));
                    self.input_error = None;
                }
                Some(Err(error)) => {
                    let reason = error.to_string();
                    self.report_input_error(
                        catalog.format("file.original.error", &[("reason", &reason)]),
                    );
                }
                None => {
                    if let Some(receipt) = published {
                        let shown = receipt.path().display().to_string();
                        self.copy_note = Some(
                            catalog.format("file.original.saved_background", &[("path", &shown)]),
                        );
                    }
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn begin_save_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .copy
            .is_some_and(|phase| phase.epoch() == self.copy_epoch)
            || self
                .editor
                .as_ref()
                .is_none_or(|editor| !editor.can_save_copy())
        {
            return;
        }
        let suggested = self
            .editor
            .as_ref()
            .and_then(|editor| editor.source_path())
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(|name| format!("copy-{name}"))
            .or_else(|| Some("copy-untitled.docx".to_owned()));
        let directory = self
            .editor
            .as_ref()
            .and_then(|editor| editor.source_path())
            .and_then(|path| path.parent())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let epoch = self.copy_epoch;
        self.copy = Some(CopyPhase::Picking(epoch));
        let receiver = cx.prompt_for_new_path(&directory, suggested.as_deref());
        // Detached from the view: a write that has started finishes even if
        // the window goes away, and its result then has nowhere to land.
        cx.spawn_in(window, async move |view, cx| {
            let picked = receiver.await.ok().and_then(Result::ok).flatten();
            let started = view.update(cx, |view, cx| view.start_copy(epoch, picked, cx));
            let Ok(Some((request, job, path))) = started else {
                return;
            };
            let result = cx
                .background_spawn(async move { job.run(&path).map_err(|error| error.to_string()) })
                .await;
            let _ = view.update(cx, |view, cx| view.copy_landed(request, result, cx));
        })
        .detach();
    }
    /// The path is chosen. Take the committed version now; planning and IO
    /// run in the background. Typing may continue and stays unsaved.
    fn start_copy(
        &mut self,
        epoch: u64,
        picked: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) -> Option<(
        CopyRequest,
        joyword_shell::docx::SaveCopyJob,
        std::path::PathBuf,
    )> {
        if self.copy == Some(CopyPhase::Picking(epoch)) {
            self.copy = None;
        }
        cx.notify();
        let path = picked?;
        if self.copy_epoch != epoch {
            return None;
        }
        let job = self.editor.as_ref()?.save_copy_job()?;
        let request = CopyRequest {
            epoch,
            version: job.version().get(),
        };
        self.copy = Some(CopyPhase::Writing(request));
        self.copy_note = None;
        Some((request, job, path))
    }
    fn copy_landed(
        &mut self,
        request: CopyRequest,
        result: Result<SaveCopyReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        if self.copy == Some(CopyPhase::Writing(request)) {
            self.copy = None;
        }
        cx.notify();
        let current = self
            .editor
            .as_ref()
            .map(|editor| (self.copy_epoch, editor.version()));
        let saved = match copy_landing(request, current) {
            CopyLanding::Stale => return,
            CopyLanding::Current => "file.copy.saved",
            CopyLanding::Earlier => "file.copy.saved_earlier",
        };
        let catalog = Catalog::shipped(self.locale);
        match result {
            Ok(receipt) => {
                let shown = receipt.path().display().to_string();
                self.copy_note = Some(catalog.format(saved, &[("path", &shown)]));
                self.input_error = None;
            }
            Err(error) => {
                self.report_input_error(catalog.format("file.copy.error", &[("reason", &error)]));
            }
        }
    }
    fn confirm_if_dirty(
        &mut self,
        then: AfterDiscard,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_pending {
            return;
        }
        self.cancel_preedit(cx);
        let Some(editor) = self.editor.as_ref().filter(|e| e.has_unsaved_changes()) else {
            self.finish_discard(then, window, cx);
            return;
        };
        let expected = editor.version();
        self.close_pending = true;
        let catalog = Catalog::shipped(self.locale);
        let message = catalog.format("close.unsaved", &[("title", "JoyWord")]);
        let receiver = window.prompt(
            gpui::PromptLevel::Warning,
            &message,
            Some(&self.label("stage")),
            &[
                gpui::PromptButton::cancel(self.label("close.cancel")),
                gpui::PromptButton::new(self.label("close.discard")),
            ],
            cx,
        );
        self.close_task = Some(cx.spawn_in(window, async move |view, cx| {
            let answer = receiver.await.ok();
            let _ = view.update_in(cx, |view, window, cx| {
                view.close_pending = false;
                let Some(editor) = &view.editor else {
                    return;
                };
                match close_decision(answer, expected, editor.version()) {
                    CloseDecision::Cancel => {
                        window.focus(&view.focus_handle, cx);
                        cx.notify();
                    }
                    CloseDecision::AskAgain => view.confirm_if_dirty(then, window, cx),
                    CloseDecision::Discard => view.finish_discard(then, window, cx),
                }
            });
        }));
    }
    fn finish_discard(&mut self, then: AfterDiscard, window: &mut Window, cx: &mut Context<Self>) {
        match then {
            AfterDiscard::Close { quit } => {
                if quit {
                    cx.quit();
                } else {
                    window.remove_window();
                }
            }
            AfterDiscard::PickOpen => self.pick_open(cx),
        }
    }
    fn pick_open(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        self.close_task = Some(cx.spawn(async move |view, cx| {
            let picked = receiver
                .await
                .ok()
                .and_then(Result::ok)
                .flatten()
                .and_then(|mut paths| paths.pop());
            let Some(path) = picked else {
                return;
            };
            let current = view
                .update(cx, |view, _cx| {
                    view.editor
                        .as_ref()
                        .and_then(|editor| editor.source_path().map(std::path::Path::to_path_buf))
                })
                .ok()
                .flatten();
            if let Some(current) = current.as_deref()
                && path.canonicalize().ok().as_deref() == Some(current)
            {
                return;
            }
            let Some(generation) = view
                .update(cx, |view, _cx| {
                    let generation = view.session.begin_open();
                    view.open_gate = Some(generation);
                    generation
                })
                .ok()
            else {
                return;
            };
            let opened = cx
                .background_spawn(async move { BoundDocx::open_default(&path) })
                .await;
            let _ = view.update(cx, |view, cx| {
                if view.open_gate != Some(generation) {
                    view.open_gate = None;
                    let _ = view.session.finish_open(generation);
                    cx.notify();
                    return;
                }
                let buffered = match view.session.finish_open(generation) {
                    Ok(text) => text,
                    Err(error) => {
                        view.open_gate = None;
                        view.report_input_error(
                            Catalog::shipped(view.locale)
                                .format("file.open.error", &[("reason", &error.to_string())]),
                        );
                        cx.notify();
                        return;
                    }
                };
                view.open_gate = None;
                match opened {
                    Ok(bound) => match TextInput::new(bound, InputLimits::default()) {
                        Ok(mut input) => {
                            if !buffered.is_empty()
                                && let Err(error) =
                                    input.replace_from(InputSource::Command, None, &buffered)
                            {
                                view.report_input_error(view.label(input_error_label(&error)));
                                cx.notify();
                                return;
                            }
                            view.return_held_document();
                            let _ = view.session.attach(input);
                            view.show_active_document(cx);
                        }
                        Err(error) => {
                            view.report_input_error(view.label(input_error_label(&error)));
                        }
                    },
                    Err(error) => {
                        view.report_input_error(
                            Catalog::shipped(view.locale)
                                .format("file.open.error", &[("reason", &error.to_string())]),
                        );
                    }
                }
                cx.notify();
            });
        }));
    }
    fn apply_input(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut dyn DocEditor) -> Result<(), InputError>,
    ) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let result = f(editor.as_mut());
        if result.is_ok() {
            self.copy_note = None;
            self.journal_edit();
        }
        self.input_error = result
            .err()
            .map(|error| self.label(input_error_label(&error)));
        self.preferred_x = None;
        // Committing a preview can publish exactly the same snapshot. Its
        // source IDs, version and content must all match before reusing the
        // layout and its pixels.
        if let (Some(font), Some(profile), Some(editor)) = (&self.font, self.profile, &self.editor)
        {
            self.canvas.reuse(editor.view(), font, profile);
        }
        self.capture_ime_anchor();
        self.reveal_pending = true;
        self.reveal_caret();
        cx.notify();
    }
    fn journal_edit(&self) {
        let Some(directory) = &self.recovery_dir else {
            return;
        };
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(baseline) = editor.recovery_baseline() else {
            return;
        };
        let text = editor.view().index().text();
        let _ = joyword_shell::storage::commit_recovery(directory, baseline, text);
    }
    fn cancel_preedit(&mut self, cx: &mut Context<Self>) {
        self.dragging = false;
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.is_composing())
        {
            self.apply_input(cx, |editor| editor.cancel_composition().map(|_| ()));
        }
    }
    pub(crate) fn input_availability(&self, id: CommandId) -> Availability {
        if matches!(id, FILE_SAVE | FILE_SAVE_COPY) {
            return if self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.can_save_copy())
            {
                Availability::Enabled
            } else {
                Availability::Disabled("file.save_copy.unavailable")
            };
        }
        if matches!(
            id,
            STYLE_LIST
                | STYLE_APPLY
                | LIST_APPLY
                | LIST_LEVEL
                | LIST_REMOVE
                | FIND_NEXT
                | FIND_PREVIOUS
                | FIND_REPLACE
                | FIND_REPLACE_ALL
                | FORMAT_COPY
                | FORMAT_PAINT
                | FORMAT_CANCEL
                | TABLE_INSERT
                | TABLE_ROW_INSERT
                | TABLE_ROW_DELETE
                | TABLE_COLUMN_INSERT
                | TABLE_COLUMN_DELETE
                | TABLE_SELECT
        ) {
            if self.open_gate.is_some() && id != STYLE_LIST {
                return Availability::Disabled("input.read_only");
            }
            let Some(editor) = &self.editor else {
                return Availability::Disabled("input.loading");
            };
            if matches!(
                id,
                STYLE_APPLY
                    | LIST_APPLY
                    | LIST_LEVEL
                    | LIST_REMOVE
                    | FIND_REPLACE
                    | FIND_REPLACE_ALL
                    | FORMAT_PAINT
                    | TABLE_INSERT
                    | TABLE_ROW_INSERT
                    | TABLE_ROW_DELETE
                    | TABLE_COLUMN_INSERT
                    | TABLE_COLUMN_DELETE
            ) && editor.read_only()
            {
                return Availability::Disabled("input.read_only");
            }
            return Availability::Enabled;
        }
        if id.0.starts_with("page.") {
            if self.open_gate.is_some() {
                return Availability::Disabled("input.read_only");
            }
            let Some(editor) = &self.editor else {
                return Availability::Disabled("input.loading");
            };
            if editor.read_only() {
                return Availability::Disabled("input.read_only");
            }
            return Availability::Enabled;
        }
        if !id.0.starts_with("edit.") {
            return Availability::Enabled;
        }
        if self.open_gate.is_some() {
            return Availability::Disabled("input.read_only");
        }
        let Some(editor) = &self.editor else {
            return Availability::Disabled("input.loading");
        };
        match id {
            EDIT_UNDO if !editor.can_undo() && !editor.is_composing() => {
                Availability::Disabled("input.no_undo")
            }
            EDIT_REDO if !editor.can_redo() => Availability::Disabled("input.no_redo"),
            EDIT_REDO if editor.is_composing() => Availability::Disabled("input.composing"),
            EDIT_CANCEL if !editor.is_composing() => Availability::Disabled("input.no_composition"),
            EDIT_UNDO | EDIT_REDO | EDIT_BACKSPACE | EDIT_DELETE | EDIT_ENTER | EDIT_BOLD
            | EDIT_ITALIC | EDIT_ALIGN_CENTER | EDIT_PASTE | EDIT_INSERT | EDIT_INSERT_IMAGE
                if editor.read_only() =>
            {
                Availability::Disabled("input.read_only")
            }
            EDIT_UP | EDIT_DOWN | EDIT_EXTEND_UP | EDIT_EXTEND_DOWN | EDIT_HOME | EDIT_END
            | EDIT_EXTEND_HOME | EDIT_EXTEND_END
                if self.ready_layout().is_none() =>
            {
                Availability::Disabled("input.layout_pending")
            }
            _ => Availability::Enabled,
        }
    }
    pub(crate) fn return_held_document(&mut self) {
        let Some(id) = self.held_document.take() else {
            return;
        };
        let Some(editor) = self.editor.take() else {
            return;
        };
        if let Some(input) = editor.into_bound() {
            let _ = self.session.restore(id, input);
        }
    }
    pub(crate) fn show_active_document(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.session.hold_active() else {
            return;
        };
        self.held_document = self.session.active_id();
        self.clear_canvas(cx);
        self.install_editor(Box::new(input));
        self.ime_anchor = None;
        self.copy_note = None;
        self.input_error = None;
        self.preferred_x = None;
        self.reveal_pending = true;
        cx.notify();
    }
    pub(crate) fn open_path_sync(
        &mut self,
        path: &std::path::Path,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.return_held_document();
        let generation = self.session.begin_open();
        self.open_gate = Some(generation);
        let opened = self.session.accept_open(generation, path);
        self.open_gate = None;
        opened.map_err(|error| error.to_string())?;
        self.show_active_document(cx);
        Ok(())
    }
    pub(crate) fn save_to(&mut self, path: &std::path::Path) -> Result<(), String> {
        let saved = self
            .editor
            .as_ref()
            .ok_or_else(|| "file.save_copy.unavailable".to_string())?
            .save_copy(path)
            .map_err(|error| error.to_string())?;
        let shown = saved.path().display().to_string();
        self.copy_note =
            Some(Catalog::shipped(self.locale).format("file.copy.saved", &[("path", &shown)]));
        self.input_error = None;
        Ok(())
    }
    pub(crate) fn execute_input(
        &mut self,
        id: CommandId,
        args: &std::collections::BTreeMap<&'static str, joyword_shell::command::ArgValue>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.input_error = None;
        if self.open_gate.is_some() {
            if let EDIT_INSERT = id
                && let Some(joyword_shell::command::ArgValue::Text(text)) = args.get("text")
            {
                self.session
                    .type_during_open(text)
                    .map_err(|error| error.to_string())?;
                return Ok(());
            }
            return Err("input.read_only".into());
        }
        match id {
            EDIT_UNDO => self.apply_input(cx, |editor| editor.undo()),
            EDIT_REDO => self.apply_input(cx, |editor| editor.redo()),
            EDIT_SELECT_ALL => self.apply_input(cx, |editor| editor.select_all()),
            EDIT_BACKSPACE => {
                self.apply_input(cx, |editor| editor.delete(Direction::Backward).map(|_| ()))
            }
            EDIT_DELETE => {
                self.apply_input(cx, |editor| editor.delete(Direction::Forward).map(|_| ()))
            }
            EDIT_ENTER => self.apply_input(cx, |editor| editor.replace(None, "\n").map(|_| ())),
            EDIT_CANCEL => self.apply_input(cx, |editor| editor.cancel_composition().map(|_| ())),
            EDIT_BOLD => self.apply_input(cx, |editor| {
                editor
                    .format_runs(joyword_engine::RunChange::Bold(Some(true)))
                    .map(|_| ())
            }),
            STYLE_LIST => {
                if let Some(editor) = self.editor.as_ref() {
                    let names = editor.list_styles();
                    self.copy_note = Some(if names.is_empty() {
                        self.label("command.style.list")
                    } else {
                        names.join("\n")
                    });
                } else {
                    self.input_error = Some("input.loading".into());
                }
            }
            STYLE_APPLY => {
                let Some(joyword_shell::command::ArgValue::Text(style)) = args.get("style") else {
                    self.input_error = Some("style".into());
                    return Err("style".into());
                };
                let style = style.clone();
                self.apply_input(cx, |editor| editor.apply_style(&style).map(|_| ()))
            }
            LIST_APPLY => {
                let Some(joyword_shell::command::ArgValue::Text(num)) = args.get("num") else {
                    self.input_error = Some("num".into());
                    return Err("num".into());
                };
                let Ok(num_id) = num.parse::<u32>() else {
                    self.input_error = Some("num".into());
                    return Err("num".into());
                };
                self.apply_input(cx, |editor| editor.apply_list(num_id).map(|_| ()))
            }
            LIST_LEVEL => {
                let Some(joyword_shell::command::ArgValue::Text(level)) = args.get("level") else {
                    self.input_error = Some("level".into());
                    return Err("level".into());
                };
                let Ok(level) = level.parse::<u8>() else {
                    self.input_error = Some("level".into());
                    return Err("level".into());
                };
                self.apply_input(cx, |editor| editor.set_list_level(level).map(|_| ()))
            }
            LIST_REMOVE => self.apply_input(cx, |editor| editor.remove_list().map(|_| ())),
            PAGE_BREAK => self.apply_input(cx, |editor| editor.insert_page_break().map(|_| ())),
            PAGE_SIZE => {
                let Some(joyword_shell::command::ArgValue::Text(width)) = args.get("width") else {
                    self.input_error = Some("width".into());
                    return Err("width".into());
                };
                let Some(joyword_shell::command::ArgValue::Text(height)) = args.get("height")
                else {
                    self.input_error = Some("height".into());
                    return Err("height".into());
                };
                let (Ok(width), Ok(height)) = (width.parse::<i64>(), height.parse::<i64>()) else {
                    self.input_error = Some("page.size".into());
                    return Err("page.size".into());
                };
                self.apply_input(cx, |editor| editor.set_page_size(width, height).map(|_| ()))
            }
            PAGE_MARGINS => {
                let parse = |name: &str| -> Result<i64, String> {
                    let Some(joyword_shell::command::ArgValue::Text(text)) = args.get(name) else {
                        return Err(name.into());
                    };
                    text.parse::<i64>().map_err(|_| name.to_string())
                };
                let top = parse("top")?;
                let right = parse("right")?;
                let bottom = parse("bottom")?;
                let left = parse("left")?;
                self.apply_input(cx, |editor| {
                    editor
                        .set_page_margins(top, right, bottom, left)
                        .map(|_| ())
                })
            }
            PAGE_ORIENTATION => {
                let Some(joyword_shell::command::ArgValue::Text(orientation)) =
                    args.get("orientation")
                else {
                    self.input_error = Some("orientation".into());
                    return Err("orientation".into());
                };
                let landscape = match orientation.as_str() {
                    "landscape" => true,
                    "portrait" => false,
                    _ => {
                        self.input_error = Some("orientation".into());
                        return Err("orientation".into());
                    }
                };
                self.apply_input(cx, |editor| {
                    editor.set_page_orientation(landscape).map(|_| ())
                })
            }
            FIND_NEXT | FIND_PREVIOUS | FIND_REPLACE | FIND_REPLACE_ALL | FORMAT_COPY
            | FORMAT_PAINT | FORMAT_CANCEL => {
                let request = match id {
                    FIND_NEXT | FIND_PREVIOUS => {
                        let Some(joyword_shell::command::ArgValue::Text(query)) = args.get("query")
                        else {
                            self.input_error = Some("query".into());
                            return Err("query".into());
                        };
                        joyword_shell::input::EditRequest::Find {
                            query: query.clone(),
                            case_sensitive: matches!(
                                args.get("case"),
                                Some(joyword_shell::command::ArgValue::Bool(true))
                            ),
                            whole_word: matches!(
                                args.get("word"),
                                Some(joyword_shell::command::ArgValue::Bool(true))
                            ),
                            forward: id == FIND_NEXT,
                        }
                    }
                    FIND_REPLACE | FIND_REPLACE_ALL => {
                        let Some(joyword_shell::command::ArgValue::Text(query)) = args.get("query")
                        else {
                            self.input_error = Some("query".into());
                            return Err("query".into());
                        };
                        let Some(joyword_shell::command::ArgValue::Text(replacement)) =
                            args.get("replacement")
                        else {
                            self.input_error = Some("replacement".into());
                            return Err("replacement".into());
                        };
                        joyword_shell::input::EditRequest::Replace {
                            query: query.clone(),
                            replacement: replacement.clone(),
                            case_sensitive: matches!(
                                args.get("case"),
                                Some(joyword_shell::command::ArgValue::Bool(true))
                            ),
                            whole_word: matches!(
                                args.get("word"),
                                Some(joyword_shell::command::ArgValue::Bool(true))
                            ),
                            all: id == FIND_REPLACE_ALL,
                        }
                    }
                    FORMAT_COPY => joyword_shell::input::EditRequest::CopyFormat {
                        repeat: matches!(
                            args.get("repeat"),
                            Some(joyword_shell::command::ArgValue::Bool(true))
                        ),
                    },
                    FORMAT_PAINT => joyword_shell::input::EditRequest::PaintFormat,
                    _ => joyword_shell::input::EditRequest::CancelFormat,
                };
                let mut report = None;
                self.apply_input(cx, |editor| {
                    report = Some(editor.editing(request)?);
                    Ok(())
                });
                if let Some(report) = report {
                    let catalog = Catalog::shipped(self.locale);
                    let mut note = catalog.format(
                        "find.status",
                        &[
                            ("matches", &report.matches.to_string()),
                            ("skipped", &report.skipped.to_string()),
                            ("replaced", &report.replaced.to_string()),
                        ],
                    );
                    if report.truncated {
                        note.push(' ');
                        note.push_str(&catalog.text("find.truncated"));
                    }
                    if report.matches == 0
                        && report.replaced == 0
                        && matches!(
                            id,
                            FIND_NEXT | FIND_PREVIOUS | FIND_REPLACE | FIND_REPLACE_ALL
                        )
                    {
                        note = catalog.text("find.none");
                        if report.skipped > 0 {
                            note.push_str(&catalog.format(
                                "find.status",
                                &[
                                    ("matches", "0"),
                                    ("skipped", &report.skipped.to_string()),
                                    ("replaced", "0"),
                                ],
                            ));
                        }
                    }
                    if matches!(id, FORMAT_COPY | FORMAT_PAINT | FORMAT_CANCEL) {
                        note = catalog.text(if report.armed {
                            "format.armed"
                        } else {
                            "format.applied"
                        });
                    }
                    self.find_note = Some(note);
                }
            }
            EDIT_ITALIC => self.apply_input(cx, |editor| {
                editor
                    .format_runs(joyword_engine::RunChange::Italic(Some(true)))
                    .map(|_| ())
            }),
            EDIT_ALIGN_CENTER => self.apply_input(cx, |editor| {
                editor
                    .format_paragraph(joyword_engine::ParagraphChange::Justification(Some(
                        joyword_model::Justification::Center,
                    )))
                    .map(|_| ())
            }),
            EDIT_COPY => {
                if let Some(editor) = self.editor.as_ref() {
                    if let Some(bytes) = editor.selected_image_bytes() {
                        match clipboard_image_format(bytes.as_ref()) {
                            Some(format) => {
                                let image =
                                    gpui::Image::from_bytes(format, bytes.as_ref().to_vec());
                                cx.write_to_clipboard(gpui::ClipboardItem::new_image(&image));
                            }
                            None => self.input_error = Some(self.label("input.image")),
                        }
                    } else {
                        match editor.selected_text() {
                            Ok(text) => {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    text.clone(),
                                ));
                                self.clipboard = text;
                            }
                            Err(error) => self.input_error = Some(error.to_string()),
                        }
                    }
                } else {
                    self.input_error = Some("input.loading".into());
                }
            }
            EDIT_INSERT => {
                let Some(joyword_shell::command::ArgValue::Text(text)) = args.get("text") else {
                    self.input_error = Some("text".into());
                    return Err("text".into());
                };
                let text = text.clone();
                self.apply_input(cx, |editor| {
                    editor
                        .replace_from(InputSource::Command, None, &text)
                        .map(|_| ())
                })
            }
            EDIT_PASTE => {
                // The system pasteboard is the source of truth. `text()` is
                // not used: it turns file paths into text. The in-app string
                // only backs paste where no system clipboard exists.
                if let Some(item) = cx.read_from_clipboard() {
                    let offers: Vec<joyword_shell::input::ClipboardOffer<'_>> = item
                        .entries()
                        .iter()
                        .map(|entry| match entry {
                            gpui::ClipboardEntry::Image(image) => {
                                joyword_shell::input::ClipboardOffer::Image {
                                    bytes: image.bytes.as_slice(),
                                }
                            }
                            gpui::ClipboardEntry::String(text) => {
                                joyword_shell::input::ClipboardOffer::Text(text.text.as_str())
                            }
                            gpui::ClipboardEntry::ExternalPaths(_) => {
                                joyword_shell::input::ClipboardOffer::Paths
                            }
                        })
                        .collect();
                    match joyword_shell::input::choose_paste(&offers) {
                        joyword_shell::input::PasteChoice::Image(bytes) => {
                            let bytes = bytes.to_vec();
                            self.apply_input(cx, |editor| {
                                editor.insert_image_bytes(&bytes).map(|_| ())
                            });
                        }
                        joyword_shell::input::PasteChoice::Html(markup) => {
                            self.paste_parsed(
                                cx,
                                joyword_shell::rich_paste::parse_html(markup),
                                &offers,
                            );
                        }
                        joyword_shell::input::PasteChoice::Rtf(markup) => {
                            self.paste_parsed(
                                cx,
                                joyword_shell::rich_paste::parse_rtf(markup),
                                &offers,
                            );
                        }
                        joyword_shell::input::PasteChoice::Text(text) => {
                            let text = text.to_owned();
                            self.apply_input(cx, |editor| {
                                editor
                                    .replace_from(InputSource::Paste, None, &text)
                                    .map(|_| ())
                            });
                        }
                        joyword_shell::input::PasteChoice::Empty => {}
                        joyword_shell::input::PasteChoice::Refused(_) => {
                            self.input_error = Some(self.label("input.image"));
                        }
                    }
                } else if !self.clipboard.is_empty() {
                    let text = self.clipboard.clone();
                    self.apply_input(cx, |editor| {
                        editor
                            .replace_from(InputSource::Paste, None, &text)
                            .map(|_| ())
                    });
                }
            }
            EDIT_INSERT_IMAGE => {
                let Some(joyword_shell::command::ArgValue::Path(path)) = args.get("path") else {
                    self.input_error = Some("path".into());
                    return Err("path".into());
                };
                let path = path.clone();
                self.apply_input(cx, |editor| editor.insert_image_file(&path).map(|_| ()))
            }
            EDIT_LEFT | EDIT_RIGHT | EDIT_EXTEND_LEFT | EDIT_EXTEND_RIGHT => {
                let direction = if id == EDIT_LEFT || id == EDIT_EXTEND_LEFT {
                    Direction::Backward
                } else {
                    Direction::Forward
                };
                let extend = id == EDIT_EXTEND_LEFT || id == EDIT_EXTEND_RIGHT;
                self.apply_input(cx, |e| e.move_grapheme(direction, extend));
            }
            EDIT_UP | EDIT_DOWN | EDIT_EXTEND_UP | EDIT_EXTEND_DOWN | EDIT_HOME | EDIT_END
            | EDIT_EXTEND_HOME | EDIT_EXTEND_END => self.move_visual(id, cx),
            TABLE_INSERT => {
                let rows = table_axis(args, "rows")?;
                let columns = table_axis(args, "columns")?;
                self.apply_input(cx, |editor| editor.insert_table(rows, columns).map(|_| ()))
            }
            TABLE_ROW_INSERT => {
                self.apply_input(cx, |editor| editor.insert_table_row().map(|_| ()))
            }
            TABLE_ROW_DELETE => {
                self.apply_input(cx, |editor| editor.delete_table_row().map(|_| ()))
            }
            TABLE_COLUMN_INSERT => {
                self.apply_input(cx, |editor| editor.insert_table_column().map(|_| ()))
            }
            TABLE_COLUMN_DELETE => {
                self.apply_input(cx, |editor| editor.delete_table_column().map(|_| ()))
            }
            TABLE_SELECT => self.apply_input(cx, |editor| editor.select_table_cell()),
            _ => return Err(format!("{id} is not an input command")),
        }
        window.invalidate_character_coordinates();
        match &self.input_error {
            Some(reason) => Err(reason.clone()),
            None => Ok(()),
        }
    }
    fn move_visual(&mut self, id: CommandId, cx: &mut Context<Self>) {
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.is_composing())
        {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(frame) = self.ready_layout() else {
            return;
        };
        let selection = editor.view().selection();
        let Ok(caret) = frame.layout.caret(selection.head) else {
            return;
        };
        let Some(i) = frame
            .layout
            .lines()
            .iter()
            .position(|l| l.page == caret.page && l.top == caret.top)
        else {
            return;
        };
        let lines = frame.layout.lines();
        let vertical = matches!(id, EDIT_UP | EDIT_DOWN | EDIT_EXTEND_UP | EDIT_EXTEND_DOWN);
        let extend = matches!(
            id,
            EDIT_EXTEND_UP | EDIT_EXTEND_DOWN | EDIT_EXTEND_HOME | EDIT_EXTEND_END
        );
        let x = self.preferred_x.unwrap_or(caret.x);
        let target = if vertical {
            let j = if id == EDIT_UP || id == EDIT_EXTEND_UP {
                i.saturating_sub(1)
            } else {
                (i + 1).min(lines.len() - 1)
            };
            let line = &lines[j];
            frame
                .layout
                .hit_test(
                    line.page,
                    x,
                    Emu::new(line.top.raw() + line.height.raw() / 2),
                )
                .ok()
        } else {
            let line = &lines[i];
            let start = id == EDIT_HOME || id == EDIT_EXTEND_HOME;
            let stop = if start {
                line.carets.first()
            } else {
                line.carets.last()
            };
            stop.map(|stop| Position {
                paragraph: line.paragraph,
                offset: stop.offset,
                bias: if start { Bias::After } else { Bias::Before },
                ..selection.head
            })
        };
        if let Some(head) = target {
            self.apply_input(cx, |e| {
                e.select_positions(Selection {
                    anchor: if extend { selection.anchor } else { head },
                    head,
                })
            });
            if vertical {
                self.preferred_x = Some(x);
            }
        }
    }
    pub(crate) fn input_status(&self) -> String {
        if let Some(reason) = &self.input_error {
            return Catalog::shipped(self.locale).format("input.error", &[("reason", reason)]);
        }
        if let Some(phase) = self
            .original_save
            .filter(|phase| phase.epoch() == self.copy_epoch)
        {
            return match phase {
                OriginalSavePhase::Probing(_) => self.label("file.original.checking"),
                OriginalSavePhase::Prompting(_) => self.label("file.original.confirming"),
                OriginalSavePhase::Writing(_) => self.label("file.original.saving"),
            };
        }
        if matches!(self.copy, Some(CopyPhase::Writing(request)) if request.epoch == self.copy_epoch)
        {
            return self.label("file.copy.saving");
        }
        if let Some(frame) = self.ready_layout()
            && !frame.layout.diagnostics().is_empty()
        {
            return self.label("input.missing_glyphs");
        }
        if self.canvas_error.is_none()
            && self.canvas.layout().is_some()
            && self.ready_layout().is_none()
        {
            return self.label("input.layout_pending");
        }
        if let Some(note) = &self.copy_note {
            return note.clone();
        }
        if let Some(editor) = &self.editor {
            if editor.is_composing() {
                return self.label("input.composing");
            }
            if editor.has_unsaved_changes() {
                return self.label("input.dirty");
            }
        }
        self.label("status")
    }
    fn scale(&self) -> ScreenScale {
        ScreenScale::new(f64::from(self.zoom_percent) / 100.0).expect("clamped zoom")
    }
    fn placement(&self) -> Option<Placement> {
        Some(Placement {
            viewport: self.viewport_bounds?,
            offset: self.scroll.offset(),
            scale: self.scale(),
        })
    }
    /// Pages of the installed layout in view now; before the viewport is
    /// first laid out, the first page.
    pub(crate) fn visible_pages(&self) -> Range<usize> {
        let Some(frame) = self.canvas.layout() else {
            return 0..0;
        };
        let placement = self.placement().unwrap_or(Placement {
            viewport: Bounds::default(),
            offset: Point::default(),
            scale: self.scale(),
        });
        visible_pages(&frame.layout, placement)
    }
    pub(crate) fn reveal_caret(&mut self) {
        if !self.reveal_pending {
            return;
        }
        let (Some(editor), Some(placement)) = (&self.editor, self.placement()) else {
            return;
        };
        // The scroll content is the installed layout, current or not.
        let Some(painted) = self.canvas.layout() else {
            return;
        };
        let Some((caret, owed)) = reveal_target(
            editor.view(),
            editor.version(),
            self.ready_layout().map(|frame| frame.layout.as_ref()),
            self.ime_anchor.as_ref(),
        ) else {
            return;
        };
        let offset = reveal_offset(
            placement.viewport,
            placement.project(caret),
            placement.offset,
            content_size(&painted.layout, placement.scale),
        );
        self.scroll.set_offset(offset);
        self.reveal_pending = owed;
    }
    fn scene_rect(
        &self,
        page: usize,
        x: Emu,
        top: Emu,
        width: Emu,
        height: Emu,
    ) -> Option<Bounds<Pixels>> {
        let layout = &self.ready_layout()?.layout;
        let b = SceneBox::on(layout, page, x, top, width, height)?;
        Some(self.placement()?.project(b))
    }
    fn hit(&self, point: Point<Pixels>, clamp: bool) -> Option<Position> {
        hit_at(
            &self.ready_layout()?.layout,
            self.placement()?,
            self.zoom_percent,
            point,
            clamp,
        )
    }
    /// Insert Picture asks for a file, then uses the existing image insert path.
    pub(crate) fn prompt_insert_picture(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let picked = receiver
                .await
                .ok()
                .and_then(Result::ok)
                .flatten()
                .and_then(|mut paths| paths.pop());
            let Some(path) = picked else {
                return;
            };
            let _ = view.update(cx, |view, cx| {
                view.apply_input(cx, |editor| editor.insert_image_file(&path).map(|_| ()));
            });
        })
        .detach();
    }

    fn paste_parsed(
        &mut self,
        cx: &mut Context<Self>,
        parsed: joyword_shell::rich_paste::ParsedRich,
        offers: &[joyword_shell::input::ClipboardOffer<'_>],
    ) {
        let plain = joyword_shell::input::plain_text_offer(offers)
            .filter(|text| !text.is_empty())
            .map(str::to_owned);
        match parsed {
            joyword_shell::rich_paste::ParsedRich::Formatted(document) => {
                let fallback = rich_as_text(&document);
                self.apply_input(cx, |editor| match editor.paste_rich(&document) {
                    Err(InputError::Budget(_)) => {
                        let text = if fallback.is_empty() {
                            plain.clone().unwrap_or_default()
                        } else {
                            fallback.clone()
                        };
                        if text.is_empty() {
                            return Err(InputError::Budget("commands"));
                        }
                        editor
                            .replace_from(InputSource::Paste, None, &text)
                            .map(|_| ())
                    }
                    other => other.map(|_| ()),
                });
            }
            joyword_shell::rich_paste::ParsedRich::Plain(text) => {
                let text = if text.is_empty() {
                    plain.unwrap_or_default()
                } else {
                    text
                };
                if text.is_empty() {
                    return;
                }
                self.apply_input(cx, |editor| {
                    editor
                        .replace_from(InputSource::Paste, None, &text)
                        .map(|_| ())
                });
            }
            joyword_shell::rich_paste::ParsedRich::Refuse(_) => {
                if let Some(text) = plain {
                    self.apply_input(cx, |editor| {
                        editor
                            .replace_from(InputSource::Paste, None, &text)
                            .map(|_| ())
                    });
                } else {
                    self.input_error = Some(self.label("input.rejected"));
                    cx.notify();
                }
            }
        }
    }

    /// Ctrl+click (Command+click on macOS) opens an allowlisted link and does
    /// not move the selection. A refused scheme leaves the click as a selection.
    fn try_open_link(&mut self, event: &gpui::MouseDownEvent, cx: &mut Context<Self>) -> bool {
        if !link_modifier(&event.modifiers) {
            return false;
        }
        let Some(position) = self.hit(event.position, false) else {
            return false;
        };
        let Some(url) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.openable_link(&position))
            .map(str::to_owned)
        else {
            return false;
        };
        match open_external(&url) {
            Ok(()) => self.input_error = None,
            Err(_) => {
                self.input_error = Some(self.label("input.rejected"));
                cx.notify();
            }
        }
        true
    }

    fn mouse_select(
        &mut self,
        point: Point<Pixels>,
        extend: bool,
        clicks: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Reject stale geometry before changing either composition or selection.
        if self.hit(point, extend).is_none() {
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.is_composing())
        {
            self.apply_input(cx, |editor| editor.unmark().map(|_| ()));
        }
        let Some(head) = self.hit(point, extend) else {
            return;
        };
        let Some(editor) = &self.editor else {
            return;
        };
        let anchor = if extend {
            editor.view().selection().anchor
        } else {
            head
        };
        let offset = editor.view().index().offset(head).ok();
        self.apply_input(cx, |editor| {
            if clicks >= 3 {
                editor.select_paragraph_at(offset.ok_or(InputError::InvalidRange)?)
            } else if clicks == 2 {
                editor.select_word_at(offset.ok_or(InputError::InvalidRange)?)
            } else {
                editor.select_positions(Selection { anchor, head })
            }
        });
        window.focus(&self.focus_handle, cx);
        window.invalidate_character_coordinates();
    }
    /// A pending layout keeps the previous anchor; only a current one
    /// replaces it. Called when a layout lands or is reused, not only on
    /// paint: AppKit may ask for the next composition before a frame is drawn.
    pub(crate) fn capture_ime_anchor(&mut self) {
        let anchor = self
            .ready_layout()
            .zip(self.editor.as_ref())
            .map(|(frame, editor)| {
                ImeAnchor::capture(editor.view(), editor.version(), &frame.layout)
            });
        if let Some(anchor) = anchor {
            self.ime_anchor = anchor;
        }
    }
    fn refresh_ime_geometry(&mut self, window: &mut Window) {
        let geometry = self.ready_layout().and_then(|frame| {
            Some(ImeGeometry {
                key: frame.key.clone(),
                zoom: self.zoom_percent,
                viewport: self.viewport_bounds?,
                scroll: self.scroll.offset(),
                selection: self.editor.as_ref()?.view().selection(),
            })
        });
        if geometry != self.ime_geometry {
            self.ime_geometry = geometry;
            if self.ime_geometry.is_some() {
                self.capture_ime_anchor();
                window.invalidate_character_coordinates();
            }
        }
    }
    fn paint_input(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(frame) = self.ready_layout() else {
            return;
        };
        let selection = editor.view().selection();
        let (range, _) = editor.view().selected_range();
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            let index = editor.view().index();
            let rectangles = |range: Range<usize>| {
                let start = index.position(range.start, Bias::After).ok()?;
                let end = index.position(range.end, Bias::Before).ok()?;
                frame.layout.selection_rects(start, end).ok()
            };
            if let Some(rects) = rectangles(range.clone()) {
                for r in rects {
                    if let Some(b) = self.scene_rect(r.page, r.x, r.top, r.width, r.height) {
                        window.paint_quad(fill(b, rgba(0x3984d640)));
                    }
                }
            }
            if let Some(marked) = editor.view().marked_range()
                && let Some(rects) = rectangles(marked)
            {
                for r in rects {
                    if let Some(mut b) = self.scene_rect(r.page, r.x, r.top, r.width, r.height) {
                        b.origin.y += b.size.height - px(1.5);
                        b.size.height = px(1.5);
                        window.paint_quad(fill(b, rgb(0x1f5b9b)));
                    }
                }
            }
            if self.focus_handle.is_focused(window)
                && window.is_window_active()
                && let Ok(c) = frame.layout.caret(selection.head)
                && let Some(mut b) = self.scene_rect(c.page, c.x, c.top, Emu::new(1), c.height)
            {
                b.size.width = px(1.5);
                window.paint_quad(fill(b, rgb(0x163b65)));
            }
        });
    }
    pub(crate) fn input_viewport(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entity = cx.entity();
        let prepaint_entity = entity.clone();
        div()
            .id("input-viewport")
            .relative()
            .flex_1()
            .min_h_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &gpui::MouseDownEvent, window, cx| {
                    if view.try_open_link(event, cx) {
                        return;
                    }
                    view.dragging = view.hit(event.position, false).is_some();
                    view.mouse_select(
                        event.position,
                        event.modifiers.shift,
                        event.click_count,
                        window,
                        cx,
                    );
                }),
            )
            .on_mouse_move(
                cx.listener(|view, event: &gpui::MouseMoveEvent, window, cx| {
                    if view.dragging && event.dragging() {
                        view.mouse_select(event.position, true, 1, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.dragging = false),
            )
            .child(
                div()
                    .id("page-scroll")
                    .size_full()
                    .overflow_scroll()
                    .track_scroll(&self.scroll)
                    .p(px(DOC_PAD))
                    .child(self.canvas()),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        prepaint_entity.update(cx, |view, cx| {
                            // Render ran with the previous size; a resized
                            // viewport may show pages with no pixels yet.
                            if view.viewport_bounds.replace(bounds) != Some(bounds) {
                                view.request_canvas(cx);
                            }
                        });
                    },
                    move |bounds, (), window, cx| {
                        let focus = entity.read(cx).focus_handle.clone();
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                        entity.update(cx, |view, _| {
                            view.refresh_ime_geometry(window);
                            view.paint_input(bounds, window);
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }
}

impl EntityInputHandler for JoyWord {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = None;
        let text = self
            .editor
            .as_ref()?
            .view()
            .index()
            .text_for_range(range.clone())
            .ok()?;
        *adjusted = Some(range);
        Some(text)
    }
    fn selected_text_range(
        &mut self,
        ignore_disabled: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let editor = self.editor.as_ref()?;
        if editor.read_only() && !ignore_disabled {
            return None;
        }
        let (range, reversed) = editor.view().selected_range();
        Some(UTF16Selection { range, reversed })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.editor.as_ref()?.view().marked_range()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_input(cx, |editor| editor.unmark().map(|_| ()));
        window.invalidate_character_coordinates();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_input(cx, |e| e.replace(range, text).map(|_| ()));
        window.invalidate_character_coordinates();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_input(cx, |e| e.mark(range, text, selected));
        window.invalidate_character_coordinates();
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let editor = self.editor.as_ref()?;
        ime_bounds(
            editor.view(),
            editor.version(),
            self.ready_layout().map(|frame| frame.layout.as_ref()),
            self.ime_anchor.as_ref(),
            self.placement()?,
            range,
        )
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let position = self.hit(point, false)?;
        self.editor.as_ref()?.view().index().offset(position).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_copy_applies_only_to_its_session_and_version() {
        let request = CopyRequest {
            epoch: 1,
            version: 3,
        };
        assert_eq!(copy_landing(request, None), CopyLanding::Stale);
        assert_eq!(copy_landing(request, Some((2, 3))), CopyLanding::Stale);
        assert_eq!(copy_landing(request, Some((1, 4))), CopyLanding::Earlier);
        assert_eq!(copy_landing(request, Some((1, 3))), CopyLanding::Current);
    }

    use document_canvas::CanvasPipeline;
    use joyword_engine::Session;
    use joyword_layout::{FaceSet, LayoutSnapshot};
    use joyword_model::ScalarOffset;
    fn pages_source() -> document_canvas::CanvasSource {
        let font = LoadedFont::from_bytes(
            Arc::from(
                include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
            ),
            0,
        )
        .unwrap();
        document_canvas::source_with_text(font, &vec!["AV fi"; 100].join("\n")).unwrap()
    }
    fn editor(source: &document_canvas::CanvasSource) -> TextInput<Session> {
        let session = Session::new(source.session.document().clone());
        TextInput::new(session, InputLimits::default()).unwrap()
    }
    fn layout_of(view: &InputView, source: &document_canvas::CanvasSource) -> LayoutSnapshot {
        joyword_layout::layout_document(
            view.snapshot(),
            FaceSet::uniform(&source.font),
            source.profile,
            Default::default(),
            &Default::default(),
        )
        .unwrap()
    }
    fn placement(zoom: u16, scroll: Point<Pixels>) -> Placement {
        Placement {
            viewport: Bounds::new(point(px(17.0), px(60.0)), size(px(800.0), px(500.0))),
            offset: scroll,
            scale: ScreenScale::new(f64::from(zoom) / 100.0).unwrap(),
        }
    }
    /// UTF-16 offset of the first line placed on the second page.
    fn second_page_start(view: &InputView, layout: &LayoutSnapshot) -> usize {
        let line = layout.lines().iter().find(|line| line.page == 1).unwrap();
        let at = Position {
            story: view.snapshot().body_id(),
            paragraph: line.paragraph,
            offset: ScalarOffset(0),
            bias: Bias::After,
        };
        view.index().offset(at).unwrap()
    }
    fn edges(b: Bounds<Pixels>) -> [f32; 4] {
        [b.origin.x, b.origin.y, b.size.width, b.size.height].map(f32::from)
    }
    fn assert_near(actual: Bounds<Pixels>, expected: [f32; 4]) {
        let actual = edges(actual);
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 0.01, "{actual:?} != {expected:?}");
        }
    }
    #[test]
    fn page_geometry_round_trips_at_zoom_with_live_scroll_on_multiple_pages() {
        let source = pages_source();
        assert!(source.layout.pages().len() > 1);
        for zoom in [50, 80, 100, 150] {
            for offset in [point(px(0.0), px(0.0)), point(px(-50.0), px(-1400.0))] {
                let scale = ScreenScale::new(f64::from(zoom) / 100.0).unwrap();
                for line in source.layout.lines() {
                    let page =
                        project_page(&source.layout, placement(zoom, offset), line.page).unwrap();
                    let stop = line.carets.first().unwrap();
                    let physical = point(
                        page.origin.x + px(scale.logical_pixels(stop.x) as f32),
                        page.origin.y + px(scale.logical_pixels(line.top) as f32),
                    );
                    let (x, y) = local_emu(physical, page, zoom);
                    // Logical pixel f32 precision is bounded before integer EMU conversion.
                    assert!((x.raw() - stop.x.raw()).abs() <= 4);
                    assert!((y.raw() - line.top.raw()).abs() <= 4);
                    let hit = source
                        .layout
                        .hit_test(line.page, x, Emu::new(y.raw() + line.height.raw() / 2))
                        .unwrap();
                    assert_eq!(hit.paragraph, line.paragraph);
                    assert_eq!(hit.offset, stop.offset);
                }
            }
        }
    }
    #[test]
    fn visible_pages_follow_live_scroll_zoom_and_viewport() {
        let source = pages_source();
        let pages = source.layout.pages().len();
        assert!(pages >= 3);
        let shown =
            |zoom, y: f32| visible_pages(&source.layout, placement(zoom, point(px(0.0), px(y))));
        // At 100% an A4 page is 1122.5 px tall: page 0 spans 32–1154.5 of the
        // content, page 1 starts after the 32 px gap at 1186.5. The viewport
        // is 500 px tall.
        assert_eq!(shown(100, 0.0), 0..1);
        assert_eq!(shown(100, -1000.0), 0..2);
        assert_eq!(shown(100, -1160.0), 1..2);
        assert_eq!(shown(50, -1000.0), 1..3);
        assert_eq!(shown(100, -1.0e6), pages - 1..pages);
        // A viewport inside the gap shows no page; the one above stands in.
        let mut gap = placement(100, point(px(0.0), px(-1160.0)));
        gap.viewport.size.height = px(10.0);
        assert_eq!(visible_pages(&source.layout, gap), 0..1);
        // Every page reported meets the viewport, and no other does.
        for y in [0.0, -700.0, -1500.0, -2600.0] {
            let at = placement(80, point(px(0.0), px(y)));
            let range = visible_pages(&source.layout, at);
            for page in 0..pages {
                let b = project_page(&source.layout, at, page).unwrap();
                let meets = b.bottom() >= at.viewport.top() && b.top() <= at.viewport.bottom();
                assert_eq!(range.contains(&page), meets, "page {page} at {y}");
            }
        }
    }
    #[test]
    fn ime_bounds_are_window_logical_pixels_at_zoom_and_live_scroll() {
        let source = pages_source();
        let mut input = editor(&source);
        let layout = layout_of(input.view(), &source);
        // A4 portrait, 90 pt side margins: page and caret positions below are exact.
        let a4 = &layout.pages()[0];
        assert_eq!(
            (a4.width.raw(), a4.height.raw()),
            (11_906 * 635, 16_838 * 635)
        );
        let o = second_page_start(input.view(), &layout);
        input.select(o..o, false).unwrap();
        let line = layout.lines().iter().find(|line| line.page == 1).unwrap();
        assert_eq!(line.carets[0].x, Emu::new(1_800 * 635));
        let version = input.version();
        // Pads and gaps stay 32 px; page height (841.9 pt) and margins follow zoom.
        // The live scroll offset moves the box; device pixels never enter.
        for (zoom, scroll, x, page_y) in [
            (100, point(px(0.0), px(0.0)), 169.0, 1246.5333),
            (80, point(px(0.0), px(0.0)), 145.0, 1022.0267),
            (100, point(px(-50.0), px(-1400.0)), 119.0, -153.4667),
            (80, point(px(-50.0), px(-1400.0)), 95.0, -377.9733),
        ] {
            let placement = placement(zoom, scroll);
            let px_per_pt = (f64::from(zoom) / 100.0 * 96.0 / 72.0) as f32;
            let page = project_page(&layout, placement, 1).unwrap();
            assert_near(
                page,
                [
                    x - 90.0 * px_per_pt,
                    page_y,
                    595.3 * px_per_pt,
                    841.9 * px_per_pt,
                ],
            );
            let b =
                ime_bounds(input.view(), version, Some(&layout), None, placement, o..o).unwrap();
            let top = page_y + line.top.points() as f32 * px_per_pt;
            assert_near(b, [x, top, 1.0, line.height.points() as f32 * px_per_pt]);
            // Same space as mouse hit testing: the box maps back to its position.
            let (ex, ey) = local_emu(b.center(), page, zoom);
            let hit = layout.hit_test(1, ex, ey).unwrap();
            assert_eq!(input.view().index().offset(hit).unwrap(), o);
        }
    }
    #[test]
    fn pending_preedit_layout_answers_from_the_origin_anchor() {
        let source = pages_source();
        let mut input = editor(&source);
        let idle = layout_of(input.view(), &source);
        // Insert at a paragraph start: kerning cannot move the origin caret.
        let o = second_page_start(input.view(), &idle);
        input.select(o..o, false).unwrap();
        let version = input.version();
        let anchor = ImeAnchor::capture(input.view(), version, &idle).unwrap();
        let scroll = point(px(-50.0), px(-1400.0));
        let query = |view: &InputView, layout, anchor, zoom, scroll, range| {
            ime_bounds(
                view,
                version,
                layout,
                anchor,
                placement(zoom, scroll),
                range,
            )
        };
        let idle_100 = query(input.view(), Some(&idle), None, 100, scroll, o..o).unwrap();
        let idle_80 = query(input.view(), Some(&idle), None, 80, scroll, o..o).unwrap();
        input.mark(None, "AV", None).unwrap();
        let view = input.view();
        assert_eq!(view.marked_range(), Some(o..o + 2));
        // Without the anchor the pending preedit had no box: AppKit then used the screen origin.
        assert_eq!(query(view, None, None, 100, scroll, o..o), None);
        let pending = |zoom, scroll, range| query(view, None, Some(&anchor), zoom, scroll, range);
        for range in [o..o, o..o + 2, o + 1..o + 2, o + 2..o + 2] {
            assert_eq!(pending(100, scroll, range), Some(idle_100));
        }
        assert_eq!(pending(80, scroll, o..o), Some(idle_80));
        let scrolled = pending(100, point(px(-50.0), px(-1430.0)), o..o).unwrap();
        let mut moved = idle_100;
        moved.origin.y -= px(30.0);
        assert_near(scrolled, edges(moved));
        for outside in [o - 1..o - 1, o + 3..o + 3, o..o + 3] {
            assert_eq!(pending(100, scroll, outside), None);
        }
        // Once the preedit's own layout lands, it answers exactly and agrees with the anchor.
        let preedit = layout_of(view, &source);
        let current = |range| query(view, Some(&preedit), Some(&anchor), 100, scroll, range);
        assert_eq!(current(o..o), Some(idle_100));
        assert_eq!(ImeAnchor::capture(view, version, &preedit), Some(anchor));
        let caret = current(o + 2..o + 2).unwrap();
        assert!(caret.origin.x > idle_100.origin.x);
        assert_eq!(caret.origin.y, idle_100.origin.y);
    }
    #[test]
    fn pending_preedit_reveals_its_origin_before_appkit_asks() {
        let source = pages_source();
        let mut input = editor(&source);
        let idle = layout_of(input.view(), &source);
        let o = second_page_start(input.view(), &idle);
        input.select(o..o, false).unwrap();
        let version = input.version();
        let anchor = ImeAnchor::capture(input.view(), version, &idle).unwrap();
        // Scrolled to the top, so the origin on page 2 is below the viewport.
        let away = placement(100, point(px(0.0), px(0.0)));
        input.mark(None, "AV", None).unwrap();
        let view = input.view();
        let asked = |at| ime_bounds(view, version, None, Some(&anchor), at, o + 1..o + 2);
        assert!(asked(away).unwrap().top() > away.viewport.bottom());
        assert_eq!(reveal_target(view, version, None, None), None);
        let (origin, owed) = reveal_target(view, version, None, Some(&anchor)).unwrap();
        assert_eq!((origin, owed), (anchor.caret, true));
        let content = content_size(&idle, away.scale);
        let offset = reveal_offset(away.viewport, away.project(origin), away.offset, content);
        let shown = Placement { offset, ..away };
        let b = asked(shown).unwrap();
        assert!(shown.viewport.top() <= b.top() && b.bottom() <= shown.viewport.bottom());
        // The exact reveal of the head follows once the preedit's layout lands.
        let preedit = layout_of(view, &source);
        let head = SceneBox::caret(&preedit, view.selection().head).unwrap();
        let exact = reveal_target(view, version, Some(&preedit), Some(&anchor));
        assert_eq!(exact, Some((head, false)));
    }
    /// Only the layout stage lands; no raster job runs.
    fn layout_only(
        pipeline: &mut CanvasPipeline,
        view: &InputView,
        source: &document_canvas::CanvasSource,
    ) -> Arc<LayoutSnapshot> {
        let key = pipeline.key(view, &source.font, source.profile);
        let job = pipeline.request_layout(key.clone()).unwrap();
        let previous = pipeline.previous_layout(&source.font.key(), source.profile);
        let result = document_canvas::layout_view_incremental(
            view,
            &source.font,
            source.profile,
            &job.cancellation,
            &pipeline.paragraph_cache(),
            previous.as_deref(),
        );
        assert!(matches!(
            pipeline.install_layout(&job.ticket, &key, result),
            Landed::Installed(_)
        ));
        pipeline.ready_layout(&key).unwrap().layout.clone()
    }
    #[test]
    fn a_layout_without_pixels_answers_ime_hit_testing_and_reveal_exactly() {
        let source = pages_source();
        let mut input = editor(&source);
        let mut pipeline = CanvasPipeline::default();
        let idle = layout_only(&mut pipeline, input.view(), &source);
        let o = second_page_start(input.view(), &idle);
        input.select(o..o, false).unwrap();
        assert!(pipeline.reuse(input.view(), &source.font, source.profile));
        input.mark(None, "AV", None).unwrap();
        let version = input.version();
        let view = input.view();
        let key = pipeline.key(view, &source.font, source.profile);
        // Positions wait while the preedit's layout is pending; the old layout
        // still gives page boxes but maps nothing.
        assert!(pipeline.ready_layout(&key).is_none());
        assert!(pipeline.layout().is_some());
        let ready = layout_only(&mut pipeline, view, &source);
        let at_80 = joyword_render::Projection::new(96.0, 0.8).unwrap();
        assert!(pipeline.ready_page(&key, 0, at_80).is_none());
        assert_eq!(pipeline.resident_pixels(), 0);
        // Exactly what a directly computed layout of this view answers.
        let direct = layout_of(view, &source);
        // Scrolled so the composition is inside the viewport.
        let at = placement(80, point(px(-50.0), px(-1000.0)));
        let query = |layout, range| ime_bounds(view, version, Some(layout), None, at, range);
        for range in [o..o, o..o + 2, o + 1..o + 2, o + 2..o + 2] {
            let b = query(&ready, range.clone());
            assert!(b.is_some());
            assert_eq!(b, query(&direct, range));
        }
        // The box now follows the preedit end, not only the origin anchor.
        let origin = query(&ready, o..o).unwrap();
        let end = query(&ready, o + 2..o + 2).unwrap();
        assert!(end.origin.x > origin.origin.x);
        assert_eq!(end.origin.y, origin.origin.y);
        // Hit testing and character_index_for_point map through it too.
        for (b, offset) in [(origin, o), (end, o + 2)] {
            let hit = hit_at(&ready, at, 80, b.center(), false).unwrap();
            assert_eq!(view.index().offset(hit).unwrap(), offset);
        }
        assert_eq!(hit_at(&ready, at, 80, point(px(0.0), px(0.0)), false), None);
        // Reveal is exact at once; nothing is owed to a later frame.
        let head = SceneBox::caret(&ready, view.selection().head).unwrap();
        assert_eq!(
            reveal_target(view, version, Some(&ready), None),
            Some((head, false))
        );
    }
    #[test]
    fn commit_then_type_answers_from_the_committed_layout_before_its_pixels() {
        let source = pages_source();
        let mut input = editor(&source);
        let mut pipeline = CanvasPipeline::default();
        let idle = layout_only(&mut pipeline, input.view(), &source);
        let o = second_page_start(input.view(), &idle);
        input.select(o..o, false).unwrap();
        assert!(pipeline.reuse(input.view(), &source.font, source.profile));
        input.mark(None, "fi", None).unwrap();
        let before = input.version();
        let preview = layout_only(&mut pipeline, input.view(), &source);
        let previous = ImeAnchor::capture(input.view(), before, &preview).unwrap();
        // The commit differs from the preedit, as hanzi differ from pinyin.
        input.replace(None, "AV\n").unwrap();
        let committed = input.version();
        assert_ne!(committed, before);
        assert!(!pipeline.reuse(input.view(), &source.font, source.profile));
        // Its layout lands, without pixels, before the next syllable.
        let landed = layout_only(&mut pipeline, input.view(), &source);
        let anchor = ImeAnchor::capture(input.view(), committed, &landed).unwrap();
        input.mark(None, "fi", None).unwrap();
        let view = input.view();
        let origin = o + 3;
        assert_eq!(view.marked_range(), Some(origin..origin + 2));
        let at = placement(100, point(px(-50.0), px(-1400.0)));
        let asked = origin + 1..origin + 2;
        // The previous composition's anchor no longer answers.
        let pending = |anchor| ime_bounds(view, committed, None, Some(anchor), at, asked.clone());
        assert_eq!(pending(&previous), None);
        // The committed layout puts the bar at the new origin, a line lower.
        let answered = pending(&anchor).unwrap();
        assert!(answered.origin.y > at.project(previous.caret).origin.y);
        let exact = layout_of(view, &source);
        let start = ime_bounds(view, committed, Some(&exact), None, at, origin..origin);
        assert_eq!(Some(answered), start);
    }
    #[test]
    fn origin_anchor_is_dropped_for_other_versions_or_origins_and_queries_clamp_to_end() {
        let source = pages_source();
        let mut input = editor(&source);
        let idle = layout_of(input.view(), &source);
        let at = placement(100, point(px(0.0), px(0.0)));
        let version = input.version();
        let anchor = ImeAnchor::capture(input.view(), version, &idle).unwrap();
        input.mark(None, "fi", None).unwrap();
        assert!(ime_bounds(input.view(), version, None, Some(&anchor), at, 0..0).is_some());
        assert_eq!(
            ime_bounds(input.view(), version + 1, None, Some(&anchor), at, 0..0),
            None
        );
        input.unmark().unwrap();
        let committed = input.version();
        assert_ne!(committed, version);
        assert_eq!(
            ime_bounds(input.view(), committed, None, Some(&anchor), at, 2..2),
            None
        );
        // Same version number, moved origin: a new composition after the commit.
        let moved = ImeAnchor {
            version: committed,
            ..anchor
        };
        input.mark(None, "AV", None).unwrap();
        assert_eq!(
            ime_bounds(input.view(), committed, None, Some(&moved), at, 2..2),
            None
        );
        input.cancel_composition().unwrap();
        let current = layout_of(input.view(), &source);
        let len = input.view().index().len_utf16();
        let query = |range| ime_bounds(input.view(), committed, Some(&current), None, at, range);
        let end = query(len..len).unwrap();
        assert_eq!(query(len..len + 1), Some(end));
        assert_eq!(query(len + 4..len + 9), Some(end));
    }
    #[test]
    fn caret_reveal_scrolls_minimally_and_clamps_at_content_edges() {
        let viewport = Bounds::new(point(px(20.0), px(60.0)), size(px(500.0), px(400.0)));
        let content = size(px(1000.0), px(2000.0));
        let offset = point(px(-40.0), px(-200.0));
        let inside = Bounds::new(point(px(50.0), px(100.0)), size(px(1.0), px(24.0)));
        assert_eq!(reveal_offset(viewport, inside, offset, content), offset);
        let below = Bounds::new(point(px(550.0), px(470.0)), size(px(1.0), px(24.0)));
        assert_eq!(
            reveal_offset(viewport, below, offset, content),
            point(px(-79.0), px(-242.0))
        );
        let above = Bounds::new(point(px(-1000.0), px(-1000.0)), size(px(1.0), px(24.0)));
        assert_eq!(
            reveal_offset(viewport, above, offset, content),
            point(px(0.0), px(0.0))
        );
        let far = Bounds::new(point(px(10000.0), px(10000.0)), size(px(1.0), px(24.0)));
        assert_eq!(
            reveal_offset(viewport, far, offset, content),
            point(px(-500.0), px(-1600.0))
        );
    }
    #[test]
    fn original_save_prompt_defaults_to_copy_and_rechecks_edited_versions() {
        let version = 7;
        for answer in [None, Some(0), Some(3), Some(usize::MAX)] {
            assert_eq!(
                original_decision(answer, version, version),
                OriginalDecision::Cancel
            );
        }
        assert_eq!(
            original_decision(Some(1), version, version + 1),
            OriginalDecision::SaveCopy
        );
        assert_eq!(
            original_decision(Some(2), version, version),
            OriginalDecision::Confirm
        );
        assert_eq!(
            original_decision(Some(2), version, version + 1),
            OriginalDecision::Reprobe
        );
    }

    #[test]
    fn only_explicit_discard_for_the_prompted_version_can_close() {
        let v = 4;
        assert_eq!(close_decision(None, v, v), CloseDecision::Cancel);
        for answer in [0, 2, usize::MAX] {
            assert_eq!(close_decision(Some(answer), v, v), CloseDecision::Cancel);
        }
        assert_eq!(close_decision(Some(1), v, v), CloseDecision::Discard);
        assert_eq!(close_decision(Some(1), v, 5), CloseDecision::AskAgain);
    }
}
