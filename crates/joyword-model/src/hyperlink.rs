//! Hyperlinks (ECMA-376 Part 1 §17.16.22 `w:hyperlink`).
//!
//! An external target is the relationship's `Target` text. Nothing here
//! fetches it, follows a redirect, or treats the string as a command.
//! Opening is a separate decision with a scheme allowlist.

use std::sync::Arc;

/// Longest target or anchor this slice will store from an edit.
pub const MAX_HYPERLINK_CHARS: usize = 2_048;

/// A hyperlink shared by one or more runs.
///
/// `relationship` is the package `r:id` when one already exists. It is not
/// part of the semantic target: two links with the same external string and
/// anchor are the same link even if a later export assigns a new id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Hyperlink {
    pub relationship: Option<Arc<str>>,
    pub external: Option<Arc<str>>,
    pub anchor: Option<Arc<str>>,
}

/// Why a stored target must not be handed to the platform opener.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenRefusal {
    /// Bookmark-only links are not URLs.
    NoExternalTarget,
    Scheme,
    Malformed,
    TooLong,
}

impl Hyperlink {
    /// A target an editor command may store. External targets must already
    /// pass the open allowlist. At least one of the two parts is required.
    pub fn for_edit(external: Option<&str>, anchor: Option<&str>) -> Result<Self, &'static str> {
        let external = clean_part(external)?;
        let anchor = clean_part(anchor)?;
        if external.is_none() && anchor.is_none() {
            return Err("hyperlink needs a target or an anchor");
        }
        let link = Self {
            relationship: None,
            external,
            anchor,
        };
        if link.external.is_some() && open_target(&link).is_err() {
            return Err("hyperlink scheme is not allowed");
        }
        Ok(link)
    }

    /// An imported target. The external string is kept even when opening it
    /// would be refused. Empty parts are dropped.
    pub fn imported(
        relationship: Option<Arc<str>>,
        external: Option<Arc<str>>,
        anchor: Option<Arc<str>>,
    ) -> Option<Self> {
        let external = external.filter(|value| !value.is_empty());
        let anchor = anchor.filter(|value| !value.is_empty());
        if external.is_none() && anchor.is_none() {
            return None;
        }
        Some(Self {
            relationship,
            external,
            anchor,
        })
    }
}

fn clean_part(value: Option<&str>) -> Result<Option<Arc<str>>, &'static str> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > MAX_HYPERLINK_CHARS {
        return Err("hyperlink is too long");
    }
    if value
        .chars()
        .any(|ch| ch.is_control() || ch == '<' || ch == '>' || ch == '"')
    {
        return Err("hyperlink contains a character that cannot be stored");
    }
    Ok(Some(Arc::from(value)))
}

/// The external target when its scheme is `http`, `https`, or `mailto`.
/// Any other scheme, including `javascript`, `data`, and `file`, is refused.
/// The returned string is the stored target, not a rewritten URL.
pub fn open_target(link: &Hyperlink) -> Result<&str, OpenRefusal> {
    let Some(target) = link.external.as_deref() else {
        return Err(OpenRefusal::NoExternalTarget);
    };
    if target.chars().count() > MAX_HYPERLINK_CHARS {
        return Err(OpenRefusal::TooLong);
    }
    if target
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\' || ch == '<' || ch == '>')
    {
        return Err(OpenRefusal::Malformed);
    }
    let Some((scheme, rest)) = target.split_once(':') else {
        return Err(OpenRefusal::Scheme);
    };
    if scheme.is_empty()
        || !scheme.bytes().all(|byte| byte.is_ascii_alphabetic())
        || rest.is_empty()
    {
        return Err(OpenRefusal::Scheme);
    }
    match scheme_lower(scheme).as_deref() {
        Some("http") | Some("https") => {
            let Some(after) = rest.strip_prefix("//") else {
                return Err(OpenRefusal::Malformed);
            };
            let host = after.split(['/', '?', '#']).next().unwrap_or("");
            if host.is_empty() || host.contains('@') || host.starts_with('.') {
                return Err(OpenRefusal::Malformed);
            }
            Ok(target)
        }
        Some("mailto") => {
            if rest.starts_with("//") || rest.contains(':') || !rest.contains('@') {
                return Err(OpenRefusal::Malformed);
            }
            Ok(target)
        }
        _ => Err(OpenRefusal::Scheme),
    }
}

fn scheme_lower(scheme: &str) -> Option<String> {
    if scheme.len() > 32 {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}
