#![no_std]

use serde::{Deserialize, Serialize};
use watchos_protocol::{
    Appearance, ComplicationAccent, ComplicationKind, ComplicationStyle, ConnectionState, Face,
    Palette,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Screen {
    #[default]
    Boot,
    Home,
    Launcher,
    Assistant,
    Notifications,
    QuickSettings,
    Connection,
    Settings,
    DeviceInfo,
    FacePicker = 9,
    ThemePicker = 10,
    Appearance = 11,
    Health = 12,
    Complications = 13,
    SlotPicker = 14,
    MetricDetail = 15,
    Utilities = 16,
    IconGallery = 17,
    SlotAppearance = 18,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub use_24h: bool,
    pub quiet: bool,
    pub reduce_motion: bool,
    pub brightness: u8,
    #[serde(default)]
    pub appearance: Appearance,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            use_24h: true,
            quiet: false,
            reduce_motion: false,
            brightness: 80,
            appearance: Appearance::default(),
        }
    }
}

impl Settings {
    pub fn is_valid(&self) -> bool {
        (20..=100).contains(&self.brightness) && self.appearance.is_valid()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum DataState {
    Live = 0,
    #[default]
    Unavailable = 1,
    PermissionRequired = 2,
    Disconnected = 3,
    Stale = 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub status: DataState,
    pub value: Option<u32>,
}

impl Reading {
    pub fn is_valid(&self) -> bool {
        self.value.is_some() == matches!(self.status, DataState::Live | DataState::Stale)
    }

    pub fn is_valid_for(&self, kind: ComplicationKind) -> bool {
        self.is_valid()
            && match kind {
                ComplicationKind::HeartRate => self.value.is_none_or(|value| value > 0),
                ComplicationKind::Battery | ComplicationKind::BloodOxygen => {
                    self.value.is_none_or(|value| value <= 100)
                }
                _ => true,
            }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Telemetry {
    pub heart_rate: Reading,
    pub steps: Reading,
    pub battery: Reading,
    pub metrics: [Reading; 22],
}

impl Telemetry {
    pub fn reading(&self, kind: ComplicationKind) -> Reading {
        match kind {
            ComplicationKind::HeartRate => self.heart_rate,
            ComplicationKind::Steps => self.steps,
            ComplicationKind::Battery => self.battery,
            _ => self.metrics[usize::from(kind as u8) - 3],
        }
    }

    pub fn is_valid(&self) -> bool {
        ComplicationKind::ALL
            .iter()
            .all(|kind| self.reading(*kind).is_valid_for(*kind))
    }

    fn set_reading(&mut self, kind: ComplicationKind, reading: Reading) {
        match kind {
            ComplicationKind::HeartRate => self.heart_rate = reading,
            ComplicationKind::Steps => self.steps = reading,
            ComplicationKind::Battery => self.battery = reading,
            _ => self.metrics[usize::from(kind as u8) - 3] = reading,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandAngles {
    pub hour: f32,
    pub minute: f32,
    pub second: f32,
}

/// Degrees clockwise from twelve, including fractional minute and hour movement.
pub fn hand_angles(hour: u8, minute: u8, second: u8) -> Option<HandAngles> {
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(HandAngles {
        hour: f32::from(hour % 12) * 30.0 + f32::from(minute) * 0.5 + f32::from(second) / 120.0,
        minute: f32::from(minute) * 6.0 + f32::from(second) * 0.1,
        second: f32::from(second) * 6.0,
    })
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    BootComplete,
    Open(Screen),
    Back,
    Home,
    PreviewFace,
    ToggleClock,
    ToggleQuiet,
    ToggleMotion,
    CycleBrightness,
    SetFace(Face),
    SetPalette(Palette),
    ToggleDate,
    ToggleSeconds,
    ToggleComplications,
    CycleComplication(u8),
    EditSlot(u8),
    SetSlotKind(ComplicationKind),
    CycleSlotAccent,
    ToggleSlotVisible,
    CycleSlotStyle,
    OpenMetric(ComplicationKind),
    CycleGlow,
    RestoreDefaults,
    BeginCustomization,
    ApplyCustomization,
    CancelCustomization,
    ReadingUpdated(ComplicationKind, Reading),
    ConnectionChanged(ConnectionState, bool),
}

#[derive(Debug, Default)]
pub struct State {
    pub screen: Screen,
    pub settings: Settings,
    pub draft: Option<Settings>,
    pub telemetry: Telemetry,
    pub connection: ConnectionState,
    pub assistant_active: bool,
    pub active_slot: u8,
    pub metric: ComplicationKind,
    pub preview_return: Option<Screen>,
}

impl State {
    pub fn editing(&self) -> bool {
        self.draft.is_some()
    }

    /// Replaces the complete telemetry snapshot only when every reading is valid.
    pub fn update_telemetry(&mut self, telemetry: Telemetry) -> bool {
        if !telemetry.is_valid() {
            return false;
        }
        self.telemetry = telemetry;
        true
    }

    /// Returns whether settings should be persisted; drafts remain in memory until applied.
    pub fn dispatch(&mut self, event: Event) -> bool {
        let before = self.settings;
        match event {
            Event::BootComplete if self.screen == Screen::Boot => self.screen = Screen::Home,
            Event::BootComplete => (),
            Event::Open(screen) if screen != Screen::Boot => self.screen = screen,
            Event::Open(_) => (),
            Event::Home => self.screen = Screen::Home,
            Event::PreviewFace => {
                if self.editing() && self.screen != Screen::Home {
                    self.preview_return = Some(self.screen);
                }
                self.screen = Screen::Home;
            }
            Event::Back => {
                self.screen = if self.screen == Screen::Home && self.editing() {
                    self.preview_return.take().unwrap_or(Screen::Home)
                } else {
                    match self.screen {
                        Screen::Boot | Screen::Home | Screen::Launcher => Screen::Home,
                        Screen::DeviceInfo
                        | Screen::FacePicker
                        | Screen::ThemePicker
                        | Screen::Appearance
                        | Screen::Complications => Screen::Settings,
                        Screen::SlotPicker => Screen::SlotAppearance,
                        Screen::SlotAppearance => Screen::Complications,
                        Screen::MetricDetail => {
                            if self.metric.is_health() {
                                Screen::Health
                            } else {
                                Screen::Utilities
                            }
                        }
                        Screen::Utilities => Screen::Launcher,
                        Screen::IconGallery => Screen::DeviceInfo,
                        _ => Screen::Launcher,
                    }
                }
            }
            Event::ToggleClock => self.settings.use_24h = !self.settings.use_24h,
            Event::ToggleQuiet => self.settings.quiet = !self.settings.quiet,
            Event::ToggleMotion => self.settings.reduce_motion = !self.settings.reduce_motion,
            Event::CycleBrightness => {
                self.settings.brightness = match self.settings.brightness {
                    20..=39 => 40,
                    40..=59 => 60,
                    60..=79 => 80,
                    80..=99 => 100,
                    _ => 20,
                }
            }
            Event::SetFace(face) => self.settings.appearance.face = face,
            Event::SetPalette(palette) => self.settings.appearance.palette = palette,
            Event::ToggleDate => {
                self.settings.appearance.show_date = !self.settings.appearance.show_date;
            }
            Event::ToggleSeconds => {
                self.settings.appearance.show_seconds = !self.settings.appearance.show_seconds;
            }
            Event::ToggleComplications => {
                self.settings.appearance.show_complications =
                    !self.settings.appearance.show_complications;
            }
            Event::CycleComplication(slot) => {
                if let Some(slot) = self
                    .settings
                    .appearance
                    .complications
                    .get_mut(usize::from(slot))
                {
                    slot.kind = ComplicationKind::from_index(slot.kind as u8 + 1)
                        .unwrap_or(ComplicationKind::HeartRate);
                }
            }
            Event::EditSlot(slot) if slot < 3 => {
                self.draft.get_or_insert(self.settings);
                self.active_slot = slot;
                self.screen = Screen::SlotAppearance;
                return false;
            }
            Event::EditSlot(_) => (),
            Event::SetSlotKind(kind) => {
                if let Some(slot) = self
                    .settings
                    .appearance
                    .complications
                    .get_mut(usize::from(self.active_slot))
                {
                    slot.kind = kind;
                    self.metric = kind;
                    self.screen = Screen::SlotAppearance;
                }
            }
            Event::CycleSlotAccent => {
                if let Some(slot) = self
                    .settings
                    .appearance
                    .complications
                    .get_mut(usize::from(self.active_slot))
                {
                    slot.accent = match slot.accent {
                        ComplicationAccent::Theme => ComplicationAccent::Coral,
                        ComplicationAccent::Coral => ComplicationAccent::Cyan,
                        ComplicationAccent::Cyan => ComplicationAccent::Mint,
                        ComplicationAccent::Mint => ComplicationAccent::Amber,
                        ComplicationAccent::Amber => ComplicationAccent::Violet,
                        ComplicationAccent::Violet => ComplicationAccent::Theme,
                    };
                }
            }
            Event::ToggleSlotVisible => {
                if let Some(slot) = self
                    .settings
                    .appearance
                    .complications
                    .get_mut(usize::from(self.active_slot))
                {
                    slot.visible = !slot.visible;
                }
            }
            Event::CycleSlotStyle => {
                if let Some(slot) = self
                    .settings
                    .appearance
                    .complications
                    .get_mut(usize::from(self.active_slot))
                {
                    slot.style = match slot.style {
                        ComplicationStyle::IconValue => ComplicationStyle::IconOnly,
                        ComplicationStyle::IconOnly => ComplicationStyle::ValueOnly,
                        ComplicationStyle::ValueOnly => ComplicationStyle::IconValue,
                    };
                }
            }
            Event::OpenMetric(kind) => {
                self.metric = kind;
                self.screen = Screen::MetricDetail;
            }
            Event::CycleGlow => {
                self.settings.appearance.glow = match self.settings.appearance.glow {
                    0..=19 => 20,
                    20..=39 => 40,
                    40..=59 => 60,
                    60..=79 => 80,
                    80..=99 => 100,
                    _ => 0,
                };
            }
            Event::RestoreDefaults => self.settings = Settings::default(),
            Event::BeginCustomization => {
                self.draft.get_or_insert(self.settings);
                return false;
            }
            Event::ApplyCustomization => {
                self.preview_return = None;
                return self
                    .draft
                    .take()
                    .is_some_and(|original| original != self.settings);
            }
            Event::CancelCustomization => {
                self.preview_return = None;
                if let Some(original) = self.draft.take() {
                    self.settings = original;
                }
                return false;
            }
            Event::ReadingUpdated(kind, reading) => {
                if reading.is_valid_for(kind) {
                    self.telemetry.set_reading(kind, reading);
                }
            }
            Event::ConnectionChanged(connection, assistant_active) => {
                self.connection = connection;
                self.assistant_active =
                    connection == ConnectionState::Connected && assistant_active;
            }
        }
        !self.editing() && before != self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_and_late_boot() {
        let mut s = State::default();
        s.dispatch(Event::BootComplete);
        assert_eq!(s.screen, Screen::Home);
        for page in [
            Screen::Assistant,
            Screen::Notifications,
            Screen::QuickSettings,
            Screen::Connection,
            Screen::Settings,
        ] {
            s.dispatch(Event::Open(page));
            s.dispatch(Event::BootComplete);
            assert_eq!(s.screen, page);
            s.dispatch(Event::Back);
            assert_eq!(s.screen, Screen::Launcher);
        }
        s.dispatch(Event::Open(Screen::DeviceInfo));
        s.dispatch(Event::Back);
        assert_eq!(s.screen, Screen::Settings);
        s.dispatch(Event::Home);
        s.dispatch(Event::Back);
        assert_eq!(s.screen, Screen::Home);
        s.dispatch(Event::Open(Screen::Boot));
        assert_eq!(s.screen, Screen::Home);
    }

    #[test]
    fn face_preview_returns_to_editor_only_during_a_draft() {
        let mut state = State::default();
        state.dispatch(Event::BeginCustomization);
        state.dispatch(Event::Open(Screen::SlotAppearance));
        state.dispatch(Event::PreviewFace);
        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.preview_return, Some(Screen::SlotAppearance));
        state.dispatch(Event::Back);
        assert_eq!(state.screen, Screen::SlotAppearance);
        state.dispatch(Event::PreviewFace);
        state.dispatch(Event::CancelCustomization);
        assert_eq!(state.preview_return, None);
        assert_eq!(state.screen, Screen::Home);

        state.dispatch(Event::BeginCustomization);
        state.dispatch(Event::Open(Screen::Appearance));
        state.dispatch(Event::PreviewFace);
        state.dispatch(Event::ApplyCustomization);
        assert_eq!(state.preview_return, None);

        state.dispatch(Event::PreviewFace);
        state.dispatch(Event::Back);
        assert_eq!(state.screen, Screen::Home);
    }
    #[test]
    fn settings_change_and_stay_bounded() {
        let mut s = State::default();
        assert!(!s.dispatch(Event::Home));
        assert!(s.dispatch(Event::ToggleClock));
        assert!(!s.settings.use_24h);
        assert!(s.dispatch(Event::ToggleQuiet));
        assert!(s.settings.quiet);
        assert!(s.dispatch(Event::ToggleMotion));
        assert!(s.settings.reduce_motion);
        for expected in [100, 20, 40, 60, 80] {
            assert!(s.dispatch(Event::CycleBrightness));
            assert_eq!(s.settings.brightness, expected);
            assert!(s.settings.is_valid());
        }
        s.settings.brightness = 0;
        assert!(!s.settings.is_valid());
    }

    #[test]
    fn old_settings_migrate_and_invalid_settings_reject() {
        let settings: Settings = serde_json::from_str(
            r#"{"use_24h":false,"quiet":true,"reduce_motion":true,"brightness":40}"#,
        )
        .unwrap();
        assert_eq!(settings.appearance, Appearance::default());
        assert!(!settings.use_24h);
        assert!(settings.quiet && settings.reduce_motion && settings.is_valid());
        assert_eq!(
            serde_json::from_str::<Settings>(&serde_json::to_string(&settings).unwrap()).unwrap(),
            settings
        );
        assert!(
            !Settings {
                appearance: Appearance {
                    glow: 101,
                    ..Appearance::default()
                },
                ..settings
            }
            .is_valid()
        );
    }

    #[test]
    fn phase_two_settings_migrate_without_losing_preferences() {
        let settings: Settings = serde_json::from_str(r#"{"use_24h":false,"quiet":true,"reduce_motion":true,"brightness":60,"appearance":{"face":"dashboard","palette":"rose","show_date":false,"show_seconds":true,"show_complications":false,"complications":["battery","heart_rate","steps"],"glow":80}}"#).unwrap();
        assert!(!settings.use_24h && settings.quiet && settings.reduce_motion);
        assert_eq!(settings.brightness, 60);
        assert_eq!(settings.appearance.face, Face::Dashboard);
        assert_eq!(settings.appearance.palette, Palette::Rose);
        assert_eq!(
            settings.appearance.complications[0].kind,
            ComplicationKind::Battery
        );
        assert_eq!(
            settings.appearance.complications[0].accent,
            ComplicationAccent::Mint
        );
        assert_eq!(
            settings.appearance.complications[1].accent,
            ComplicationAccent::Coral
        );
    }

    #[test]
    fn draft_apply_cancel_and_restore() {
        let mut state = State::default();
        let original = state.settings;
        assert!(!state.dispatch(Event::BeginCustomization));
        assert!(state.editing());
        for event in [
            Event::SetFace(Face::Classic),
            Event::SetPalette(Palette::Emerald),
            Event::ToggleDate,
            Event::ToggleSeconds,
            Event::ToggleComplications,
            Event::CycleComplication(0),
            Event::CycleGlow,
            Event::ToggleClock,
            Event::ToggleQuiet,
            Event::ToggleMotion,
            Event::CycleBrightness,
        ] {
            assert!(!state.dispatch(event));
        }
        assert_eq!(state.settings.appearance.face, Face::Classic);
        assert_eq!(state.settings.appearance.palette, Palette::Emerald);
        assert_eq!(
            state.settings.appearance.complications[0].kind,
            ComplicationKind::Steps
        );
        assert!(!state.dispatch(Event::BeginCustomization));
        assert_eq!(state.draft, Some(original));
        assert!(!state.dispatch(Event::CancelCustomization));
        assert_eq!(state.settings, original);
        assert!(!state.editing());
        assert!(!state.dispatch(Event::ApplyCustomization));
        state.dispatch(Event::BeginCustomization);
        assert!(!state.dispatch(Event::ApplyCustomization));
        state.dispatch(Event::BeginCustomization);
        state.dispatch(Event::SetFace(Face::Minimal));
        assert!(state.dispatch(Event::ApplyCustomization));
        assert!(!state.editing());
        assert_eq!(state.settings.appearance.face, Face::Minimal);
        let applied = state.settings;
        state.dispatch(Event::BeginCustomization);
        assert!(!state.dispatch(Event::RestoreDefaults));
        assert_eq!(state.settings, Settings::default());
        state.dispatch(Event::CancelCustomization);
        assert_eq!(state.settings, applied);
        assert!(state.dispatch(Event::RestoreDefaults));
        assert_eq!(state.settings, Settings::default());
        assert!(!state.dispatch(Event::CycleComplication(3)));
        for expected in [80, 100, 0, 20, 40, 60] {
            assert!(state.dispatch(Event::CycleGlow));
            assert_eq!(state.settings.appearance.glow, expected);
        }
    }

    #[test]
    fn extended_navigation_keeps_screen_ids() {
        assert_eq!(Screen::Boot as u8, 0);
        assert_eq!(Screen::DeviceInfo as u8, 8);
        assert_eq!(Screen::FacePicker as u8, 9);
        assert_eq!(Screen::Complications as u8, 13);
        assert_eq!(Screen::SlotAppearance as u8, 18);
        let mut state = State::default();
        for page in [
            Screen::FacePicker,
            Screen::ThemePicker,
            Screen::Appearance,
            Screen::Complications,
        ] {
            assert!(!state.dispatch(Event::Open(page)));
            assert!(!state.dispatch(Event::Back));
            assert_eq!(state.screen, Screen::Settings);
        }
        state.dispatch(Event::Open(Screen::Health));
        state.dispatch(Event::Back);
        assert_eq!(state.screen, Screen::Launcher);
    }

    #[test]
    fn telemetry_requires_values_and_valid_ranges() {
        assert!(Telemetry::default().is_valid());
        for status in [DataState::Live, DataState::Stale] {
            assert!(
                !Reading {
                    status,
                    value: None
                }
                .is_valid()
            );
            assert!(
                Reading {
                    status,
                    value: Some(0)
                }
                .is_valid()
            );
        }
        for status in [
            DataState::Unavailable,
            DataState::PermissionRequired,
            DataState::Disconnected,
        ] {
            assert!(
                Reading {
                    status,
                    value: None
                }
                .is_valid()
            );
            assert!(
                !Reading {
                    status,
                    value: Some(1)
                }
                .is_valid()
            );
        }
        let telemetry = Telemetry {
            heart_rate: Reading {
                status: DataState::Live,
                value: Some(72),
            },
            steps: Reading {
                status: DataState::Stale,
                value: Some(0),
            },
            battery: Reading {
                status: DataState::Live,
                value: Some(100),
            },
            ..Telemetry::default()
        };
        let mut state = State::default();
        assert!(state.update_telemetry(telemetry));
        assert_eq!(state.telemetry, telemetry);
        for invalid in [
            Telemetry {
                heart_rate: Reading {
                    value: Some(0),
                    ..telemetry.heart_rate
                },
                ..telemetry
            },
            Telemetry {
                battery: Reading {
                    value: Some(101),
                    ..telemetry.battery
                },
                ..telemetry
            },
            Telemetry {
                steps: Reading {
                    status: DataState::Disconnected,
                    ..telemetry.steps
                },
                ..telemetry
            },
        ] {
            assert!(!invalid.is_valid());
            assert!(!state.update_telemetry(invalid));
            assert_eq!(state.telemetry, telemetry);
        }
        assert_eq!(state.connection, ConnectionState::Unavailable);
        assert!(!state.dispatch(Event::ConnectionChanged(ConnectionState::Connected, true)));
        assert!(state.assistant_active);
        for connection in [
            ConnectionState::Disconnected,
            ConnectionState::Connecting,
            ConnectionState::Unavailable,
        ] {
            assert!(!state.dispatch(Event::ConnectionChanged(connection, true)));
            assert!(!state.assistant_active);
        }
    }

    #[test]
    fn telemetry_event_stays_small_and_ignores_invalid_readings() {
        assert!(core::mem::size_of::<Event>() <= 32);
        let mut state = State::default();
        let valid = Reading {
            status: DataState::Live,
            value: Some(80),
        };
        assert!(!state.dispatch(Event::ReadingUpdated(ComplicationKind::Steps, valid)));
        assert_eq!(state.telemetry.steps, valid);
        let invalid = Reading {
            status: DataState::Live,
            value: Some(0),
        };
        assert!(!state.dispatch(Event::ReadingUpdated(ComplicationKind::HeartRate, invalid)));
        assert_eq!(state.telemetry.heart_rate, Reading::default());
    }

    #[test]
    fn slots_support_all_kinds_and_cancel_apply() {
        let mut state = State::default();
        let original = state.settings;
        assert!(!state.dispatch(Event::EditSlot(1)));
        assert_eq!(state.screen, Screen::SlotAppearance);
        for kind in ComplicationKind::ALL {
            assert!(!state.dispatch(Event::SetSlotKind(kind)));
            assert_eq!(state.settings.appearance.complications[1].kind, kind);
        }
        state.dispatch(Event::CycleSlotAccent);
        state.dispatch(Event::ToggleSlotVisible);
        state.dispatch(Event::CycleSlotStyle);
        assert!(!state.dispatch(Event::CancelCustomization));
        assert_eq!(state.settings, original);
        assert!(!state.editing());
        assert!(!state.dispatch(Event::EditSlot(3)));
        assert_eq!(state.active_slot, 1);
        state.dispatch(Event::EditSlot(2));
        state.dispatch(Event::SetSlotKind(ComplicationKind::Weather));
        assert!(state.dispatch(Event::ApplyCustomization));
        assert_eq!(
            state.settings.appearance.complications[2].kind,
            ComplicationKind::Weather
        );
        for (kind, expected) in [
            (ComplicationKind::HeartRate, Screen::Health),
            (ComplicationKind::Weather, Screen::Utilities),
        ] {
            state.dispatch(Event::OpenMetric(kind));
            state.dispatch(Event::Back);
            assert_eq!(state.screen, expected);
        }
        state.dispatch(Event::Open(Screen::SlotPicker));
        state.dispatch(Event::Back);
        assert_eq!(state.screen, Screen::SlotAppearance);
        state.dispatch(Event::Open(Screen::IconGallery));
        state.dispatch(Event::Back);
        assert_eq!(state.screen, Screen::DeviceInfo);
    }

    #[test]
    fn added_metric_readings_validate_and_do_not_invent_values() {
        let mut telemetry = Telemetry::default();
        assert_eq!(
            telemetry.reading(ComplicationKind::Weather),
            Reading::default()
        );
        telemetry.metrics[3] = Reading {
            status: DataState::Live,
            value: Some(101),
        };
        assert!(!telemetry.is_valid());
        telemetry.metrics[3] = Reading {
            status: DataState::Live,
            value: Some(98),
        };
        assert!(telemetry.is_valid());
        assert_eq!(
            telemetry.reading(ComplicationKind::BloodOxygen).value,
            Some(98)
        );
    }

    #[test]
    fn clock_angles_include_fractional_movement() {
        for hour in [0, 12] {
            assert_eq!(
                hand_angles(hour, 0, 0),
                Some(HandAngles {
                    hour: 0.0,
                    minute: 0.0,
                    second: 0.0
                })
            );
        }
        let angles = hand_angles(23, 59, 59).unwrap();
        assert!((angles.hour - 359.99167).abs() < 0.0001);
        assert!((angles.minute - 359.9).abs() < 0.0001);
        assert_eq!(angles.second, 354.0);
        assert_eq!(hand_angles(24, 0, 0), None);
        assert_eq!(hand_angles(0, 60, 0), None);
        assert_eq!(hand_angles(0, 0, 60), None);
    }
}
