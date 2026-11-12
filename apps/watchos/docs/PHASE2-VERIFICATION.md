# Phase 2 verification — 2026-11-12, Asia/Kolkata

## Result

**Completed for the Windows simulator scope.** The existing five-crate Rust/Slint workspace now includes reference-aligned Hybrid, three other real-time faces, five palettes, 25 official SVG icon categories, three independently configurable complications, activity/utility/detail screens, persistent drafts, motion and complete screen-matrix evidence. This is not real-watch firmware acceptance or Android integration.

## Executed final checks

| Check | Outcome |
|---|---|
| Cargo build --locked, watchos-simulator | Exit 0 |
| Cargo test --locked --workspace | **34 unit tests passed**, 0 failed (core 12, platform 1, protocol 6, simulator 15) |
| Cargo fmt --all -- --check | Exit 0 |
| Cargo clippy --locked --workspace --all-targets -- -D warnings | Exit 0 |
| Cargo check core/protocol/platform --target thumbv7em-none-eabi | Exit 0; no_std portability check only |
| Native --smoke-test at 240/320/400 | All exited 0 |
| Forced maximum-glow + foreground-hand rendered-pixel regression | Passed at 240/320/400 |
| PNG matrix headers | 19 screens × three sizes × full/dial = **114 PNGs**, expected dimensions verified |
| Official icon XML validation | 25/25 SVG files: 24×24 viewBox, 2px rounded strokes; upstream license retained |

Pinned environment: Rust 1.99.1 GNU Windows, Slint/slint-build 1.18.1, local GCC/MinGW. No new global tools or UI framework. New use of already-pinned image 0.25.10 PNG/GIF features supports native diagnostic exports. Font assets retain OFL notices. Icons are pinned Lucide 1.72.0; see ICONS.md for source commit, mapping, checksums and license.

## Real visual audit

GPT-6 Luna subagents reviewed every one of the 19 native screen captures plus all four faces at **each** supported size. Reports: UI-UX-AUDIT.md, AUDIT-320.md, AUDIT-400.md. Before/after evidence is retained under artifacts/ui-phase2/before-audit and the final matrix. These are actual Slint software-renderer snapshots, not mockups.

Demonstrated fixes:

- Replaced unsupported gradient-round-rectangle treatment with solid round surfaces and theme-tinted SVG illumination masks.
- Corrected typography, explicit numeral bounds, fixed digital columns and date alignment; raised analog pivot and balanced complication positions against the reference.
- Fixed hidden list rows leaving empty gaps by using filtered health/utility models.
- Fixed clipped Device Info and metric-note glyphs using native line height and compact explicit blocks.
- Increased icon-sheet diagnostic labels; all 25 categories render consistently.
- Hands and hub paint last, **above text**, per the owner correction. An explicitly marked 05:10 fixture checks a known white-hand pixel over the readout at all three sizes. It is test time, not a live-clock claim.
- Fixed maximum-glow opacity exceeding 1 during animation, which caused an intermittent debug software-renderer overflow. Opacity is clamped; native smoke forces glow=100, phase=90 and brightness=100 at all sizes.
- Kept data models stable across clock-only updates; reduced-motion/off-screen paths stop decorative movement.

The final Luna confirmations report no remaining glyph cutoff in the inspected screens. Small unlabeled tops of following list capsules are deliberate scroll continuation, not clipped labels. Wheel interaction reached lower controls in the native editor. Token contrast calculation returned a minimum **4.74:1** for normal text/selected palette combinations; dim unavailable icons are excluded and retain full-contrast status text.

## Executed native functionality

Observed with actual Windows screenshots, accessibility queries, clicks, wheel and keyboard:

- Clock/date read Windows local time; analog/digital values share one snapshot, with millisecond hand angles and rollover tests.
- Complication tap opens its own metric detail; Weather showed an explicit unavailable state rather than a fabricated reading.
- Metric picker selected Calories and returned to the slot editor with the matching Lucide icon.
- Accent changed Coral → Cyan immediately; visibility changed to Hidden; style changed to Icon only. Full-face preview showed the hidden slot absent while retaining other slots.
- A subsequent native Apply saved the configured Weather/Cyan slot into isolated test JSON. Closing/restarting the process reloaded the same metric/accent and other slot fields into the real editor. Evidence: slot-edit-native.jpg, slot-hidden-native.jpg, slot-persisted-native.jpg and metric-detail-native.jpg.
- Preview → Escape/Back returned to the slot editor. Normal Back/Home behavior remains; Apply/Cancel clear preview return state. Failed saves keep retryable drafts and preserve their return target.
- Native callback smoke additionally exercised all 25 metric selections/routes, all four faces, all five palettes, accent/style/visibility changes, save/readback, Cancel, restore/revert and connection-state bindings.
- Theme selection/restart persistence was also observed in the earlier native pass. Normal local-data settings were preserved; final slot tests used target/phase2-check.

Physical Escape later stopped Computer Use. No further automated UI input was sent after that stop. Final interactive state is handed to the user; no claim is made that automation subsequently returned that window to Home.

## Animation evidence

Hybrid uses real-time smooth hand endpoints, a restrained traveling rim highlight, gentle lower waves and short control feedback. Decorative motion is disabled by reduced motion. Approximately 33ms sampling is used on visible Home; other/reduced states use a one-second clock cadence. Phase includes full local time of day, avoiding a minute-boundary jump. No shader or artificial clock pose runs in normal mode.

The native GIF exporter records 24 real window snapshots and validates different rendered pixels and finite advancing phase. GIF encoding occurs after bounded sampling so quantization cannot stall capture; measured intervals are retained in the manifest. Recording buffers exist only in explicit diagnostic mode. This is animation evidence, not a measured frame-rate, power or embedded-memory benchmark.

## Honest limitations

- Live watch sensors, health, battery, Bluetooth, Android sync, assistant connection, weather, alarm/timer/stopwatch/music and phone-finder providers are not implemented. Unsupported categories show unavailable. Local brightness/quiet/settings controls are labeled local and function in simulator state.
- Optional design-preview examples are visibly marked in the shell and dial and use isolated preferences. No example data is presented as a genuine measurement.
- The supplied photo is used only for simulator casing/strap outside an opaque live display. Clock, text, hands, complications and status are rendered UI; the reference is not watch-face wallpaper. The case is not physical hardware-fit evidence.
- No real-watch write, OTA/DFU/flash, Android/backend/server/Crest modification or speculative JieLi/Realtek driver occurred. JieLi remains the leading Bluetooth vendor from owner-supplied evidence; MCU/application processor/peripherals are unknown.
- No optimized release/package, clean-machine deployment, full screen-reader certification, high-DPI matrix, physical touch/crown calibration, TFT measurement or hardware memory/power profiling. Generic ARM compilation does not identify the watch architecture or link firmware.
- Settings still use the original build checkout; distribution needs a runtime data-path policy. Multiple instances remain last-writer-wins.

## Artifacts and reproducibility

Exact executable: `D:\watch-research\charlie-watchos\target\debug\watchos-simulator.exe`.

README.md documents build/run/capture; artifacts/ui-phase2/README.md indexes the face and screen matrices, native interaction evidence, animation and marked regression fixtures. Local logs: phase2-build.log, phase2-tests.log, phase2-clippy.log, phase2-embedded.log, phase2-gui.log and phase2-gui-error.log. Historical VERIFICATION.md remains Phase 1 evidence only.

Executable SHA-256: `A9EB0ADAC0C83BB3F732157A4DFC9CE243984B895A7A240469648AA9EFD95DBE`.
