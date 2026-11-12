# Phase 2 targeted UI polish

## Final owner-directed result

The existing Hybrid design and all functional settings remain. Icons were **not replaced**: the approved pulse-heart, existing steps glyph and battery stay. Final owner steering superseded the earlier decorative-wave request: Hybrid now has a black interior, cyan/violet outer ring, no bottom filled waves/inner arcs and no numeral 6. Charlie remains.

## Corrections

- Three equally spaced metric slots: reference-aligned row, common icon centers and numerical baselines. Painted capsules are 44×36 on the 240 design grid, with 44×44 minimum hit regions. Icon-only and value-only styles remain centered.
- 16px SVG icon boxes improve outline visibility without replacing the source artwork. Capsule background contrast increased. Values keep reference scale and weight; no artificial percent-character spacing.
- Text sizing measured against the supplied 1254px reference, normalized to the 950px display span used by the existing case preview. The latest attachment is byte-identical to the stored reference. Rendered digital glyph height is 26px, date about 7px, values about 7–8px and Charlie 8px at 240. These match nearest native pixels; the portable fonts and anti-aliasing are not a claim of pixel-identical photographic lettering.
- Date/numeral placement and Charlie baseline align with the reference proportions. No changes to analog/digital clock layout, time calculations or frontmost hand/hub layering.
- Cyan holds farther across the upper rim before transitioning through blue to violet. Core stroke is sharper, broad halo restrained. Maximum opacity remains clamped.
- Removed Hybrid's bottom waves, inner decorative arcs and 6 marker as requested. Outer-ring animation, real-time hand motion and reduced-motion behavior remain.

## Scope

Changed only shared Slint typography/complication styling, Hybrid decoration/spacing and small vector rim masks. No Rust business logic, protocol/settings schema, icon library, data provider, navigation or platform adapter changed. Shared component consumers were checked through native captures of Minimal, Classic and slot appearance as relevant; unrelated screens retained their prior complete audit. The 114-image matrix was not regenerated.

## Evidence

Before and after images are under `artifacts/ui-phase2-polish/before/` and `after/`. Normal captures show honest unavailable measurements. `design-preview` captures show clearly labeled example values only. Clock poses/time differ because they remain actual Windows time. See the local screenshot index.

## Executed verification

- Cargo build --locked: exit 0; native simulator executable rebuilt.
- Cargo test --locked --workspace: all **34 existing unit tests** retained and passed; 0 failed.
- Cargo fmt --all -- --check: exit 0.
- Cargo clippy --locked --workspace --all-targets -- -D warnings: exit 0.
- Native smoke at 240, 320 and 400: exit 0. Existing checks exercised four faces/five palettes, all metric selections and routes, slot configuration, Apply/Cancel, restore/revert and persisted JSON readback, plus maximum-glow and hands-front rendered-pixel regressions.
- Actual native Windows capture at 240/320/400, normal and marked example-data Hybrid. Shared Minimal/slot preview renders checked. Build/capture errors were not ignored.
- No settings-file or protocol migration was required by this pass. Normal preferences were preserved.

## Remaining limits

No targeted clipping or baseline defect observed in reviewed captures. Hands intentionally pass in front of figures when the actual time requires it, per the earlier explicit layering correction. Exact proprietary font outlines cannot be inferred from a raster reference; matching uses measured native glyph proportions and the existing portable fonts. No physical TFT calibration, real sensor/Android integration or full screen-reader re-audit. Mouse interactions were not re-toured in this style-only pass; actual callback/state/persistence checks were rerun in native windows.

Executable: `D:\watch-research\charlie-watchos\target\debug\watchos-simulator.exe`.
Logs: `polish-build.log`, `polish-tests.log`, `polish-clippy.log`, `polish-embedded.log`.

Final executable SHA-256: `07028E7486DDC7E8D3E1690082CCB1E571EFF43E8864EA31F26C9509FD2BAD66`. Final build/tests/format/Clippy and all three native smoke runs returned 0; Hybrid PNG dimensions verified.

## Final optical-balance cleanup

Following the owner's congestion feedback, the Hybrid digital band moved upward by 3 logical pixels, restoring the reference-like gap above metrics. Complication capsule fill alpha reduced from 0.9 to 0.55, letting icons/values read without three heavy button-like surfaces. Approved icons, text sizes, hand geometry/layering and all settings remain fixed.

Dev-only optimization was enabled for the existing software renderer and its two tiny-skia raster packages. Application debug code and production behavior are unchanged. Native GIF sampling median improved from approximately 346ms in the preceding run to 136ms in this run (same 24-frame diagnostic); this measures capture cadence, not screen FPS or a controlled hardware benchmark. Updated screenshots/GIF remain under the after folder.

Optical-cleanup verification: build, all 34 tests, formatting and Clippy returned 0. Native smoke at 240/320/400 returned 0, including persistence and foreground-hand/max-glow render checks.
