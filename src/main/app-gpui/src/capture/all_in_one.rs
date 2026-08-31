//! All-in-one — port of `capture/all-in-one` and
//! `renderer/components/area-overlay/all-in-one-toolbar.tsx`. One overlay that
//! switches between screenshot, recording and OCR over area, window or screen,
//! remembering the last choice when the setting is on.

use crate::config::store::ConfigStore;
use crate::system::capabilities::{is_supported, Feature};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Screenshot,
    Record,
    Ocr,
}

impl Mode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Screenshot => "screenshot",
            Self::Record => "record",
            Self::Ocr => "ocr",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "record" => Self::Record,
            "ocr" => Self::Ocr,
            _ => Self::Screenshot,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Screenshot => "camera",
            Self::Record => "video",
            Self::Ocr => "scan-text",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Target {
    #[default]
    Area,
    Window,
    Screen,
}

impl Target {
    pub const ALL: [Target; 3] = [Self::Area, Self::Window, Self::Screen];

    pub fn id(self) -> &'static str {
        match self {
            Self::Area => "area",
            Self::Window => "window",
            Self::Screen => "screen",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "window" => Self::Window,
            "screen" => Self::Screen,
            _ => Self::Area,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Area => "Area",
            Self::Window => "Window",
            Self::Screen => "Full screen",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Area => "square-dashed",
            Self::Window => "app-window",
            Self::Screen => "monitor",
        }
    }

    pub fn is_supported(self) -> bool {
        match self {
            Self::Window => is_supported(Feature::ScreenshotWindow),
            Self::Screen => is_supported(Feature::ScreenshotScreen),
            Self::Area => is_supported(Feature::ScreenshotArea),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Choices {
    pub mode: Mode,
    pub target: Target,
}

/// Port of `allInOne.rememberChoices`: the last mode and target are restored
/// only when the setting is on.
pub fn restore(store: &ConfigStore) -> Choices {
    let config = store.get();
    if !config.all_in_one.remember_choices {
        return Choices::default();
    }
    let mode = match Mode::parse(&config.all_in_one.last_mode) {
        Mode::Ocr => Mode::Screenshot,
        Mode::Record if !is_supported(Feature::Recording) => Mode::Screenshot,
        mode => mode,
    };
    let restored_target = match mode {
        Mode::Record => Target::parse(&config.all_in_one.last_targets.record),
        _ => Target::parse(&config.all_in_one.last_targets.screenshot),
    };
    let target = if restored_target.is_supported() {
        restored_target
    } else {
        Target::Area
    };
    Choices { mode, target }
}

pub fn remember(store: &ConfigStore, choices: Choices) {
    if choices.mode == Mode::Ocr || !store.get().all_in_one.remember_choices {
        return;
    }
    store.update(move |config| {
        config.all_in_one.last_mode = choices.mode.id().to_string();
        match choices.mode {
            Mode::Record => config.all_in_one.last_targets.record = choices.target.id().to_string(),
            _ => config.all_in_one.last_targets.screenshot = choices.target.id().to_string(),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> ConfigStore {
        ConfigStore::load_at(std::env::temp_dir().join(format!(
            "poratake-all-in-one-test-{}.json",
            std::process::id()
        )))
        .expect("store")
    }

    #[test]
    fn ids_round_trip() {
        for mode in [Mode::Screenshot, Mode::Record, Mode::Ocr] {
            assert_eq!(Mode::parse(mode.id()), mode);
        }
        for target in Target::ALL {
            assert_eq!(Target::parse(target.id()), target);
        }
    }

    #[test]
    fn choices_are_only_restored_when_the_setting_is_on() {
        let store = store();
        store.update(|config| {
            config.all_in_one.remember_choices = false;
            config.all_in_one.last_mode = "record".into();
            config.all_in_one.last_targets.record = "screen".into();
        });
        assert_eq!(restore(&store), Choices::default());

        store.update(|config| config.all_in_one.remember_choices = true);
        let expected = if crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::Recording,
        ) {
            Choices {
                mode: Mode::Record,
                target: Target::Screen,
            }
        } else {
            Choices::default()
        };
        assert_eq!(restore(&store), expected);
    }

    #[test]
    fn ocr_is_never_restored_or_remembered() {
        let store = store();
        store.update(|config| {
            config.all_in_one.remember_choices = true;
            config.all_in_one.last_mode = "ocr".into();
        });

        assert_eq!(restore(&store), Choices::default());

        remember(
            &store,
            Choices {
                mode: Mode::Ocr,
                target: Target::Area,
            },
        );
        assert_eq!(store.get().all_in_one.last_mode, "ocr");
    }

    #[test]
    fn record_and_screenshot_targets_are_remembered_separately() {
        let store = store();
        store.update(|config| {
            config.all_in_one.remember_choices = true;
            config.all_in_one.last_targets.screenshot = "area".into();
            config.all_in_one.last_targets.record = "area".into();
        });

        remember(
            &store,
            Choices {
                mode: Mode::Record,
                target: Target::Window,
            },
        );
        let config = store.get();
        assert_eq!(config.all_in_one.last_targets.record, "window");
        assert_eq!(config.all_in_one.last_targets.screenshot, "area");
    }
}
