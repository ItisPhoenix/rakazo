#![no_std]
extern crate alloc;
use alloc::string::String;
use serde::{Deserialize, Deserializer, Serialize};

pub const VERSION: u16 = 3;
pub const MAX_FRAME_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Face {
    #[default]
    Hybrid = 0,
    Minimal = 1,
    Classic = 2,
    Dashboard = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Palette {
    #[default]
    CyanViolet = 0,
    ElectricBlue = 1,
    Emerald = 2,
    Sunset = 3,
    Rose = 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ComplicationKind {
    #[default]
    HeartRate = 0,
    Steps = 1,
    Battery = 2,
    Calories = 3,
    Distance = 4,
    Sleep = 5,
    BloodOxygen = 6,
    Workout = 7,
    Running = 8,
    Walking = 9,
    Cycling = 10,
    ActivityGoals = 11,
    ExerciseDuration = 12,
    Bluetooth = 13,
    Notifications = 14,
    Weather = 15,
    Alarm = 16,
    Timer = 17,
    Stopwatch = 18,
    Music = 19,
    Settings = 20,
    PhoneFinder = 21,
    Brightness = 22,
    DoNotDisturb = 23,
    Charlie = 24,
}

impl ComplicationKind {
    pub const ALL: [Self; 25] = [
        Self::HeartRate,
        Self::Steps,
        Self::Battery,
        Self::Calories,
        Self::Distance,
        Self::Sleep,
        Self::BloodOxygen,
        Self::Workout,
        Self::Running,
        Self::Walking,
        Self::Cycling,
        Self::ActivityGoals,
        Self::ExerciseDuration,
        Self::Bluetooth,
        Self::Notifications,
        Self::Weather,
        Self::Alarm,
        Self::Timer,
        Self::Stopwatch,
        Self::Music,
        Self::Settings,
        Self::PhoneFinder,
        Self::Brightness,
        Self::DoNotDisturb,
        Self::Charlie,
    ];

    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(usize::from(index)).copied()
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::HeartRate => "Heart Rate",
            Self::Steps => "Steps",
            Self::Battery => "Battery",
            Self::Calories => "Calories",
            Self::Distance => "Distance",
            Self::Sleep => "Sleep",
            Self::BloodOxygen => "Blood Oxygen",
            Self::Workout => "Workout",
            Self::Running => "Running",
            Self::Walking => "Walking",
            Self::Cycling => "Cycling",
            Self::ActivityGoals => "Activity Goals",
            Self::ExerciseDuration => "Exercise Duration",
            Self::Bluetooth => "Bluetooth",
            Self::Notifications => "Notifications",
            Self::Weather => "Weather",
            Self::Alarm => "Alarm",
            Self::Timer => "Timer",
            Self::Stopwatch => "Stopwatch",
            Self::Music => "Music",
            Self::Settings => "Settings",
            Self::PhoneFinder => "Phone Finder",
            Self::Brightness => "Brightness",
            Self::DoNotDisturb => "Do Not Disturb",
            Self::Charlie => "Charlie",
        }
    }

    pub const fn is_health(self) -> bool {
        matches!(
            self,
            Self::HeartRate
                | Self::Steps
                | Self::Calories
                | Self::Distance
                | Self::Sleep
                | Self::BloodOxygen
                | Self::Workout
                | Self::Running
                | Self::Walking
                | Self::Cycling
                | Self::ActivityGoals
                | Self::ExerciseDuration
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ComplicationAccent {
    #[default]
    Theme = 0,
    Coral = 1,
    Cyan = 2,
    Mint = 3,
    Amber = 4,
    Violet = 5,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ComplicationStyle {
    #[default]
    IconValue = 0,
    IconOnly = 1,
    ValueOnly = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplicationSlot {
    pub kind: ComplicationKind,
    pub accent: ComplicationAccent,
    pub visible: bool,
    pub style: ComplicationStyle,
}

impl ComplicationSlot {
    pub const fn new(kind: ComplicationKind, accent: ComplicationAccent) -> Self {
        Self {
            kind,
            accent,
            visible: true,
            style: ComplicationStyle::IconValue,
        }
    }
}

impl Default for ComplicationSlot {
    fn default() -> Self {
        Self::new(ComplicationKind::HeartRate, ComplicationAccent::Coral)
    }
}

impl<'de> Deserialize<'de> for ComplicationSlot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum SlotInput {
            Object(SlotFields),
            Legacy(ComplicationKind),
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct SlotFields {
            kind: ComplicationKind,
            accent: ComplicationAccent,
            visible: bool,
            style: ComplicationStyle,
        }
        Ok(match SlotInput::deserialize(deserializer)? {
            SlotInput::Object(slot) => Self {
                kind: slot.kind,
                accent: slot.accent,
                visible: slot.visible,
                style: slot.style,
            },
            SlotInput::Legacy(kind) => Self::new(
                kind,
                match kind {
                    ComplicationKind::HeartRate => ComplicationAccent::Coral,
                    ComplicationKind::Steps => ComplicationAccent::Cyan,
                    ComplicationKind::Battery => ComplicationAccent::Mint,
                    _ => ComplicationAccent::Theme,
                },
            ),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub face: Face,
    pub palette: Palette,
    pub show_date: bool,
    pub show_seconds: bool,
    pub show_complications: bool,
    pub complications: [ComplicationSlot; 3],
    pub glow: u8,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            face: Face::Hybrid,
            palette: Palette::CyanViolet,
            show_date: true,
            show_seconds: false,
            show_complications: true,
            complications: [
                ComplicationSlot::new(ComplicationKind::HeartRate, ComplicationAccent::Coral),
                ComplicationSlot::new(ComplicationKind::Steps, ComplicationAccent::Cyan),
                ComplicationSlot::new(ComplicationKind::Battery, ComplicationAccent::Mint),
            ],
            glow: 60,
        }
    }
}

impl Appearance {
    pub fn is_valid(&self) -> bool {
        self.glow <= 100
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Customization {
    pub appearance: Appearance,
    pub use_24h: bool,
    pub reduce_motion: bool,
}

impl Default for Customization {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            use_24h: true,
            reduce_motion: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub version: u16,
    pub id: u64,
    pub message: Message,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Message {
    Connection(ConnectionState),
    Notification(Notification),
    Assistant(AssistantMessage),
    Action(UserAction),
    Customization(Customization),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Unavailable,
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub id: u64,
    pub app: String,
    pub title: String,
    pub body: String,
    pub received_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantMessage {
    pub request_id: u64,
    pub status: AssistantStatus,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantStatus {
    Unavailable,
    Request,
    Thinking,
    Response,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum UserAction {
    Home,
    Back,
    DismissNotification(u64),
    AskAssistant(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError {
    UnsupportedVersion,
    TextTooLong,
    EmptyRequest,
    InvalidAppearance,
}

impl Envelope {
    pub fn new(id: u64, message: Message) -> Self {
        Self {
            version: VERSION,
            id,
            message,
        }
    }
    /// Validate after bounded decoding, before updating application state.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !(1..=VERSION).contains(&self.version)
            || (self.version == 1 && matches!(&self.message, Message::Customization(_)))
        {
            return Err(ValidationError::UnsupportedVersion);
        }
        let valid = match &self.message {
            Message::Customization(customization) => {
                if !customization.appearance.is_valid() {
                    return Err(ValidationError::InvalidAppearance);
                }
                true
            }
            Message::Notification(n) => {
                n.app.len() <= 64 && n.title.len() <= 128 && n.body.len() <= 1024
            }
            Message::Assistant(a) => a.text.len() <= 2048,
            Message::Action(UserAction::AskAssistant(text)) => {
                if text.trim().is_empty() {
                    return Err(ValidationError::EmptyRequest);
                }
                text.len() <= 1024
            }
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ValidationError::TextTooLong)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages_roundtrip() {
        let messages = [
            Message::Connection(ConnectionState::Unavailable),
            Message::Notification(Notification {
                id: 2,
                app: "Charlie".into(),
                title: "Hello".into(),
                body: "Real data only".into(),
                received_unix_ms: 42,
            }),
            Message::Assistant(AssistantMessage {
                request_id: 3,
                status: AssistantStatus::Response,
                text: "Hello".into(),
            }),
            Message::Action(UserAction::Home),
            Message::Action(UserAction::Back),
            Message::Action(UserAction::DismissNotification(2)),
            Message::Action(UserAction::AskAssistant("Hello".into())),
            Message::Customization(Customization {
                appearance: Appearance {
                    face: Face::Classic,
                    palette: Palette::Sunset,
                    show_date: false,
                    show_seconds: true,
                    show_complications: false,
                    complications: [
                        ComplicationSlot::new(ComplicationKind::Battery, ComplicationAccent::Mint),
                        ComplicationSlot::new(
                            ComplicationKind::HeartRate,
                            ComplicationAccent::Coral,
                        ),
                        ComplicationSlot::new(ComplicationKind::Steps, ComplicationAccent::Cyan),
                    ],
                    glow: 100,
                },
                use_24h: false,
                reduce_motion: true,
            }),
        ];
        for message in messages {
            let e = Envelope::new(1, message);
            assert_eq!(e.validate(), Ok(()));
            let json = serde_json::to_string(&e).unwrap();
            assert_eq!(serde_json::from_str::<Envelope>(&json).unwrap(), e);
        }
    }
    #[test]
    fn rejects_version_oversize_and_empty_requests() {
        let mut e = Envelope::new(1, Message::Connection(ConnectionState::Disconnected));
        e.version = 99;
        assert_eq!(e.validate(), Err(ValidationError::UnsupportedVersion));
        e.version = VERSION;
        e.message = Message::Action(UserAction::AskAssistant(" ".into()));
        assert_eq!(e.validate(), Err(ValidationError::EmptyRequest));
        e.message = Message::Action(UserAction::AskAssistant("x".repeat(1025)));
        assert_eq!(e.validate(), Err(ValidationError::TextTooLong));
        assert!(
            serde_json::from_str::<Envelope>(
                r#"{"version":1,"id":0,"message":{"type":"unknown"}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn versions_and_appearance_validation() {
        let mut e = Envelope::new(4, Message::Connection(ConnectionState::Connected));
        e.version = 1;
        assert_eq!(e.validate(), Ok(()));
        e.message = Message::Customization(Customization::default());
        assert_eq!(e.validate(), Err(ValidationError::UnsupportedVersion));
        e.version = VERSION;
        assert_eq!(e.validate(), Ok(()));
        e.message = Message::Customization(Customization {
            appearance: Appearance {
                glow: 101,
                ..Appearance::default()
            },
            ..Customization::default()
        });
        assert_eq!(e.validate(), Err(ValidationError::InvalidAppearance));
        assert_eq!(
            serde_json::from_str::<Appearance>("{}").unwrap(),
            Appearance::default()
        );
        assert!(serde_json::from_str::<Appearance>(r#"{"glow":60,"extra":1}"#).is_err());
        assert_eq!(
            serde_json::from_str::<Customization>("{}").unwrap(),
            Customization::default()
        );
        assert!(serde_json::from_str::<Customization>(r#"{"use_24h":true,"extra":1}"#).is_err());
    }

    #[test]
    fn old_slot_arrays_migrate_and_new_slot_fields_roundtrip() {
        let old: Appearance = serde_json::from_str(
            r#"{"face":"classic","glow":40,"complications":["heart_rate","steps","battery"]}"#,
        )
        .unwrap();
        assert_eq!(old.face, Face::Classic);
        assert_eq!(old.glow, 40);
        assert_eq!(old.complications, Appearance::default().complications);
        let mut changed = Appearance::default();
        changed.complications[0].style = ComplicationStyle::ValueOnly;
        changed.complications[0].accent = ComplicationAccent::Violet;
        changed.complications[0].visible = false;
        assert_eq!(
            serde_json::from_str::<Appearance>(&serde_json::to_string(&changed).unwrap()).unwrap(),
            changed
        );
    }

    #[test]
    fn all_metric_indices_are_stable() {
        for (index, kind) in ComplicationKind::ALL.iter().enumerate() {
            assert_eq!(*kind as usize, index);
            assert_eq!(ComplicationKind::from_index(index as u8), Some(*kind));
            assert!(!kind.label().is_empty());
        }
        assert_eq!(ComplicationKind::from_index(25), None);
        assert!(ComplicationKind::HeartRate.is_health());
        assert!(!ComplicationKind::Bluetooth.is_health());
    }

    #[test]
    fn version_two_accepts_legacy_customization_and_new_output_is_v3() {
        let json = r#"{"version":2,"id":4,"message":{"type":"customization","data":{"appearance":{"complications":["heart_rate","steps","battery"]},"use_24h":false,"reduce_motion":true}}}"#;
        let envelope: Envelope = serde_json::from_str(json).unwrap();
        assert_eq!(envelope.validate(), Ok(()));
        let emitted = Envelope::new(5, envelope.message);
        assert_eq!(emitted.version, 3);
        assert!(
            serde_json::to_string(&emitted)
                .unwrap()
                .contains("\"kind\":\"heart_rate\"")
        );
    }
}
