# 320 px UI visual audit

Reviewed all 19 `screen-{0..18}-320.png` captures and all four face captures in `artifacts/ui-phase2/` at 320 logical pixels. Coordinates below use the displayed PNG's top-left origin and its pixel dimensions (432 × 518), so they can be found directly in the screenshots. This is a visual review of captured states; it does not establish behavior after scrolling, tapping, or connecting a provider.

## Current findings

1. **Screens 2, 5, 7, 11, and 18 — partial next capsule at list end (low).** A thin top strip of the next row remains visible at approximately **y=336–342** (screen 18: teal selected capsule around **y=329–342**). It exposes no row label, but reads as an abruptly clipped control. Prefer a clean row boundary at the initial position. These screenshots alone do not show whether the rows are reachable by scrolling.

### Resolved in the recaptures

- **Screen 8:** all Device info text, including `MCU / sensors unknown`, is fully visible inside the display.
- **Screen 15:** the full final note line, `generated.`, now fits above the bottom edge.
- **Screen 17:** the icon labels are visibly legible in the recapture.
- **Screen 13:** the earlier detached-Preview finding was a false positive. The recapture clearly shows Preview as a footer pill at approximately **x=287–413, y=403–447**, aligned with Back and Home, plus the editor's Cancel/Apply controls.

## Per-screen review

| ID | Screenshot | Visual check |
|---:|---|---|
| 0 | `screen-0-320.png` | Startup title and status are centered and readable; Back/Home/Apps footer controls are evenly spaced. |
| 1 | `screen-1-320.png` | Hybrid face numerals, date, digital time, and complications fit the display; no visible hand/text collision. |
| 2 | `screen-2-320.png` | Apps title and visible rows are readable; partial next capsule at bottom, see finding 3. |
| 3 | `screen-3-320.png` | Unavailable state and connection action fit; paragraph wraps cleanly and remains readable. |
| 4 | `screen-4-320.png` | Empty notification state is centered and readable. |
| 5 | `screen-5-320.png` | Quick-setting labels fit their controls; partial next capsule at bottom, see finding 3. |
| 6 | `screen-6-320.png` | Connection status and explanatory copy fit; lines remain legible within the face. |
| 7 | `screen-7-320.png` | Settings controls are aligned; partial next capsule at bottom, see finding 3. |
| 8 | `screen-8-320.png` | Device name, logical dimensions, protocol/software, target, and MCU/sensors status all fit within the display. |
| 9 | `screen-9-320.png` | Watch-face preview is centered; selected face and Cancel/Apply controls are distinguishable and aligned. |
| 10 | `screen-10-320.png` | Theme preview and selected theme label are legible and centered. |
| 11 | `screen-11-320.png` | Appearance labels and selected states are legible; partial next capsule at bottom, see finding 3. |
| 12 | `screen-12-320.png` | Heart Rate, Steps, and Calories rows fit; labels and unavailable states align. |
| 13 | `screen-13-320.png` | Three complication rows and the Back/Home/Preview footer pills are aligned; Cancel/Apply controls are visible below. |
| 14 | `screen-14-320.png` | Metric choices and unavailable states fit and align. |
| 15 | `screen-15-320.png` | Heart Rate icon, status, and all three lines of the note fit; `generated.` is no longer clipped. |
| 16 | `screen-16-320.png` | All three utility rows fit with aligned labels and statuses. |
| 17 | `screen-17-320.png` | Icon grid is evenly spaced; icons and 10 px numeric identifiers remain within the display and are legible. |
| 18 | `screen-18-320.png` | Slot title, icon, metric, and accent are readable; partial next capsule at bottom, see finding 1. |

## Face previews

| Screenshot | Visual check |
|---|---|
| `hybrid-320.png` | Date, digital time, complications, and branding fit the circular display; hands and hub render in front with no visible text collision. |
| `minimal-320.png` | Digital time and complication controls are centered; accent arcs and branding stay inside the face. |
| `classic-320.png` | Analog dial, date, digital time, and branding remain legible and within the display; hands and hub render in front. |
| `dashboard-320.png` | Date and three metric rows align; unavailable dashes share a right edge. |

No interaction state was inferred from a static screenshot. In particular, the list captures do not prove that partly visible rows can or cannot be reached by scrolling. The 320 px captures were re-opened after the layout recapture; resolved items above refer to the current screenshot set.


## Parent functional confirmation
Native wheel scrolling reached lower picker and appearance controls. Metric selection, accent, visibility, style, preview, Apply, process-restart reload, detail tap and Preview → Back were exercised. Full screen-reader certification and hardware checks remain unverified. Computer Use was later stopped by physical Escape; no further UI input was sent. See PHASE2-VERIFICATION.md for the final executed-check scope.
