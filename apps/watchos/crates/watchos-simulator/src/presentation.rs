use crate::desktop::SystemClock;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use watchos_core::{DataState, HandAngles, Reading, Screen, State, hand_angles};
use watchos_platform::{Clock, LocalTime};
use watchos_protocol::{ComplicationKind, ConnectionState};
use watchos_ui::{Theme, WatchWindow};

pub fn sync(ui: &WatchWindow, state: &State) {
    let settings = state.settings;
    let appearance = settings.appearance;
    ui.set_page(state.screen as i32);
    ui.set_clock_24(settings.use_24h);
    ui.set_quiet(settings.quiet);
    ui.set_reduced_motion(settings.reduce_motion);
    ui.set_brightness(settings.brightness.into());
    ui.set_selected_face(appearance.face as i32);
    ui.set_palette(appearance.palette as i32);
    ui.global::<Theme>().set_palette(appearance.palette as i32);
    ui.global::<Theme>()
        .set_reduced_motion(settings.reduce_motion);
    ui.set_show_date(appearance.show_date);
    ui.set_show_seconds(appearance.show_seconds);
    ui.set_show_complications(appearance.show_complications);
    ui.set_glow(appearance.glow.into());
    ui.set_editing(state.editing());
    ui.set_connection(match state.connection {
        ConnectionState::Unavailable => 0,
        ConnectionState::Disconnected => 1,
        ConnectionState::Connecting => 2,
        ConnectionState::Connected => 3,
    });
    ui.set_assistant_active(state.assistant_active);
    tick(ui, state);

    let readings =
        ComplicationKind::ALL.map(|kind| metric_text(state, kind, ui.get_preview_mode()));
    ui.set_metric_kinds(stable_model(
        ui.get_metric_kinds(),
        ComplicationKind::ALL.map(|kind| kind as i32).to_vec(),
    ));
    ui.set_health_kinds(stable_model(
        ui.get_health_kinds(),
        ComplicationKind::ALL
            .into_iter()
            .filter(|kind| kind.is_health())
            .map(|kind| kind as i32)
            .collect(),
    ));
    ui.set_utility_kinds(stable_model(
        ui.get_utility_kinds(),
        ComplicationKind::ALL
            .into_iter()
            .filter(|kind| !kind.is_health())
            .map(|kind| kind as i32)
            .collect(),
    ));
    ui.set_metric_labels(stable_model(
        ui.get_metric_labels(),
        ComplicationKind::ALL
            .map(|kind| SharedString::from(kind.label()))
            .to_vec(),
    ));
    ui.set_metric_values(stable_model(
        ui.get_metric_values(),
        readings
            .iter()
            .map(|reading| SharedString::from(reading.0.as_str()))
            .collect(),
    ));
    ui.set_metric_statuses(stable_model(
        ui.get_metric_statuses(),
        readings
            .iter()
            .map(|reading| SharedString::from(reading.1.as_str()))
            .collect(),
    ));
    let slots = appearance.complications;
    ui.set_complication_kinds(stable_model(
        ui.get_complication_kinds(),
        slots.map(|slot| slot.kind as i32).to_vec(),
    ));
    ui.set_complication_accents(stable_model(
        ui.get_complication_accents(),
        slots.map(|slot| slot.accent as i32).to_vec(),
    ));
    ui.set_complication_styles(stable_model(
        ui.get_complication_styles(),
        slots.map(|slot| slot.style as i32).to_vec(),
    ));
    ui.set_complication_visibility(stable_model(
        ui.get_complication_visibility(),
        slots.map(|slot| slot.visible).to_vec(),
    ));
    ui.set_complication_labels(stable_model(
        ui.get_complication_labels(),
        slots
            .map(|slot| SharedString::from(slot.kind.label()))
            .to_vec(),
    ));
    ui.set_complication_values(stable_model(
        ui.get_complication_values(),
        slots
            .map(|slot| SharedString::from(readings[slot.kind as usize].0.as_str()))
            .to_vec(),
    ));
    ui.set_complication_statuses(stable_model(
        ui.get_complication_statuses(),
        slots
            .map(|slot| SharedString::from(readings[slot.kind as usize].1.as_str()))
            .to_vec(),
    ));
    ui.set_active_slot(i32::from(state.active_slot.min(2)));
    ui.set_detail_kind(state.metric as i32);
    ui.set_detail_title(state.metric.label().into());
    ui.set_detail_value(readings[state.metric as usize].0.clone().into());
    ui.set_detail_status(readings[state.metric as usize].1.clone().into());
    ui.set_detail_note(
        match state.metric {
            ComplicationKind::Brightness => "Adjusts simulator preview light only.",
            ComplicationKind::DoNotDisturb => {
                "Local quiet preference. Haptic hardware is unavailable."
            }
            ComplicationKind::Settings => "Open watch settings to customize your experience.",
            ComplicationKind::Charlie => "No assistant transport is configured.",
            _ if readings[state.metric as usize].1 == "Example" => {
                "Design preview only. This is not a measurement."
            }
            _ if readings[state.metric as usize].1 == "Unavailable" => {
                "No supported provider connected. No readings generated."
            }
            _ => "Provider state. Stale readings are marked with an asterisk.",
        }
        .into(),
    );
}

fn metric_text(state: &State, kind: ComplicationKind, preview: bool) -> (String, String) {
    match kind {
        ComplicationKind::Brightness => (
            format!("{}%", state.settings.brightness),
            "Local preference".into(),
        ),
        ComplicationKind::DoNotDisturb => (
            if state.settings.quiet { "On" } else { "Off" }.into(),
            "Local preference".into(),
        ),
        ComplicationKind::Settings => ("Open".into(), "Local utility".into()),
        ComplicationKind::Charlie => (
            match state.connection {
                ConnectionState::Connected => "Ready",
                ConnectionState::Connecting => "…",
                ConnectionState::Disconnected => "Off",
                ConnectionState::Unavailable => "—",
            }
            .into(),
            match state.connection {
                ConnectionState::Connected => "Connected",
                ConnectionState::Connecting => "Connecting",
                ConnectionState::Disconnected => "Disconnected",
                ConnectionState::Unavailable => "Unavailable",
            }
            .into(),
        ),
        _ => {
            let reading = state.telemetry.reading(kind);
            let (value, mut status) = reading_text(reading, kind);
            if preview && reading.value.is_some() {
                status = "Example".into();
            }
            (value, status)
        }
    }
}

pub fn tick(ui: &WatchWindow, state: &State) {
    let time = SystemClock.local_time().ok();
    let clock = clock_text(time, state.settings.use_24h);
    let mut date = clock.date.split_whitespace();
    let (weekday, day, month) = if clock.valid {
        (
            date.next().unwrap_or(""),
            date.next().unwrap_or(""),
            date.next().unwrap_or(""),
        )
    } else {
        ("", "", "")
    };
    ui.set_weekday_text(weekday.into());
    ui.set_day_text(day.into());
    ui.set_month_text(month.into());
    ui.set_motion_phase(motion_phase(time, state));
    ui.set_connection_pulse(
        if clock.valid
            && state.connection == ConnectionState::Connecting
            && !state.settings.reduce_motion
            && time.is_some_and(|t| t.second % 2 == 0)
        {
            0.65
        } else {
            1.0
        },
    );
    ui.set_clock_valid(clock.valid);
    ui.set_clock_text(clock.clock.into());
    ui.set_hours_text(clock.hours.into());
    ui.set_minutes_text(clock.minutes.into());
    ui.set_seconds_text(clock.seconds.into());
    ui.set_date_text(clock.date.into());
    ui.set_period_text(clock.period.into());
    ui.set_hour_angle(clock.angles.hour);
    ui.set_minute_angle(clock.angles.minute);
    ui.set_second_angle(clock.angles.second);
}

fn motion_phase(time: Option<LocalTime>, state: &State) -> f32 {
    if state.screen != Screen::Home || state.settings.reduce_motion {
        return 0.0;
    }
    time.filter(|t| t.millisecond < 1000 && hand_angles(t.hour, t.minute, t.second).is_some())
        .map_or(0.0, |t| {
            (f32::from(
                ((u32::from(t.hour) * 3600 + u32::from(t.minute) * 60 + u32::from(t.second)) % 16)
                    as u8,
            ) + f32::from(t.millisecond) / 1000.0)
                * 22.5
        })
}

// Keep repeaters and keyboard focus intact across state updates.
fn stable_model<T: Clone + PartialEq + 'static>(current: ModelRc<T>, values: Vec<T>) -> ModelRc<T> {
    if current.iter().eq(values.iter().cloned()) {
        current
    } else {
        Rc::new(VecModel::from(values)).into()
    }
}

pub fn reading_text(reading: Reading, kind: ComplicationKind) -> (String, String) {
    let value = || match reading.value {
        Some(value) if kind == ComplicationKind::Battery => format!("{value}%"),
        Some(value) if kind == ComplicationKind::Steps => {
            let digits = value.to_string();
            let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
            for (index, digit) in digits.chars().enumerate() {
                if index > 0 && (digits.len() - index) % 3 == 0 {
                    grouped.push(',');
                }
                grouped.push(digit);
            }
            grouped
        }
        Some(value) => value.to_string(),
        None => "—".into(),
    };
    match reading.status {
        DataState::Live => (value(), "Live".into()),
        DataState::Unavailable => ("—".into(), "Unavailable".into()),
        DataState::PermissionRequired => ("Allow".into(), "Permission required".into()),
        DataState::Disconnected => ("Off".into(), "Disconnected".into()),
        DataState::Stale => (
            if reading.value.is_some() {
                format!("{}*", value())
            } else {
                value()
            },
            "Stale reading".into(),
        ),
    }
}

struct ClockText {
    valid: bool,
    clock: String,
    hours: String,
    minutes: String,
    seconds: String,
    date: String,
    period: String,
    angles: HandAngles,
}

fn clock_text(time: Option<LocalTime>, use_24h: bool) -> ClockText {
    if let Some((time, angles)) = time.filter(|t| t.millisecond < 1000).and_then(|t| {
        hand_angles(t.hour, t.minute, t.second).map(|mut angles| {
            let milliseconds = f32::from(t.millisecond);
            angles.hour += milliseconds / 120_000.0;
            angles.minute += milliseconds / 10_000.0;
            angles.second += milliseconds * 0.006;
            (t, angles)
        })
    }) {
        let hour = if use_24h {
            time.hour
        } else {
            (time.hour + 11) % 12 + 1
        };
        let weekdays = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
        let months = [
            "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
        ];
        let hours = format!("{hour:02}");
        let minutes = format!("{:02}", time.minute);
        return ClockText {
            valid: true,
            clock: format!("{hours}:{minutes}"),
            hours,
            minutes,
            seconds: format!("{:02}", time.second),
            date: format!(
                "{} {:02} {}",
                weekdays.get(time.weekday as usize).unwrap_or(&"???"),
                time.day,
                months
                    .get(time.month.wrapping_sub(1) as usize)
                    .unwrap_or(&"???")
            ),
            period: if use_24h {
                ""
            } else if time.hour < 12 {
                "AM"
            } else {
                "PM"
            }
            .into(),
            angles,
        };
    }
    ClockText {
        valid: false,
        clock: "--:--".into(),
        hours: "--".into(),
        minutes: "--".into(),
        seconds: "--".into(),
        date: "Time unavailable".into(),
        period: String::new(),
        angles: HandAngles {
            hour: 0.0,
            minute: 0.0,
            second: 0.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(hour: u8, minute: u8, second: u8) -> LocalTime {
        LocalTime {
            hour,
            minute,
            second,
            millisecond: 0,
            year: 2026,
            month: 11,
            day: 12,
            weekday: 3,
        }
    }

    #[test]
    fn clock_only_ticks_preserve_models() {
        let original: ModelRc<i32> = Rc::new(VecModel::from(vec![1, 2])).into();
        assert!(original == stable_model(original.clone(), vec![1, 2]));
        assert!(original != stable_model(original.clone(), vec![2, 1]));
    }
    #[test]
    fn midnight_noon_and_clock_modes() {
        for (hour, digital, period) in
            [(0, "12:05", "AM"), (12, "12:05", "PM"), (23, "11:05", "PM")]
        {
            let clock = clock_text(Some(time(hour, 5, 9)), false);
            assert!(clock.valid);
            assert_eq!(clock.clock, digital);
            assert_eq!(clock.period, period);
            assert_eq!(clock.seconds, "09");
            assert_eq!(clock.date, "THU 12 NOV");
        }
        let clock = clock_text(Some(time(0, 5, 9)), true);
        assert_eq!(clock.clock, "00:05");
        assert_eq!(clock.period, "");
        assert_eq!(clock_text(Some(time(23, 5, 9)), true).clock, "23:05");
    }

    #[test]
    fn single_snapshot_keeps_digital_analog_and_date_consistent() {
        let before = LocalTime {
            year: 2026,
            month: 12,
            day: 31,
            ..time(23, 59, 59)
        };
        let after = LocalTime {
            year: 2027,
            month: 1,
            day: 1,
            weekday: 4,
            ..time(0, 0, 0)
        };
        let clock = clock_text(Some(before), true);
        assert_eq!(clock.clock, format!("{}:{}", clock.hours, clock.minutes));
        assert_eq!(clock.seconds, "59");
        assert_eq!(clock.date, "THU 31 DEC");
        assert_eq!(clock.angles, hand_angles(23, 59, 59).unwrap());
        let clock = clock_text(Some(after), false);
        assert_eq!(clock.clock, "12:00");
        assert_eq!(clock.date, "FRI 01 JAN");
        assert_eq!(clock.period, "AM");
        assert_eq!(clock.angles, hand_angles(0, 0, 0).unwrap());
    }

    #[test]
    fn motion_is_continuous_across_minute_boundaries() {
        let state = State {
            screen: Screen::Home,
            ..State::default()
        };
        let before = LocalTime {
            millisecond: 999,
            ..time(0, 0, 59)
        };
        let after = time(0, 1, 0);
        assert!(
            (motion_phase(Some(after), &state) - motion_phase(Some(before), &state) - 0.0225).abs()
                < 0.001
        );
    }
    #[test]
    fn milliseconds_move_hands_without_advancing_digital_text() {
        let snapshot = LocalTime {
            millisecond: 500,
            ..time(13, 5, 9)
        };
        let clock = clock_text(Some(snapshot), false);
        let base = hand_angles(snapshot.hour, snapshot.minute, snapshot.second).unwrap();
        assert!(clock.valid);
        assert_eq!(clock.clock, "01:05");
        assert_eq!(clock.seconds, "09");
        assert_eq!(clock.period, "PM");
        assert_eq!(clock.date, "THU 12 NOV");
        assert!((clock.angles.hour - base.hour - 500.0 / 120_000.0).abs() < 0.0001);
        assert!((clock.angles.minute - base.minute - 0.05).abs() < 0.0001);
        assert!((clock.angles.second - base.second - 3.0).abs() < 0.0001);
    }

    #[test]
    fn millisecond_bounds_and_midnight_rollover() {
        let before = clock_text(
            Some(LocalTime {
                millisecond: 999,
                ..time(23, 59, 59)
            }),
            true,
        );
        let after = clock_text(Some(time(0, 0, 0)), true);
        assert!(before.valid);
        assert_eq!(before.clock, "23:59");
        assert_eq!(after.clock, "00:00");
        for (old, new, expected_step) in [
            (before.angles.hour, after.angles.hour, 1.0 / 120_000.0),
            (before.angles.minute, after.angles.minute, 0.0001),
            (before.angles.second, after.angles.second, 0.006),
        ] {
            assert!(((new - old).rem_euclid(360.0) - expected_step).abs() < 0.0001);
        }
    }

    #[test]
    fn motion_phase_obeys_screen_reduced_motion_and_snapshot() {
        let snapshot = Some(LocalTime {
            millisecond: 500,
            ..time(10, 20, 17)
        });
        let mut state = State::default();
        assert_eq!(motion_phase(snapshot, &state), 0.0);
        state.screen = Screen::Home;
        assert_eq!(motion_phase(snapshot, &state), 33.75);
        state.settings.reduce_motion = true;
        assert_eq!(motion_phase(snapshot, &state), 0.0);
        state.settings.reduce_motion = false;
        assert_eq!(motion_phase(None, &state), 0.0);
        assert_eq!(motion_phase(Some(time(24, 0, 0)), &state), 0.0);
        assert_eq!(
            motion_phase(
                Some(LocalTime {
                    millisecond: 1000,
                    ..time(10, 20, 17)
                }),
                &state
            ),
            0.0
        );
        assert_eq!(motion_phase(Some(time(10, 20, 16)), &state), 0.0);
    }

    #[test]
    fn missing_or_invalid_clock_clears_every_display_field() {
        for time in [
            None,
            Some(time(24, 0, 0)),
            Some(time(0, 60, 0)),
            Some(time(0, 0, 60)),
            Some(LocalTime {
                millisecond: 1000,
                ..time(0, 0, 0)
            }),
            Some(LocalTime {
                millisecond: u16::MAX,
                ..time(0, 0, 0)
            }),
        ] {
            let clock = clock_text(time, false);
            assert!(!clock.valid);
            assert_eq!(clock.clock, "--:--");
            assert_eq!(clock.hours, "--");
            assert_eq!(clock.minutes, "--");
            assert_eq!(clock.seconds, "--");
            assert_eq!(clock.date, "Time unavailable");
            assert_eq!(clock.period, "");
            assert_eq!(
                clock.angles,
                HandAngles {
                    hour: 0.0,
                    minute: 0.0,
                    second: 0.0
                }
            );
        }
    }

    #[test]
    fn readings_preserve_real_values_units_and_all_statuses() {
        for (kind, value, expected) in [
            (ComplicationKind::HeartRate, 72, "72"),
            (ComplicationKind::Steps, 0, "0"),
            (ComplicationKind::Steps, 999, "999"),
            (ComplicationKind::Steps, 1000, "1,000"),
            (ComplicationKind::Steps, 5248, "5,248"),
            (ComplicationKind::Steps, u32::MAX, "4,294,967,295"),
            (ComplicationKind::Battery, 86, "86%"),
        ] {
            assert_eq!(
                reading_text(
                    Reading {
                        status: DataState::Live,
                        value: Some(value)
                    },
                    kind
                ),
                (expected.into(), "Live".into())
            );
            assert_eq!(
                reading_text(
                    Reading {
                        status: DataState::Stale,
                        value: Some(value)
                    },
                    kind
                ),
                (format!("{expected}*"), "Stale reading".into())
            );
            for (status, text, label) in [
                (DataState::Unavailable, "—", "Unavailable"),
                (
                    DataState::PermissionRequired,
                    "Allow",
                    "Permission required",
                ),
                (DataState::Disconnected, "Off", "Disconnected"),
            ] {
                assert_eq!(
                    reading_text(
                        Reading {
                            status,
                            value: None
                        },
                        kind
                    ),
                    (text.into(), label.into())
                );
                assert_eq!(
                    reading_text(
                        Reading {
                            status,
                            value: Some(value)
                        },
                        kind
                    ),
                    (text.into(), label.into())
                );
            }
            assert_eq!(
                reading_text(
                    Reading {
                        status: DataState::Stale,
                        value: None
                    },
                    kind
                ),
                ("—".into(), "Stale reading".into())
            );
            assert_eq!(
                reading_text(Reading::default(), kind),
                ("—".into(), "Unavailable".into())
            );
        }
    }
}
