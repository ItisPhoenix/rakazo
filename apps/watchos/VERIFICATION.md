# Phase 1 verification — 2026-11-12

## Outcome

**PASS for Windows desktop Phase 1 completion criteria:** Rust/Slint application compiles, launches a real native window, reads Windows local date/time and provides navigable Charlie screens. **Not physical-watch acceptance or flashable firmware.**

## Environment and dependency installation

- Initial `rustc --version`, `cargo --version`, `rustup show`: rustup present; no installed/default toolchain. Initial commands failed honestly.
- Installed Rust 1.99.1 (58cf8f9c3, 2026-10-13) inside project `.tools/rustup`; Cargo cache inside `.tools/cargo`. MSVC and GNU host toolchains downloaded while resolving prerequisites; final build uses `x86_64-pc-windows-gnu`.
- Windows SDK 10.0.26100.0 present; Visual Studio Community lacked C++ build tools. Quiet component installation rejected with installer exit 5007 (not elevated). No successful machine-wide dependency installation.
- w64devkit 2.11.0 was insufficient: linker lacked `libgcc_eh`. Replaced for build with project-local WinLibs GCC 16.2.0 POSIX/SEH / MinGW-w64 UCRT 14.0.0 r3. Archive SHA-256 matched publisher `.sha256`: `3C69305081E4A647144339A936244FCF7D3D9E558319E1AD7BE2CD2886D4B6A1`.
- Slint and slint-build exactly 1.18.1; software renderer and winit backend; accessibility enabled. Serde 1.0.228, serde_json 1.0.145, chrono 0.4.42; all transitive packages pinned by Cargo.lock.
- Installed rustfmt, Clippy and `thumbv7em-none-eabi` Rust std target package locally. This target is a **generic compilation probe**, not a claim about the watch MCU.

## Executed checks

Commands run from project with `scripts/env.ps1` loaded when Cargo needed local tools.

| Check | Exact result |
|---|---|
| `cargo build --locked -p watchos-simulator` | Exit 0; final source compiled into native Windows GNU executable |
| `cargo test --locked --workspace` | Exit 0; 9 unit tests passed, 0 failed; doc-tests contained 0 tests |
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Exit 0; no warnings |
| `cargo check --locked -p watchos-core -p watchos-protocol -p watchos-platform --target thumbv7em-none-eabi` | Exit 0; all three compile without std |
| `target/debug/watchos-simulator.exe --smoke-test` | Exit 0; native event-loop smoke PASS at 320 |
| same with `--size 240 --smoke-test` | Exit 0; native event-loop smoke PASS |
| same with `--size 400 --smoke-test` | Exit 0; native event-loop smoke PASS |
| Normal executable launch without compiler PATH setup | Native window opened on this Windows host |
| Runtime stderr capture | Empty after inspected runs |
| PE import inspection with MinGW objdump | Windows system/UCRT DLL imports; no separate GCC runtime DLL import observed |

Tests cover core navigation, late boot timer protection, settings bounds/cycling; protocol message serde roundtrips, unknown type/version and invalid request rejection; unsupported hardware errors; midnight/noon/date formatting; file replace/read bounds; malformed settings fallback; CLI validation and callback mapping.

Native smoke opens a real window, checks boot completion, a non-placeholder clock, launcher plus six detail screens, Back/Home, four settings and persisted settings readback. Uses `target/smoke-data` separately from normal preferences. It invokes callbacks; **does not prove pointer geometry or visual rendering**.

## Executed native GUI inspection

Used Windows Computer Use screenshots, real clicks, wheel scrolling, accessibility queries and keyboard input:

- Home shows Windows local date `THU 12.11.2026` and changing clock seconds. 24/12 hour display checked; 12-hour PM setting persisted across process restart. Restored 24-hour preference afterward.
- Home → launcher, Charlie, Inbox, Controls; launcher scrolling reaches Settings/Device Info. Back button and H keyboard Home work. Escape after screen transition now works.
- Assistant says Unavailable; connection details say no transport, Bluetooth unavailable, phone disconnected. No invented assistant answer.
- Notification center says All quiet / no notifications received / no phone transport configured.
- Quick Settings and Settings render shared real state; clicking clock format updates it and saves JSON. Smoke covers all four setting callbacks; brightness/motion visual timing was not quantitatively measured.
- Device Info displays logical screen size, protocol v1, Windows simulator, unavailable battery/sensors/health, unspecified physical hardware, firmware disclaimer and Slint attribution.
- Corrected square background leakage into round preview; default face diameter now 320 logical pixels.
- Corrected focus loss when navigation destroyed the focused page button; Escape regression checked with real input.
- Corrected shared scroll position leaking from scrolled launcher into Settings; new screen now starts with Quiet mode visible. Regression verified at 400 pixels.
- Software renderer ignored transform scaling. Replaced it with explicit proportional layout/font dimensions; 400-pixel Home/launcher/Settings now visibly scale correctly. Final 240 native smoke passed; final 240 text legibility was not manually inspected.
- Accessibility exposes labeled buttons. Keyboard focus outlines present. Full screen-reader audit and exhaustive Tab traversal were not performed.

## Review and boundaries

Independent read-only implementation review found no blocking core/protocol/HAL/callback defect. Its temporary-file collision finding was fixed with process-specific staging names. Settings remain last-writer-wins across processes; there is no settings synchronization service.

Settings root intentionally embeds this build checkout (`CARGO_MANIFEST_DIR/../..`). This executable is delivered for this project directory. Moving the executable does not move its settings root; clean-machine packaging needs a runtime data-path policy before distribution. Save errors are visible and session state remains usable.

No release build, clean-machine install, optimized memory/frame-time measurement, hardware calibration, physical touch input, live Bluetooth, live Android protocol interoperability, assistant connection, sensor/battery read, OTA or flashing was performed. The no_std check does not link firmware or prove board support. Protocol uses alloc and needs an allocator on a future embedded target; ingress transport must impose the documented frame cap before parsing.

The provided reverse-engineering report and Charlie Android source were read-only references. No Charlie Android, Ubuntu server, Crest artifact, physical watch or bootloader modification occurred. All implementation, toolchain downloads, build outputs, local preferences and reports are under this project.

## Evidence files

`build.log`, `tests.log`, `clippy.log`, `embedded-check.log`, `runtime.log`, `runtime-error.log` are local execution evidence (ignored by Git). Source and Cargo.lock are deliverables. No screenshot artifact was saved; GUI observations above were inspected live.

Final debug executable: 258578235 bytes; SHA-256: `6DF161A33DB3014379C31AC4E3B79E338396666DF5F10DF81E00832343CFF33E`.

## Hardware evidence update

Owner supplied manufacturer ID `0x05D6` (Zhuhai JieLi Technology), BLE subversion `0x0772`, Classic subversion `0x22BB`. Recorded without independently re-inspecting the recordings. JieLi is the leading Bluetooth vendor; exact MCU/application processor/architecture/peripherals remain unknown. `docs/HARDWARE-PORTING.md` separates future JieLi SDK integration from simulator work. No code/dependency change or hardware action followed this update; executable hash and executed checks above remain applicable.
