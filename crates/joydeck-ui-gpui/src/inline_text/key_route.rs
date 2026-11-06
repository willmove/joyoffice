//! Route character-producing modifier chords before canvas editing shortcuts.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum KeyRoute {
    NativeText,
    Command,
    Plain,
}

pub(super) fn route(
    prefer_character_input: bool,
    control: bool,
    platform: bool,
    alt: bool,
) -> KeyRoute {
    let command = control || platform;
    if prefer_character_input || (command && alt) {
        KeyRoute::NativeText
    } else if command {
        KeyRoute::Command
    } else {
        KeyRoute::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_character_preference_overrides_every_modifier_combination() {
        for control in [false, true] {
            for platform in [false, true] {
                for alt in [false, true] {
                    assert_eq!(route(true, control, platform, alt), KeyRoute::NativeText);
                }
            }
        }
    }

    #[test]
    fn alt_modified_commands_never_fall_through_to_canvas_shortcuts() {
        // Some backends expose AltGr as Ctrl+Alt without a character-preference hint.
        assert_eq!(route(false, true, false, true), KeyRoute::NativeText);
        assert_eq!(route(false, false, true, true), KeyRoute::NativeText);
        assert_eq!(route(false, true, true, true), KeyRoute::NativeText);
    }

    #[test]
    fn standard_commands_and_unmodified_navigation_keep_their_routes() {
        assert_eq!(route(false, true, false, false), KeyRoute::Command);
        assert_eq!(route(false, false, true, false), KeyRoute::Command);
        assert_eq!(route(false, false, false, false), KeyRoute::Plain);
        assert_eq!(route(false, false, false, true), KeyRoute::Plain);
    }
}
