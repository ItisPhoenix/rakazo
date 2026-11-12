# Watch faces and customization

## Face renderer

`watchos-ui/ui/faces.slint` is a reusable WatchFace component. It consumes state-backed face index, clock text/angles, date, selected complication kinds/values/statuses, connection, assistant activity and appearance preferences. It performs no I/O and owns no real device claims. Theme definitions and shared components are separate files.

| Face | Composition | Complication support |
|---|---|---|
| Hybrid (default) | Four cardinal numerals/ticks, cyan/violet perimeter, synchronized analog/digital time, date, lower arcs and Charlie wordmark | Three selectable slots |
| Minimal Digital | Large split-tone time, concise date, quiet perimeter arcs | Three selectable slots |
| Classic Analog | Full analog focus, cardinal numerals/ticks, date and small digital confirmation | No complication row; preference retained for other faces |
| Information Dashboard | Digital time and three aligned source rows | Three selectable slots |

240 is primary; 320/400 scale dimensions and font sizes explicitly. Dial contents are distinct from desktop case/crown/chrome. Click the dial for Apps; click a complication for Health & activity. No static watch-face background is used. SVG assets contain only alpha-masked perimeter illumination.

## Clock correctness

`presentation::tick` reads SystemClock exactly once per update; state/model synchronization is separate. That LocalTime snapshot supplies digital time, abbreviated weekday/day/month and the three angles. `watchos-core::hand_angles` calculates degrees clockwise from twelve:

- hour = (hour % 12) × 30 + minute × 0.5 + second / 120
- minute = minute × 6 + second × 0.1
- second = second × 6

Slint draws hand endpoints using sine/cosine vector paths; it does not rely on unsupported software-renderer rotation transforms. 12/24 format changes digital time only. Analog seconds are optional; hour/minute keep fractional movement. Invalid/missing time clears text and hides analog hands instead of inventing midnight. Calendar follows Windows local time/timezone. Tests cover midnight, noon, year/date rollover and invalid values.

## Settings and drafts

Core Settings keeps the existing use_24h, quiet, reduce_motion and brightness fields, with default-migrated Appearance. Appearance contains face, palette, show_date, show_seconds, show_complications, three ComplicationKind slots and glow 0–100. Unknown fields/enum names and invalid bounds are rejected. Phase 1 JSON with no appearance still loads and keeps all old fields; no automatic destructive rewrite.

BeginCustomization snapshots once. Changes alter visible state immediately but do not write disk while editing. Preview/Home retain the draft; other settings can join the same draft. Apply writes validated current settings; Cancel restores the snapshot. Restore defaults uses a draft and can be canceled. Failed save restores the original snapshot so Apply remains retryable. Existing file storage flushes a process-specific staging file before replace; no partial JSON overwrite on ordinary write failure.

The simulator binds numeric UI actions to typed core events. UI indices are private presentation details; wire enums are named serde values. Future Android synchronization uses protocol v2 Customization {appearance, use_24h, reduce_motion}; v1 original message types still validate. No Android transport or synchronization is implemented.

## Telemetry and connection

DataProvider returns validated core Telemetry. Default UnavailableHardware returns no readings. DataState distinguishes Live, Unavailable, PermissionRequired, Disconnected and Stale. Value presence and battery percentage are checked before state accepts data. No age threshold is guessed: a future provider must classify freshness using its actual sampling semantics.

Charlie wordmark reads core ConnectionState. Unavailable/disconnected are neutral, connecting has a temporary restrained pulse, connected uses cyan/violet edge lettering, assistant-active adds intensity only when connected. These are implemented styles, not a claim of an active connection. Native smoke injects connection states only to test bindings; normal runtime remains Unavailable.

`--design-preview` supplies explicit examples 72 / 5248 / 86%, marks shell and dial DESIGN PREVIEW, shows Example statuses, uses isolated settings, and never represents them as real sensors. Ordinary captures and normal mode show unavailable dashes.

## Native evidence

`--capture` opens a real native window, waits for boot/render, takes Slint's native software-renderer RGBA snapshot, writes PNG plus dial crop, and exits with failure if export fails. It never constructs a second mock renderer. The matrix in `artifacts/ui-phase2` covers every face at 240/320/400. Full snapshots include only client chrome; a separate native Windows screenshot documents actual interactive theme selection.

## Final activity / motion extension

A slot now stores `{kind, accent, visible, style}`. All 25 kinds are named and versioned; old string-only slot JSON is still read. Three slots remain in fixed reference-aligned positions; hiding one does not overcrowd/reflow the others. The picker, six accent choices, three compact styles and each metric detail route are functional and persist through the same draft/storage pipeline. See PROTOCOL.md and ICONS.md.

The normal provider reports unavailable for unsupported measurements; local brightness/quiet/settings are explicitly labeled local controls. Alarm/timer/stopwatch/music/weather/phone-finder provider behavior is not invented. Data validation accepts only valid state/value combinations, positive heart rate, and bounded battery/oxygen percentages. Small per-reading events avoid large event-enum copies; full snapshots validate atomically.

Millisecond precision from the same Windows clock snapshot provides continuous hand endpoints. Active Home sampling is approximately 33ms; off-screen/reduced motion sampling is one second. Rim phase includes the whole local time of day so minute boundaries remain continuous. Opacity is clamped to 1 even at maximum glow. Button feedback is short and disabled by reduced motion.

Per the owner correction, hands and center hub are the frontmost visual layer. They intentionally cross text when actual time positions require it; this is not an accidental layout collision. Native 05:10 fixtures, clearly marked DESIGN PREVIEW, assert that the white hour hand paints over the readout at a known pixel at 240/320/400.

Normal desktop viewing enlarges the logical display 1.5×; capture stays native 1×. The supplied photographic case is simulator chrome only, behind an opaque display. The actual face remains real text, vectors, icons and state, not reference wallpaper. The photo is not a hardware identity/fit claim.
