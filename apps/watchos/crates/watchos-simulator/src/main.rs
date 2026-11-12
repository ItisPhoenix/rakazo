mod capture;
mod desktop;
// The development link transport. The simulator only uses it when
// --dev-link-port is given, so a normal run never opens a socket. Some
// primitives stay unused until the client side arrives.
#[allow(dead_code)]
mod devlink;
mod presentation;
use desktop::{FileStorage, PreviewDisplay};
use presentation::sync;
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};
use watchos_core::{DataState, Event, Reading, Screen, Settings, State, Telemetry};
use watchos_platform::{
    Connectivity, DataProvider, Display, DisplayConfig, Storage, UnavailableHardware,
};
use watchos_protocol::{ComplicationKind, Face, Palette};
use watchos_ui::WatchWindow;

#[derive(Debug)]
struct Options {
    size: u16,
    smoke: bool,
    capture: bool,
    face: Option<Face>,
    preview: bool,
    motion: bool,
    screen: Option<Screen>,
    dev_link_port: Option<u16>,
    settings_file: Option<PathBuf>,
}
fn options(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut result = Options {
        size: 240,
        smoke: false,
        capture: false,
        face: None,
        preview: false,
        motion: false,
        screen: None,
        dev_link_port: None,
        settings_file: None,
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--size" => {
                result.size = args
                    .next()
                    .ok_or("--size needs pixels")?
                    .parse()
                    .map_err(|_| "--size must be integer pixels")?
            }
            "--smoke-test" => result.smoke = true,
            "--capture" => result.capture = true,
            "--capture-motion" => result.motion = true,
            "--settings-file" => {
                result.settings_file = Some(PathBuf::from(
                    args.next().ok_or("--settings-file needs a path")?,
                ))
            }
            "--dev-link-port" => {
                let port = args
                    .next()
                    .ok_or("--dev-link-port needs a port number")?
                    .parse::<u16>()
                    .map_err(|_| "--dev-link-port must be a port number between 1 and 65535")?;
                if port == 0 {
                    return Err("--dev-link-port must be between 1 and 65535".into());
                }
                result.dev_link_port = Some(port);
            }
            "--screen" => {
                result.screen = Some(
                    screen_from_name(&args.next().ok_or("--screen needs a screen name")?)
                        .ok_or("Unknown screen name")?,
                )
            }
            "--design-preview" => result.preview = true,
            "--face" => {
                result.face = Some(match args.next().as_deref() {
                    Some("hybrid") => Face::Hybrid,
                    Some("minimal") => Face::Minimal,
                    Some("classic") => Face::Classic,
                    Some("dashboard") => Face::Dashboard,
                    _ => return Err("--face needs hybrid|minimal|classic|dashboard".into()),
                })
            }
            _ => {
                return Err(format!(
                    "Unknown argument: {arg}. Usage: watchos-simulator [--size 240..800] [--face hybrid|minimal|classic|dashboard] [--capture | --smoke-test] [--design-preview] [--settings-file PATH] [--dev-link-port PORT]"
                ));
            }
        }
    }
    if !(DisplayConfig {
        width: result.size,
        height: result.size,
    })
    .is_valid()
    {
        return Err("--size must be 240..800".into());
    }
    if u8::from(result.smoke) + u8::from(result.capture) + u8::from(result.motion) > 1 {
        return Err("Choose --capture, --capture-motion or --smoke-test".into());
    }
    if result.dev_link_port.is_some() && (result.capture || result.motion || result.preview) {
        return Err("--dev-link-port cannot be combined with --capture, --capture-motion or --design-preview".into());
    }
    Ok(result)
}

fn screen_from_name(name: &str) -> Option<Screen> {
    Some(match name {
        "boot" => Screen::Boot,
        "home" => Screen::Home,
        "apps" => Screen::Launcher,
        "assistant" => Screen::Assistant,
        "notifications" => Screen::Notifications,
        "quick-settings" => Screen::QuickSettings,
        "connection" => Screen::Connection,
        "settings" => Screen::Settings,
        "device-info" => Screen::DeviceInfo,
        "faces" => Screen::FacePicker,
        "themes" => Screen::ThemePicker,
        "appearance" => Screen::Appearance,
        "health" => Screen::Health,
        "complications" => Screen::Complications,
        "slot-picker" => Screen::SlotPicker,
        "metric-detail" => Screen::MetricDetail,
        "utilities" => Screen::Utilities,
        "icons" => Screen::IconGallery,
        "slot-appearance" => Screen::SlotAppearance,
        _ => return None,
    })
}

fn load_settings(storage: &dyn Storage) -> (Settings, String) {
    let mut bytes = [0; 2048];
    match storage.load(&mut bytes) {
        Ok(0) => (Settings::default(), String::new()),
        Ok(n) => match serde_json::from_slice::<Settings>(&bytes[..n]) {
            Ok(s) if s.is_valid() => (s, String::new()),
            _ => (Settings::default(), "Settings unreadable. Defaults active; original file preserved until you save a change.".into()),
        },
        Err(_) => (Settings::default(), "Settings storage unavailable. Defaults active; changes may be session-only.".into()),
    }
}

fn event(action: i32) -> Option<Event> {
    Some(match action {
        1 => Event::Home,
        30 => Event::PreviewFace,
        2 => Event::Open(Screen::Launcher),
        3 => Event::Open(Screen::Assistant),
        4 => Event::Open(Screen::Notifications),
        5 => Event::Open(Screen::QuickSettings),
        6 => Event::Open(Screen::Connection),
        7 => Event::Open(Screen::Settings),
        8 => Event::Open(Screen::DeviceInfo),
        9 => Event::Back,
        10 => Event::ToggleClock,
        11 => Event::ToggleQuiet,
        12 => Event::ToggleMotion,
        13 => Event::CycleBrightness,
        14 => Event::ToggleDate,
        15 => Event::ToggleSeconds,
        16 => Event::ToggleComplications,
        17 => Event::CycleGlow,
        18 => Event::Open(Screen::Health),
        40..=42 => Event::EditSlot((action - 40) as u8),
        43 => Event::CycleSlotAccent,
        44 => Event::ToggleSlotVisible,
        45 => Event::CycleSlotStyle,
        46 => Event::Open(Screen::SlotPicker),
        19 => Event::Open(Screen::Utilities),
        300..=324 => Event::SetSlotKind(ComplicationKind::from_index((action - 300) as u8)?),
        400..=424 => Event::OpenMetric(ComplicationKind::from_index((action - 400) as u8)?),
        100 => Event::SetFace(Face::Hybrid),
        101 => Event::SetFace(Face::Minimal),
        102 => Event::SetFace(Face::Classic),
        103 => Event::SetFace(Face::Dashboard),
        200 => Event::SetPalette(Palette::CyanViolet),
        201 => Event::SetPalette(Palette::ElectricBlue),
        202 => Event::SetPalette(Palette::Emerald),
        203 => Event::SetPalette(Palette::Sunset),
        204 => Event::SetPalette(Palette::Rose),
        _ => return None,
    })
}

fn dispatch_action(state: &mut State, action: i32) -> bool {
    match action {
        20..=23 => {
            state.dispatch(Event::BeginCustomization);
            state.dispatch(Event::Open(match action {
                20 => Screen::FacePicker,
                21 => Screen::ThemePicker,
                22 => Screen::Appearance,
                _ => Screen::Complications,
            }));
            false
        }
        24 => {
            state.dispatch(Event::BeginCustomization);
            state.dispatch(Event::RestoreDefaults);
            state.dispatch(Event::Home);
            false
        }
        31 => {
            let save = state.dispatch(Event::ApplyCustomization);
            state.dispatch(Event::Home);
            save
        }
        32 => {
            state.dispatch(Event::CancelCustomization);
            state.dispatch(Event::Home);
            false
        }
        _ => event(action).is_some_and(|event| state.dispatch(event)),
    }
}

fn action_and_save(state: &mut State, action: i32, storage: &dyn Storage) -> Option<bool> {
    let previous = state.settings;
    let original = state.draft;
    let preview_return = state.preview_return;
    if !dispatch_action(state, action) {
        return None;
    }
    let saved = serde_json::to_vec(&state.settings)
        .ok()
        .is_some_and(|bytes| storage.save(&bytes).is_ok());
    if !saved {
        state.draft = Some(original.unwrap_or(previous));
        state.preview_return = preview_return;
    }
    Some(saved)
}

fn start_clock(weak: slint::Weak<WatchWindow>, state: Rc<RefCell<State>>) {
    let Some(ui) = weak.upgrade() else {
        return;
    };
    let animated = {
        let state = state.borrow();
        state.screen == Screen::Home && !state.settings.reduce_motion && ui.window().is_visible()
    };
    slint::Timer::single_shot(
        Duration::from_millis(if animated { 33 } else { 1000 }),
        move || {
            if let Some(ui) = weak.upgrade() {
                presentation::tick(&ui, &state.borrow());
                start_clock(weak, state);
            }
        },
    );
}

fn start_link_poll(
    weak: slint::Weak<WatchWindow>,
    state: Rc<RefCell<State>>,
    link: Option<Rc<devlink::server::DevLink>>,
    storage: Rc<FileStorage>,
) {
    slint::Timer::single_shot(Duration::from_millis(50), move || {
        if let Some(link) = &link {
            // Requests arrive on a socket thread and are applied here, on the
            // thread that owns the watch state and the settings file.
            let answered = {
                let mut state = state.borrow_mut();
                link.pump(&mut state, storage.as_ref())
            };
            if answered > 0
                && let Some(ui) = weak.upgrade()
            {
                sync(&ui, &state.borrow());
            }
        }
        start_link_poll(weak, state, link, storage);
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = options(std::env::args().skip(1)).map_err(std::io::Error::other)?;
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let isolated = options.smoke
        || options.capture
        || options.motion
        || options.preview
        || options.screen.is_some();
    let storage = Rc::new(FileStorage(match options.settings_file.clone() {
        Some(path) => path,
        None => root.join(if isolated {
            "target/phase2-check/settings.json"
        } else {
            "local-data/settings.json"
        }),
    }));
    let (settings, warning) = if options.capture || options.motion {
        (Settings::default(), String::new())
    } else {
        load_settings(storage.as_ref())
    };
    let mut initial = State {
        settings,
        ..State::default()
    };
    initial.update_telemetry(UnavailableHardware.telemetry());
    initial.dispatch(Event::ConnectionChanged(UnavailableHardware.state(), false));
    if let Some(face) = options.face {
        initial.settings.appearance.face = face;
    }
    if options.preview {
        let reading = |value| Reading {
            status: DataState::Live,
            value: Some(value),
        };
        initial.update_telemetry(Telemetry {
            heart_rate: reading(59),
            steps: reading(3406),
            battery: reading(41),
            ..Telemetry::default()
        });
    }
    if let Some(screen) = options.screen {
        if matches!(
            screen,
            Screen::FacePicker
                | Screen::ThemePicker
                | Screen::Appearance
                | Screen::Complications
                | Screen::SlotPicker
                | Screen::SlotAppearance
        ) {
            initial.dispatch(Event::BeginCustomization);
        }
        initial.screen = screen;
    }
    let state = Rc::new(RefCell::new(initial));
    let ui = WatchWindow::new()?;
    ui.set_logical_size(options.size as i32);
    ui.set_preview_mode(options.preview);
    let preview_scale = if options.capture || options.motion || options.smoke {
        1.0
    } else {
        1.5
    };
    ui.window().set_size(slint::LogicalSize::new(
        f32::from(options.size) * preview_scale + 112.,
        f32::from(options.size) * preview_scale + 196.,
    ));
    ui.set_storage_status(warning.into());
    sync(&ui, &state.borrow());
    PreviewDisplay(DisplayConfig {
        width: options.size,
        height: options.size,
    })
    .set_brightness(settings.brightness)
    .map_err(|e| format!("Display error: {e:?}"))?;
    // The development link is opt in. Without --dev-link-port the simulator
    // never opens a socket and stays exactly as it was.
    let link = match options.dev_link_port {
        Some(port) => {
            let token =
                devlink::server::session_token_from_environment().map_err(std::io::Error::other)?;
            let link = Rc::new(
                devlink::server::DevLink::start(port, token).map_err(std::io::Error::other)?,
            );
            link.seed(&devlink::server::customization_of(&state.borrow().settings));
            eprintln!(
                "dev link: listening on 127.0.0.1:{} for a local development client",
                link.port()
            );
            Some(link)
        }
        None => None,
    };
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let storage = storage.clone();
        let link = link.clone();
        ui.on_action(move |action| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            if let Some(saved) = action_and_save(&mut state, action, storage.as_ref()) {
                ui.set_storage_status(if saved {
                    "".into()
                } else {
                    "Last save failed. Check local-data permissions before applying changes.".into()
                });
                // A committed change is published to the development link. A
                // cancelled draft changes nothing, so the revision stays.
                if saved && let Some(link) = &link {
                    link.publish(&devlink::server::customization_of(&state.settings));
                }
            }
            sync(&ui, &state);
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_key_action(move |key| {
            if let Some(ui) = weak.upgrade() {
                match key.as_str() {
                    "h" | "H" => ui.invoke_action(1),
                    "\u{1b}" | "\u{8}" => ui.invoke_action(9),
                    "a" | "A" => ui.invoke_action(2),
                    _ => (),
                }
            }
        });
    }
    start_clock(ui.as_weak(), state.clone());
    if link.is_some() {
        start_link_poll(ui.as_weak(), state.clone(), link.clone(), storage.clone());
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        slint::Timer::single_shot(Duration::from_millis(900), move || {
            if let Some(ui) = weak.upgrade() {
                if options.screen != Some(Screen::Boot) {
                    state.borrow_mut().dispatch(Event::BootComplete);
                }
                sync(&ui, &state.borrow());
            }
        });
    }
    if options.smoke {
        let fixture_directory = root.join("artifacts/ui-phase2");
        let weak = ui.as_weak();
        let state = state.clone();
        let storage = storage.clone();
        slint::Timer::single_shot(Duration::from_millis(1500), move || {
            let ui = weak.upgrade().expect("window exists");
            assert_eq!(ui.get_page(), 1);
            assert!(ui.get_clock_valid());
            assert_ne!(ui.get_clock_text(), "--:--");
            for action in 2..=8 {
                ui.invoke_action(action);
                assert_eq!(ui.get_page(), action);
            }
            ui.invoke_action(18);
            assert_eq!(ui.get_page(), 12);
            for action in 10..=17 {
                ui.invoke_action(action);
            }
            let original = state.borrow().settings;
            ui.invoke_action(20);
            for face in 100..=103 {
                ui.invoke_action(face);
                assert_eq!(ui.get_selected_face(), face - 100);
            }
            for palette in 200..=204 {
                ui.invoke_action(palette);
                assert_eq!(ui.get_palette(), palette - 200);
            }
            for slot in 40..=42 {
                ui.invoke_action(slot);
            }
            for kind in ComplicationKind::ALL {
                ui.invoke_action(300 + kind as i32);
                assert_eq!(
                    state.borrow().settings.appearance.complications[2].kind,
                    kind
                );
                ui.invoke_action(400 + kind as i32);
                assert_eq!(ui.get_detail_kind(), kind as i32);
            }
            ui.invoke_action(43);
            ui.invoke_action(44);
            ui.invoke_action(45);
            assert!(ui.get_editing());
            ui.invoke_action(32);
            assert_eq!(state.borrow().settings, original);
            ui.invoke_action(21);
            ui.invoke_action(202);
            ui.invoke_action(101);
            ui.invoke_action(31);
            ui.invoke_action(40);
            ui.invoke_action(315);
            ui.invoke_action(43);
            ui.invoke_action(44);
            ui.invoke_action(45);
            ui.invoke_action(31);
            let (saved, warning) = load_settings(storage.as_ref());
            assert!(warning.is_empty());
            assert_eq!(saved, state.borrow().settings);
            assert_eq!(saved.appearance.palette, Palette::Emerald);
            assert_eq!(saved.appearance.face, Face::Minimal);
            ui.invoke_action(24);
            assert!(ui.get_editing());
            ui.invoke_action(32);
            assert_eq!(state.borrow().settings, saved);
            for connection in [
                watchos_protocol::ConnectionState::Unavailable,
                watchos_protocol::ConnectionState::Disconnected,
                watchos_protocol::ConnectionState::Connecting,
                watchos_protocol::ConnectionState::Connected,
            ] {
                state
                    .borrow_mut()
                    .dispatch(Event::ConnectionChanged(connection, false));
                sync(&ui, &state.borrow());
                assert_eq!(ui.get_connection(), connection as i32);
            }
            ui.invoke_action(1);
            assert_eq!(ui.get_page(), 1);
            ui.set_preview_mode(true);
            ui.set_brightness(100);
            ui.set_glow(100);
            ui.set_motion_phase(90.0);
            ui.set_selected_face(0);
            ui.set_hours_text("05".into());
            ui.set_minutes_text("10".into());
            let angles = watchos_core::hand_angles(5, 10, 0).unwrap();
            ui.set_hour_angle(angles.hour);
            ui.set_minute_angle(angles.minute);
            let snapshot = ui.window().take_snapshot().expect("native layer fixture");
            let scale = f32::from(options.size) / 240.0 * ui.window().scale_factor();
            let x = (56.0 * ui.window().scale_factor() + 140.0 * scale).round() as usize;
            let y = (56.0 * ui.window().scale_factor() + 150.0 * scale).round() as usize;
            assert!(
                snapshot.as_slice()[y * snapshot.width() as usize + x].r > 180,
                "Hour hand must paint above the cyan digital text/backplate"
            );
            capture::save(
                &ui,
                &fixture_directory,
                &format!("hands-front-fixture-{}", options.size),
                options.size,
            )
            .expect("save layer evidence");
            ui.set_preview_mode(options.preview);
            sync(&ui, &state.borrow());
            println!(
                "PHASE2 SMOKE PASS: native clock, navigation, four faces, five palettes, settings, draft Cancel/Apply, reset/revert, persistence, connection bindings"
            );
            slint::quit_event_loop().expect("quit event loop");
        });
    }
    let capture_failed = Rc::new(Cell::new(false));
    if options.capture {
        let directory = root.join("artifacts/ui-phase2");
        let weak = ui.as_weak();
        let state = state.clone();
        let failed = capture_failed.clone();
        slint::Timer::single_shot(Duration::from_millis(1700), move || {
            let ui = weak.upgrade().expect("window exists");
            let name = options
                .screen
                .map(|screen| format!("screen-{}", screen as u8))
                .unwrap_or_else(|| {
                    ["hybrid", "minimal", "classic", "dashboard"]
                        [state.borrow().settings.appearance.face as usize]
                        .to_string()
                });
            let name = format!(
                "{name}-{}{}",
                options.size,
                if options.preview {
                    "-design-preview"
                } else {
                    ""
                }
            );
            // Reference pose is confined to explicitly labeled design captures.
            if options.preview && options.screen.is_none() {
                ui.set_hours_text("22".into());
                ui.set_minutes_text("18".into());
                ui.set_period_text("".into());
                ui.set_weekday_text("THU".into());
                ui.set_day_text("12".into());
                ui.set_month_text("NOV".into());
                let angles = watchos_core::hand_angles(22, 18, 0).unwrap();
                ui.set_hour_angle(angles.hour);
                // The reference stages 22:18 with its minute hand just past 3.
                ui.set_minute_angle(103.0);
            }
            if let Err(error) = capture::save(&ui, &directory, &name, options.size) {
                eprintln!("Capture failed: {error}");
                failed.set(true);
            } else {
                println!("CAPTURE PASS: {name}");
            }
            slint::quit_event_loop().expect("quit capture loop");
        });
    }
    if options.motion {
        capture::start_motion(
            &ui,
            root.join("artifacts/ui-phase2"),
            capture_failed.clone(),
        );
    }
    ui.run()?;
    if let Some(link) = &link {
        link.shutdown();
    }
    if capture_failed.get() {
        return Err("Native capture failed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_and_callbacks() {
        assert_eq!(options(std::iter::empty()).unwrap().size, 240);
        assert_eq!(
            options(["--size".into(), "400".into()].into_iter())
                .unwrap()
                .size,
            400
        );
        for args in [
            vec!["--size", "0"],
            vec!["--size"],
            vec!["--face", "bad"],
            vec!["--capture", "--smoke-test"],
            vec!["--other"],
        ] {
            assert!(options(args.into_iter().map(String::from)).is_err());
        }
        for action in 1..=18 {
            assert!(event(action).is_some());
        }
        assert!(event(99).is_none());
    }
    #[test]
    fn customization_is_reviewable() {
        let mut s = State::default();
        let original = s.settings;
        assert!(!dispatch_action(&mut s, 21));
        assert!(s.editing());
        assert_eq!(s.screen, Screen::ThemePicker);
        assert!(!dispatch_action(&mut s, 204));
        assert_ne!(s.settings, original);
        assert!(!dispatch_action(&mut s, 32));
        assert_eq!(s.settings, original);
        dispatch_action(&mut s, 20);
        dispatch_action(&mut s, 103);
        assert!(dispatch_action(&mut s, 31));
        assert_eq!(s.settings.appearance.face, Face::Dashboard);
        assert!(!s.editing());
    }
    #[test]
    fn failed_save_keeps_retryable_draft() {
        struct Flaky {
            fail: Cell<bool>,
            bytes: RefCell<Vec<u8>>,
        }
        impl Storage for Flaky {
            fn load(&self, _: &mut [u8]) -> Result<usize, watchos_platform::PlatformError> {
                Ok(0)
            }
            fn save(&self, bytes: &[u8]) -> Result<(), watchos_platform::PlatformError> {
                if self.fail.get() {
                    return Err(watchos_platform::PlatformError::Io);
                }
                self.bytes.replace(bytes.to_vec());
                Ok(())
            }
        }
        let storage = Flaky {
            fail: Cell::new(true),
            bytes: RefCell::new(Vec::new()),
        };
        let mut state = State::default();
        dispatch_action(&mut state, 21);
        dispatch_action(&mut state, 204);
        assert_eq!(action_and_save(&mut state, 31, &storage), Some(false));
        assert!(state.editing());
        assert_eq!(state.settings.appearance.palette, Palette::Rose);
        storage.fail.set(false);
        assert_eq!(action_and_save(&mut state, 31, &storage), Some(true));
        assert!(!state.editing());
        assert_eq!(
            serde_json::from_slice::<Settings>(&storage.bytes.borrow()).unwrap(),
            state.settings
        );
    }
    #[test]
    fn malformed_settings_use_defaults() {
        struct Bad;
        impl Storage for Bad {
            fn load(&self, b: &mut [u8]) -> Result<usize, watchos_platform::PlatformError> {
                b[..2].copy_from_slice(b"{}");
                Ok(2)
            }
            fn save(&self, _: &[u8]) -> Result<(), watchos_platform::PlatformError> {
                panic!("must not overwrite corrupt data on load")
            }
        }
        let (settings, warning) = load_settings(&Bad);
        assert_eq!(settings, Settings::default());
        assert!(!warning.is_empty());
    }
}
