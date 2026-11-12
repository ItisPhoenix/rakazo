# Reference face rebuild — 2026-11-13

Scope: hybrid dial composition, shared Lucide icon assets, and an explicitly labeled reference capture. Live clock, telemetry, storage, other faces and navigation are preserved. No bottom waves or 6 numeral.

Coordinates are in the existing 240-unit dial and scale uniformly. Latest isolated reference ring spans about 1120px in a 1254px image. On the 240-unit dial the hand hub is (121,105); hour and minute lengths are 56px and 90px, with reference preview minute angle 103 degrees. Digital type is 37px, numerals 18px, date 10px, values 10px and wordmark 12px. Hybrid digital band y=111, date y=51, capsules y=154, wordmark y=211. Metric centers are 69,121,173 with 44x39px capsules and 13px icons. Hour stroke is 7px; minute stroke 3.6px. Violet tint is restricted to the hybrid outer rim. Lower dial remains black with no waves.

The native software renderer visibly squares rounded gradient rectangles, so solid rounded capsule/hub surfaces are retained. The measured staged comparison uses 22:18, THU 12 NOV and example readings (59 bpm, 3,406 steps, 41% battery). This pose is restricted to `--capture --design-preview`. Normal preview and live runtime still use system time. Design preview labeling remains outside the watch display.

The shared OS `Icon` component now maps kind 0/1/2 to the official Lucide `heart`, `sport-shoe` and `battery` SVGs selected by the owner. These render in the watch face, launchers, pickers and details with the existing color and accessibility behavior. Existing Lucide 1.72.0 assets are retained for the rest of the system. Both asset sources use ISC; attribution and commit pins are recorded in ICONS.md. The supplied photographic artwork does not identify its font or original vector files; portable font rendering and icon paths cannot be certified pixel-identical to that source.

Verification results are appended after execution. Captures under after/ are actual native Slint snapshots, not composited renderings. before/ is the previously delivered face and uses its historical live time, so compare spacing rather than clock pose.

## Final rebuild verification

`cargo build --locked -p watchos-simulator` succeeded after the final UI changes. The actual native Slint simulator captured the design preview at 240, 320 and 400 logical pixels, and live mode at 240. I visually inspected the captures. The design capture now uses the reference's 22:18, THU 12 NOV pose and matching minute hand; this pose exists only under `--design-preview`. The live capture showed the local system time and unavailable dashes, with no sample sensor data. Snapshots are in `artifacts/reference-rebuild/after/`.

No test suite or Clippy run was performed in this targeted visual pass. The shared component bindings changed only their asset source; other navigation paths were not exercised again. The photographic source does not expose font metadata, so the bundled portable Outfit/DM Sans faces are matched by measured size/placement rather than exact proprietary glyph outlines. Physical TFT rendering remains unverified.


The face and system share the exact Lucide `heart`, `sport-shoe` and `battery` icons selected by the owner. Live state and accents use the shared `Icon` component.
