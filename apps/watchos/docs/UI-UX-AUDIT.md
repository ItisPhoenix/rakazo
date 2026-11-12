# Phase 2 UI/UX audit

Status: **240-pixel visual pass complete for the supplied captures; manual interaction/accessibility acceptance pending.** The latest `screen-0..18-240.png` set and four 240-pixel face captures were reviewed after the text-layout and analog-layer updates. The 320/400 app-screen matrix, every palette/time case, and interaction/accessibility checks are outside this screenshot pass.

## Scope and evidence

Reviewed the readme, design-system and verification docs, `theme.slint`, `components.slint`, `faces.slint`, `watch.slint`, core navigation and simulator action mapping. Viewed `references/hybrid-reference.png`, the existing full-client Hybrid and Dashboard images, dial crops for all four faces at 240, and 320/400 crops for Hybrid, Minimal, Classic and Dashboard. Also viewed the example-data Hybrid crop and native Sunset theme interaction capture.

The image set in `artifacts/ui-phase2/` is the pre-integration native Windows Slint software-renderer output. The `*-dial.png` images are logical display crops. Reference coordinates below use the logical crop origin at its upper left; observations concern rendered glyph pixels in the images, not just overlapping declared layout or touch-area bounds. The supplied photographic reference is a design target, not a hardware calibration or physical screen capture.

## Observed baseline findings

1. **P1 — Hybrid readout collides with moving analog hand at 240/320/400.** In `hybrid-240-dial.png`, rendered hour-hand pixels cross the white `18` glyphs and hub in the digital time area (approximately x=104–119, y=137–160 in the 240-pixel crop). The same composition persists in `hybrid-320-dial.png` and `hybrid-400-dial.png`. It happens at the actual captured time; it is not a fixed reference pose. Bounded direction: reserve independent regions for hand sweep and digital readout, or otherwise keep glyphs legible across representative clock positions; inspect native captures after the change.

2. **P1 — Hybrid lower content is crowded at the primary size.** The rendered complication circles and small cardinal `6` nearly touch in `hybrid-240-dial.png`; the wordmark sits close to the lower circumference. At 320 and 400 the wordmark visibly crosses or crowds the lower perimeter arc (`hybrid-320-dial.png`, `hybrid-400-dial.png`). Bounded direction: give the lower data row, cardinal marker, wordmark and perimeter arc distinct vertical clearance while keeping all content within the round safe area.

3. **P2 — Classic’s live digital label intersects the moving hour hand.** In `classic-320-dial.png` and `classic-400-dial.png`, the downward-left hour hand passes through the region used for the date/time label; rendered strokes visibly cross the date in the captures. Bounded direction: protect the date/time glyphs from the hand sweep or relocate the date/time to a zone that remains clear for changing time.

4. **P2 — Small preview status is difficult to read.** The `DESIGN PREVIEW` mark in `hybrid-240-design-preview-dial.png` is rendered in a very small top-edge line, close to the perimeter and ticks. The full-shell status is legible, but the display-level indication is easy to miss at this scale. Bounded direction: preserve explicit example-data labeling inside the face with readable size and safe-area clearance.

5. **P2 — Baseline evidence does not cover page-level visual/accessibility states.** Existing native captures cover face/size combinations and two theme interactions; they do not provide screenshots for every app/settings/customization/detail page, focus state, keyboard flow, every palette, or reduced-motion behavior. This is a verification gap, not evidence that those unpictured states are defective.

The 240-pixel Minimal and Dashboard crops are comparatively clear in the observed static states. The viewed 320/400 crops for those faces, and 320/400 Classic, do not show a similarly clear text collision in the captured clock frames. That does not cover arbitrary minute/hour positions or interaction states.

These five findings describe the earlier `*-dial.png`/full captures only. Current face captures were reviewed separately below. In particular, hand/text intersections are intentionally omitted as defects because the requested hierarchy places analog hands and hub above the digital text; fresh captures after that layer change remain pending.

## Screen and flow inventory

Slint-visible screen values in `watch.slint`:

| UI page | Screen/state | Visible content and key path |
|---|---|---|
| 0 | Boot | Charlie / Starting; transitions to Home |
| 1 | Home / watch face | Four face variants, date, complications, status; dial opens Apps or Health |
| 2 | Apps launcher | Assistant, Notifications, Health, Quick settings, Settings, Connection |
| 3 | Charlie assistant | Unavailable state and connection details |
| 4 | Notifications | Empty state / unavailable phone transport |
| 5 | Quick settings | Quiet, preview light, reduced motion, Appearance |
| 6 | Connection | Connection status and unavailable Bluetooth/transport details |
| 7 | Settings | Faces, themes, appearance, complications, clock, device info, restore defaults |
| 8 | Device info | Build/platform statements and About Slint |
| 9 | Watch-face picker | Mini preview, four choices |
| 10 | Theme picker | Mini preview, five palettes |
| 11 | Appearance | Date, seconds, complications, glow, clock format, motion |
| 12 | Health & activity | Heart, steps, battery readings/status and provider caveat |
| 13 | Complications | Three source-slot controls and explanatory text |

Additional core states/actions need current implementation and capture mapping confirmed after integration: customization preview (action 30/Home while editing), Apply (31), Cancel (32), restore-defaults draft (24), and slot edit/appearance/detail flows in `watchos-core` (`SlotPicker`, `SlotAppearance`, `MetricDetail`, plus Utilities and IconGallery). They are not all represented as distinct page branches in the reviewed `watch.slint` baseline. Do not mark them as covered based on callback smoke tests.

Primary user journeys to capture end-to-end:

- Home → Apps → each launcher destination → Back/Home, including keyboard focus and rotary/wheel scrolling.
- Settings → Face picker or Theme picker → preview → Apply; Cancel; restore-defaults → Apply/Cancel; failed-save warning and retryable draft.
- Settings → Appearance and Complications → each slot edit/style/source → Home; include face variants that do not render complications.
- Home complication → Health details, plus unavailable/example states.
- Quick Settings → quiet, preview light and reduced motion; verify each state and persistence behavior.
- Back/Escape/Backspace, Home/H and Apps/A across nested and editing screens.

## Central token proposal

Keep palette colors, font sizes and logical spacing in shared `Theme` values. Existing visual baseline defines Outfit for display/control type and DM Sans for concise metadata. Suggested 240-logical-pixel starting values (not final approval):

| Token group | Values to centralize |
|---|---|
| Type | display 36; secondary digital 48; screen title 17; body/control 13; caption 13; metadata 10–11; wordmark 12 |
| Dial safe-area/layout | horizontal inset 35; top 25; bottom 27; screen heading 23 high; content spacing 7–10 |
| Controls | minimum hit/control height 44; button side text inset 9; root Back/Home/Apps 44 high |
| Face geometry | 240 reference diameter; safe inset and lower-row/wordmark clearances; keep those separate from simulator-shell dimensions |

Use the current logical-diameter scale for face artwork and page paddings, but preserve the 44-pixel minimum for control targets. The 240/320/400 matrix must confirm that font growth and safe areas do not create wraps, clipping or awkward empty space. Keep palette-specific semantic colors centralized; ensure state is not communicated through color alone. These are baseline proposals for the parent’s in-progress token set.

## Acceptance checklist

- [x] Review the latest 19 screen captures and four face captures at 240 px; keep earlier captures labeled as baseline.
- [ ] All four face variants at 240, 320 and 400: changing time frames, text glyph bounds, analog hand/readout collisions, perimeter, complication row, numeral six, logo, date, preview status and every palette as applicable.
- [ ] Every screen/state listed above plus current core-only detail flows: native screen image at 240, then spot-check tight safe-area and typography states at 320/400. Current 240 image matrix is complete; larger app-screen captures and core-only interaction flows are pending.
- [x] Record rendered text/glyph pixel bounds for observed cutoffs; do not infer a visible defect from element/touch-area boxes.
- [ ] Icons: visual stroke/optical size, semantic distinction and clipping reviewed; accessible name/role/action still needs interaction/screen-reader verification.
- [ ] Navigation: pointer, wheel/crown simulation, Back/Home/Apps, keyboard focus order, Enter/Space, focus visibility, return destinations and scroll reset.
- [ ] Accessibility: names/roles/state and reading order; visible focus; color contrast and non-color state cues; zoom/large text behavior if supported; reduced-motion behavior checked in native UI.
- [ ] Performance claims limited to available measurements; screenshots or a 1 Hz binding design do not establish frame time, memory, battery or hardware performance.
- [ ] Confirm current code fixes each finding with new screenshots and actual interaction evidence; update this document with result and evidence paths only then.

**Baseline acceptance was open.** No fresh screenshots for the parent's current UI integration were available during the original baseline pass. No claim is made about physical watch dimensions, pixels-per-inch, touch calibration, controller behavior, memory, frame time, or motion comfort on hardware.

## Current native screen review — 240 px

Reviewed every supplied full-client screenshot at `artifacts/ui-phase2/screen-{0..18}-240.png` and the four 240-pixel face screenshots listed below. These are static captures, not an interaction, screen-reader, reduced-motion, or performance run. Each client capture is 352×436 with a 240×240 logical dial; coordinates below are full-client image coordinates.

| Image | Visual review | Result / follow-up |
|---|---|---|
| `screen-0-240.png` | Boot | Centered status; no visible clipping. Shell says Desktop preview. |
| `screen-1-240.png` | Home / Hybrid | Core elements legible and inside dial in the current live-time frame. See the explicit design fixture for hands-last layering; no arbitrary-time capture matrix is represented by this frame. |
| `screen-2-240.png` | Apps | Rows align and read clearly. Later rows are beyond scroll viewport; wheel/crown discoverability and navigation not verified by this image. |
| `screen-3-240.png` | Charlie | Status and explanatory text fit; no visible cutoff. |
| `screen-4-240.png` | Notifications | Empty state centered; no visible cutoff. |
| `screen-5-240.png` | Quick settings | First rows readable; later content continues below scroll viewport. No partial text glyph observed. |
| `screen-6-240.png` | Connection | Status/body hierarchy clear; no visible cutoff. |
| `screen-7-240.png` | Settings | First rows readable; later settings continue below scroll viewport. |
| `screen-8-240.png` | Device info | **Pass after update:** three compact information blocks now display complete text at rest; the former fragment at the viewport edge is gone. |
| `screen-9-240.png` | Watch-face picker | Preview and selected Hybrid row readable; only first option is visible at rest, remaining choices require scrolling. Verify scroll affordance/focus order. |
| `screen-10-240.png` | Theme picker | Preview and selected Cyan/Violet row readable; remaining choices require scrolling. Verify scroll affordance/focus order. |
| `screen-11-240.png` | Appearance | First controls legible; more options continue below viewport. No visible partial label in this frame. |
| `screen-12-240.png` | Health & activity | Heart rate, steps, calories rows readable with unavailable status; no visible cutoff. |
| `screen-13-240.png` | Complications | Three slot rows and detail labels readable; no visible cutoff. |
| `screen-14-240.png` | Choose metric | Selected metric and other available rows readable; list continues below viewport. |
| `screen-15-240.png` | Heart Rate detail | **Pass after update:** the full three-line note (“No supported provider / connected. No readings / generated.”) is visible in the dial, including its final line. The former cutoff is gone. |
| `screen-16-240.png` | Utilities | First rows readable; additional utilities continue below viewport. |
| `screen-17-240.png` | Icon library | **Pass after update:** all 25 glyphs remain aligned in a 5×5 grid; numeric captions are larger (10px source, previously 8px) and legible in this capture. Screenshot alone cannot establish accessible names. |
| `screen-18-240.png` | Slot appearance | Selected complication, metric and accent controls readable; remaining controls continue below viewport. Verify that visibility/style controls are reachable by wheel and keyboard. |

### Face captures

| Image | Visual review | Result / follow-up |
|---|---|---|
| `hybrid-240.png` | Hybrid | Readout, date, complications and wordmark legible in the captured live-time frame. The final hand/hub-over-text order is also visibly represented by the separate explicit design fixture below; this face screenshot is not itself a full clock-position matrix. |
| `minimal-240.png` | Minimal Digital | Strong time hierarchy, readable date and complication icons, clear lower wordmark; no visible text cutoff. |
| `classic-240.png` | Classic Analog | Date/time legible in the captured live-time frame with no other visible glyph cutoff. The parent's rendered-pixel assertions cover hand layering at 240/320/400; no arbitrary-time capture matrix was supplied here. |
| `dashboard-240.png` | Information Dashboard | Labels and unavailable values legible; row alignment consistent; no visible text cutoff. |

### Findings and limits

- **Resolved in the latest captures:** Device info no longer exposes the partial caption at rest; Heart Rate detail now shows its full note; icon-gallery numeric captions increased to 10px and are legible in the updated image.
- Viewed `hands-front-fixture-240.png`: this is explicitly marked DESIGN PREVIEW · EXAMPLES and renders 05:10 as a design fixture, not a live clock claim. The hour hand/hub visibly sit above the readout as requested. The parent also reports rendered-pixel assertions passed at 240/320/400; those assertions are not independent physical-clock evidence.
- No new text cutoff or alignment regression was visible in the recaptured screens 0, 3, 4, 5, 6, 9, 10, 11, 13 or 18. Partial next rows at the bottom of list screens read as intentional scroll hints; actual reachability still needs manual interaction verification.
- Theme contrast results were supplied by the parent: 4.74:1 minimum for labels and active palette 5. This screenshot pass did not independently calculate contrast ratios; disabled icons are excluded from that reported result.
- The picker/settings/list captures show normal scroll continuation. Their scroll affordance, focus sequence, tap targets, accessibility semantics and return navigation require actual interaction checks; screenshots alone do not prove them.
- All 19 latest screen captures and four latest face captures were viewed. This pass does not establish behavior for 320/400 app screens, every palette, keyboard/screen-reader interaction, reduced-motion state, frame time, memory, or real hardware. Parent is separately checking manual navigation and scroll reachability.
- The photographic case remains visible outside the opaque logical dial by design. It is simulator chrome and gives no evidence about a physical case, display bezel, or hardware fit.

**Visual status for the supplied 240-pixel captures: pass with resolved findings.** Overall UI acceptance remains pending the parent's manual navigation/accessibility checks and any required 320/400 page review.


## Parent functional confirmation
Native wheel scrolling reached lower picker and appearance controls. Metric selection, accent, visibility, style, preview, Apply, process-restart reload, detail tap and Preview → Back were exercised. Full screen-reader certification and hardware checks remain unverified. Computer Use was later stopped by physical Escape; no further UI input was sent. See PHASE2-VERIFICATION.md for the final executed-check scope.
