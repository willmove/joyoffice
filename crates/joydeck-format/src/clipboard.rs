//! Bounded import of editable text fragments from other applications.
mod html;
mod rtf;
#[cfg(test)]
mod tests;

pub use html::import as import_html;
pub use rtf::{import as import_rtf, import_for_paste as import_rtf_for_paste};

use joydeck_model::{
    ColorRef, Fill, Paragraph, Rgba, TextBody,
    text::{Action, Hyperlink, Run, RunKind, RunProps, safe_url},
};

pub const MAX_INPUT: usize = 1024 * 1024;
const MAX_TEXT: usize = 64 * 1024;
const MAX_RUNS: usize = 8192;
const MAX_PARAGRAPHS: usize = 1024;
const MAX_DEPTH: usize = 128;

#[derive(Default)]
struct Fragment {
    body: TextBody,
    current: Paragraph,
    bytes: usize,
    runs: usize,
    overflow: bool,
}

impl Fragment {
    fn text(&mut self, value: &str, props: &RunProps) {
        let text: String = value
            .chars()
            .filter(|&c| !c.is_control() || c == '\t')
            .collect();
        if text.is_empty() || self.overflow {
            return;
        }
        self.bytes += text.len();
        if self.bytes > MAX_TEXT {
            self.overflow = true;
        } else if let Some(last) = self.current.runs.last_mut()
            && last.kind == RunKind::Text
            && last.props == *props
        {
            last.text.push_str(&text);
        } else {
            self.run(Run::with(text, props.clone()));
        }
    }

    fn run(&mut self, run: Run) {
        self.runs += 1;
        if self.runs > MAX_RUNS || self.bytes > MAX_TEXT {
            self.overflow = true;
        }
        if !self.overflow {
            self.current.runs.push(run);
        }
    }

    fn soft_break(&mut self, props: &RunProps) {
        self.bytes += 1;
        self.run(Run {
            kind: RunKind::Break,
            ..Run::with("", props.clone())
        });
    }

    fn paragraph(&mut self) {
        if self.body.paragraphs.len() + 1 >= MAX_PARAGRAPHS || self.bytes >= MAX_TEXT {
            self.overflow = true;
        } else if !self.overflow {
            self.bytes += 1;
            self.body.paragraphs.push(std::mem::take(&mut self.current));
        }
    }

    fn finish(mut self) -> Option<TextBody> {
        if !self.current.runs.is_empty() {
            self.body.paragraphs.push(self.current);
        }
        (!self.overflow && !self.body.is_empty()).then_some(self.body)
    }
}

fn url_link(url: &str) -> Option<Hyperlink> {
    let url = url.trim();
    (url.len() <= 2048
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && safe_url(url))
    .then(|| Hyperlink {
        action: Action::Url { url: url.into() },
        tooltip: String::new(),
        highlight_click: false,
        sound: None,
        stop_sound: false,
        zoom_return: None,
    })
}

fn color_fill(rgb: Rgba) -> Fill {
    Fill::solid(ColorRef::rgb(rgb))
}
