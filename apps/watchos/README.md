# Charlie WatchOS — Phase 2

A Rust + Slint Windows smartwatch simulator. Default is the **Charlie Hybrid** face at **240 × 240 logical pixels**; 320 and 400 remain supported. This is desktop software, not flashable watch firmware.

## Run

```powershell
cd D:\watch-research\charlie-watchos
.\target\debug\watchos-simulator.exe
.\target\debug\watchos-simulator.exe --size 320
.\target\debug\watchos-simulator.exe --size 400 --face classic
```

Exact executable: `D:\watch-research\charlie-watchos\target\debug\watchos-simulator.exe`.

Tap the dial, click Apps or use the simulator crown to open the launcher. Wheel-scroll lists. Back / Escape / Backspace return through the hierarchy (including full-face preview back to its editor); Home / H return to the face; A opens Apps. Tab and Enter/Space operate shared buttons. Normal desktop preview is enlarged 1.5× for readability; native capture stays 1×. The decorative bezel/crown is outside the logical display; it is not a model of confirmed hardware internals.

## Customize

Apps → Settings → Watch faces / Themes / Appearance / Complications. Changes preview immediately. **Preview** shows the full dial. **Apply** persists the draft; **Cancel** restores its original settings. You can visit multiple customization screens in one draft. Restore defaults opens a reviewable draft; Apply or Cancel decides the result. Failed saves retain a retryable draft and visible warning.

Four real-time faces: Hybrid, Minimal Digital, Classic Analog, Information Dashboard. Five palettes: Cyan/Violet, Electric Blue, Emerald/Mint, Sunset Orange, Rose/Purple. Customize date, analog seconds, complications and their three independent metric/accent/visibility/style slots, glow, 12/24 hour clock and reduced motion. Quick Settings retains quiet preference and preview light (window opacity only, not monitor/display hardware brightness).

Settings live in `local-data/settings.json`. Existing Phase 1 clock/quiet/motion/light preferences survive migration; missing appearance fields receive defaults. Invalid files remain untouched until an explicit change is applied. Settings currently use the original build checkout as data root even if the executable is moved; packaging needs a runtime data-path policy before distribution. Multiple instances are last-writer-wins, with process-specific save staging files.

## Honest device state

Normal mode uses real Windows local time and **no invented health/battery readings**. The local icon library covers 25 health/system categories; health and utility dashboards open actual detail states. Brightness/quiet/settings controls work locally; unsupported sensor and utility providers remain unavailable. All three complications are unavailable; tap one for Health & activity. Provider states support live, unavailable, permission required, disconnected and stale readings. Charlie styling uses connection state; no real connection/assistant transport exists, so normal wordmark remains neutral. No BLE, phone pairing, Android sync, network request, OTA or device write is implemented.

Optional design preview:

```powershell
.\target\debug\watchos-simulator.exe --design-preview
```

This explicitly injects **example** heart rate 72, steps 5248 and battery 86%. Window and dial say DESIGN PREVIEW; health status says Example. These are not measurements. Preview mode uses isolated settings under `target/phase2-check`; it never modifies normal preferences or claims a real Charlie connection.

## Build and verify

Pinned Rust 1.99.1 GNU Windows host and Slint 1.18.1. Fresh Windows setup: install [rustup](https://rustup.rs/), then `rustup toolchain install 1.99.1-x86_64-pc-windows-gnu --profile minimal --component rustfmt --component clippy`; install x64 MinGW-w64 with SEH exception support. This checkout uses project-local [WinLibs GCC 16.2.0 / MinGW-w64 UCRT 14.0.0 r3](https://github.com/brechtsanders/winlibs_mingw/releases/tag/16.2.0posix-23.1.3-14.0.0-ucrt-r3). w64devkit alone lacks the required `libgcc_eh`.

```powershell
. .\scripts\env.ps1
cargo build --locked -p watchos-simulator
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p watchos-core -p watchos-protocol -p watchos-platform --target thumbv7em-none-eabi
.\target\debug\watchos-simulator.exe --smoke-test
.\target\debug\watchos-simulator.exe --size 320 --smoke-test
.\target\debug\watchos-simulator.exe --size 400 --smoke-test
```

The local environment script selects tools inside `.tools/`. With a normal rustup installation, use `cargo +1.99.1-x86_64-pc-windows-gnu` if required. Install the generic embedded compilation probe with `rustup target add thumbv7em-none-eabi --toolchain 1.99.1-x86_64-pc-windows-gnu`. **This target is not a WA49V3 architecture claim.** Cargo.lock pins transitive dependencies. Rust core/protocol/platform compile without std; protocol uses alloc. UI and simulator currently use desktop features.

Smoke tests open a native window/event loop and assert callback wiring, four faces, five palettes, settings, draft Apply/Cancel, restore/revert, persistence and connection bindings. They do not prove pointer geometry or appearance. Native screenshots and actual mouse/keyboard checks are recorded separately.

Capture one face (real Windows time, unavailable data):

```powershell
.\target\debug\watchos-simulator.exe --size 240 --face hybrid --capture
```

Capture exits after writing full-client and dial-only PNGs to `artifacts/ui-phase2`. `--face` accepts `hybrid`, `minimal`, `classic`, `dashboard`. Add `--design-preview` only for clearly marked example-data captures. Capture and smoke use isolated settings; normal preferences are preserved. Full screenshots include the shell; dial crops contain only the logical display at the host's DPI scale. Captures are native Slint software-renderer snapshots, not mockups.

## Architecture and design

| Crate | Responsibility |
|---|---|
| watchos-core | no_std events, navigation, settings drafts, telemetry validation, clock angles |
| watchos-protocol | no_std + alloc versioned messages, shared appearance/customization schema |
| watchos-platform | no_std hardware contracts and unavailable provider |
| watchos-ui | theme tokens, shared controls, four functional Slint faces and round-screen flows |
| watchos-simulator | one Windows clock snapshot, file storage, bindings, native loop, capture |

[Complete UI audit](docs/UI-UX-AUDIT.md) · [Icon sources/license](docs/ICONS.md) · [Design system](docs/UI-DESIGN-SYSTEM.md) · [Watch faces/customization](docs/WATCH-FACES.md) · [Protocol](docs/PROTOCOL.md) · [Phase 2 verification](docs/PHASE2-VERIFICATION.md) · [Screenshot index](artifacts/ui-phase2/README.md).

The reference is `references/hybrid-reference.png`. Its casing/strap artwork is used only as simulator chrome outside an opaque live Slint dial; its original clock/data pixels are covered. Static vector masks supply perimeter illumination; actual time, hands, numerals, date, complications, status and controls are rendered components. No whole-face image is used. Software-renderer limitations require explicit sizing and small SVG alpha masks rather than gradient rounded rectangles. No real-time shaders. Millisecond-based hands and a restrained traveling outer-rim highlight update while Home is visible; reduced-motion/off-screen paths return to a one-second clock cadence.

## Licensing and hardware boundaries

Project source is GPL-3.0-only ([LICENSE](LICENSE)). Slint offers GPL-3.0-only, Royalty-free Desktop/Mobile/Web 2.0 and commercial options. The royalty-free desktop license excludes embedded systems; a distributed watch port must satisfy GPL obligations or use an appropriate commercial license. Review [Slint terms](https://slint.dev/pricing) before distribution. Device Info includes AboutSlint. Bundled Outfit and DM Sans fonts retain their SIL Open Font License notices in `crates/watchos-ui/ui/fonts/`.

Owner supplied published WA49V3 model specifications make 240×240 TFT the Phase 2 design target. These are not controller, MCU or board measurements. Owner-supplied manufacturer ID `0x05D6`, BLE subversion `0x0772`, Classic subversion `0x22BB` make JieLi the leading Bluetooth vendor; application processor, architecture and peripherals remain unknown. [Future JieLi SDK port](docs/HARDWARE-PORTING.md) is separate. No speculative vendor driver, Android/server/Crest modification or firmware-writing path exists.

`VERIFICATION.md` is the historical Phase 1 report; current acceptance is in `docs/PHASE2-VERIFICATION.md`. Release packaging, clean-machine distribution and physical-watch acceptance are outside this result.

## Activity complications

Settings → Complications → Slot 1/2/3. Choose among 25 categories, cycle six accents, hide/show independently, or choose Icon + value / Icon only / Value only. Preview updates immediately; Apply persists, Cancel restores. Each visible slot opens its own detail screen. Existing legacy three-string slot arrays migrate without dropping previous preferences. Protocol v3 carries the same slot configuration; older v1/v2 messages remain accepted where documented. No Android sync is claimed.

Icons are bundled official Lucide 1.72.0 SVGs, pinned to upstream commit with the upstream license retained. No icon fonts, emojis or runtime downloads.

Animation evidence: `watchos-simulator.exe --capture-motion` writes a 24-frame native GIF and timing/phase/pixel manifest to `artifacts/ui-phase2`. Capture buffers are bounded and exist only in diagnostic capture mode. `--screen settings --capture` captures a named screen for the audit matrix (19 screens × three sizes). Normal operation has no recording buffers.

Latest targeted polish: [corrections and verification](docs/PHASE2-FINAL-POLISH.md) · [before/after screenshots](artifacts/ui-phase2-polish/README.md). Hybrid keeps the approved icon shapes, uses reference-sized text and has a clean black interior without the 6 or bottom blobs.
