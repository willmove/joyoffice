//! GPUI exposes plain text and images on Linux. Read rich MIME offers using
//! the clipboard helper for the compositor that actually owns the window.
//! Preserve `Some(empty)` from hidden RTF to stop lower-priority representations.
use joydeck_model::TextBody;
use joydeck_shell::clipboard_import::{self, MAX_INPUT};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(crate) fn rich_text(compositor: &str) -> Option<TextBody> {
    let deadline = Instant::now() + Duration::from_millis(400);
    let (helper, args, list, target) = match compositor {
        "X11" => (
            "xclip",
            &["-selection", "clipboard", "-out"][..],
            &["-target", "TARGETS"][..],
            "-target",
        ),
        "Wayland" => (
            "wl-paste",
            &["--no-newline"][..],
            &["--list-types"][..],
            "--type",
        ),
        _ => return None,
    };
    let bytes = read(Command::new(helper).args(args).args(list), deadline)?;
    let offers = std::str::from_utf8(&bytes).ok()?;
    import_offer(offers, |mime| {
        read(
            Command::new(helper).args(args).args([target, mime]),
            deadline,
        )
    })
}

fn import_offer(offers: &str, mut read: impl FnMut(&str) -> Option<Vec<u8>>) -> Option<TextBody> {
    let types = offers.lines().map(str::trim);
    for (kind, import) in [
        (
            "text/html",
            clipboard_import::import_html as fn(&[u8]) -> Option<TextBody>,
        ),
        ("text/rtf", clipboard_import::import_rtf_for_paste),
        ("application/rtf", clipboard_import::import_rtf_for_paste),
        ("application/x-rtf", clipboard_import::import_rtf_for_paste),
    ] {
        // Prefer the canonical target, then try provider-specific parameters.
        let canonical = types.clone().find(|mime| *mime == kind);
        let aliases = types.clone().filter(|mime| {
            mime.split_once(';')
                .is_some_and(|(base, _)| base.trim_end() == kind)
        });
        for mime in canonical.into_iter().chain(aliases) {
            if let Some(body) = read(mime)
                .filter(|bytes| bytes.len() <= MAX_INPUT)
                .and_then(|bytes| import(&bytes))
            {
                return Some(body);
            }
        }
    }
    None
}

fn read(command: &mut Command, deadline: Instant) -> Option<Vec<u8>> {
    if Instant::now() >= deadline {
        return None;
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let result = (|| {
        let mut stdout = child.stdout.take()?;
        let mut bytes = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let timeout =
                Timespec::try_from(deadline.checked_duration_since(Instant::now())?).ok()?;
            if poll(&mut [PollFd::new(&stdout, PollFlags::IN)], Some(&timeout)).ok()? == 0 {
                return None;
            }
            let count = stdout.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            if bytes.len() + count > MAX_INPUT {
                return None;
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        loop {
            if let Some(status) = child.try_wait().ok()? {
                return status.success().then_some(bytes);
            }
            let remaining = deadline.checked_duration_since(Instant::now())?;
            thread::sleep(remaining.min(Duration::from_millis(2)));
        }
    })();
    if result.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "Test assertions")]

    use super::*;
    use joydeck_engine::{Session, clipboard, original_bindings};
    use joydeck_model::Presentation;

    const RTF: &[u8] = br"{\rtf1\ansi\uc1{\i \u20013?\u-10179?\u-8704?}}";

    #[test]
    fn html_takes_priority_and_pastes_as_one_canvas_transaction() {
        let body = import_offer("application/rtf\ntext/html;charset=utf-8\n", |mime| {
            assert_eq!(mime, "text/html;charset=utf-8");
            Some(
                r#"<b>中文😀</b><a href="javascript:alert(1)">unsafe</a>"#
                    .as_bytes()
                    .to_vec(),
            )
        })
        .expect("HTML");
        assert_eq!(body.text(), "中文😀unsafe");
        let runs = &body.paragraphs[0].runs;
        assert_eq!(runs[0].props.bold, Some(true));
        assert!(runs.iter().all(|run| run.props.link.is_none()));

        let model = Presentation::default();
        let mut session = Session::new(model.clone(), original_bindings(&model), true);
        assert!(session.execute(clipboard::text_box(body)).expect("paste"));
        assert_eq!(session.revision(), 1);
        assert!(
            session
                .execute(joydeck_engine::Command::Undo)
                .expect("undo")
        );
        assert_eq!(session.model(), &model);
        assert!(!session.can_undo());
        assert!(
            session
                .execute(joydeck_engine::Command::Redo)
                .expect("redo")
        );
    }

    #[test]
    fn unusable_html_falls_back_to_rtf_and_unusable_rich_leaves_plain_text_to_gpui() {
        for html in [
            b"<script>hidden</script>".to_vec(),
            vec![b'x'; MAX_INPUT + 1],
        ] {
            let body = import_offer("text/html\napplication/rtf\ntext/plain\n", |mime| {
                Some(if mime == "text/html" {
                    html.clone()
                } else {
                    RTF.to_vec()
                })
            })
            .expect("RTF fallback");
            assert_eq!(body.text(), "中😀");
            assert_eq!(body.paragraphs[0].runs[0].props.italic, Some(true));
        }
        assert!(
            import_offer("text/html\ntext/rtf\ntext/plain\n", |mime| {
                assert_ne!(mime, "text/plain");
                Some(if mime == "text/html" {
                    vec![0xff]
                } else {
                    br"{\rtf1 unclosed".to_vec()
                })
            })
            .is_none()
        );
        assert!(
            import_offer("text/plain;charset=utf-8\n", |_| panic!(
                "plain text belongs to GPUI"
            ))
            .is_none()
        );
    }

    #[test]
    fn hidden_rtf_stops_alias_and_plain_text_fallback() {
        for kind in [
            "text/rtf",
            "application/rtf",
            "application/x-rtf",
            "text/rtf;charset=utf-8",
        ] {
            let offers = format!("{kind}\napplication/x-rtf;alternative=yes\ntext/plain\n");
            let body = import_offer(&offers, |mime| {
                assert_eq!(
                    mime, kind,
                    "must not try another representation after hidden RTF"
                );
                Some(br"{\rtf1\v secret}".to_vec())
            })
            .expect("hidden RTF consumes paste");
            assert!(body.is_empty());
        }
    }

    #[test]
    fn helper_output_is_bounded_and_failed_or_stalled_helpers_are_discarded() {
        let deadline = || Instant::now() + Duration::from_secs(3);
        let mut exact = Command::new("head");
        exact.args(["-c", &MAX_INPUT.to_string(), "/dev/zero"]);
        assert_eq!(
            read(&mut exact, deadline()).expect("bounded output").len(),
            MAX_INPUT
        );

        let mut oversized = Command::new("head");
        oversized.args(["-c", &(MAX_INPUT + 1).to_string(), "/dev/zero"]);
        assert!(read(&mut oversized, deadline()).is_none());

        assert!(read(&mut Command::new("false"), deadline()).is_none());
        let start = Instant::now();
        assert!(
            read(
                Command::new("sleep").arg("10"),
                start + Duration::from_millis(50)
            )
            .is_none()
        );
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
