# Native UI visual audit — 400 px captures

Status: **post-fix visual review complete for all 23 current captures; interaction and acceptance review remains open.** Rechecked every `artifacts/ui-phase2/screen-{0..18}-400.png` and `hybrid-400.png`, `minimal-400.png`, `classic-400.png`, and `dashboard-400.png` after the postfix recapture. These are actual native Slint-window captures. Coordinates below refer approximately to the displayed PNG canvas (512 × 574), including simulator shell and footer, and locate only visible evidence.

## Current visual status

1. **Resolved — Metric detail final line.** In the current `screen-15-400.png`, “No supported provider connected. No readings generated.” fits in the visible content area; no glyph clipping is visible.
2. **Resolved — Device information block.** In the current `screen-8-400.png`, all three compact information blocks, including “TFT design target / MCU / sensors unknown,” fit fully inside the visible area.
3. **Intentional scroll hints — Partial next controls.** `screen-2-400.png` (Apps), `screen-5-400.png` (Quick settings), `screen-7-400.png` (Settings), `screen-11-400.png` (Appearance), and `screen-18-400.png` (Slot appearance) show only the upper portion of an unlabeled next rounded control at the viewport edge. Per the owner, these are intentional scroll hints. No text is visibly cut in the fragments; interaction/reachability will be checked separately.
4. **Icon grid labels legible.** In the current `screen-17-400.png`, numeric diagnostic labels 0–24 appear consistently sized and readable beneath the indexed glyphs.
5. **Hands and hubs are foregrounded on Hybrid and Classic.** Current `hybrid-400.png` and `classic-400.png` show the hand/hub layer in front. The separate `hands-front-fixture-400.png` is explicitly marked design preview at 05:10; it is not a live-time capture.

No other clear typography, spacing, alignment, safe-area, contrast, icon-state, or clipping regression was visible in the rechecked stills. No interaction, keyboard focus, dynamic content, or unsupplied state is inferred from these stills.

## Per-screen review

| ID | Capture | Visual review |
|---:|---|---|
| 0 | `screen-0-400.png` | Starting state is centered and legible; shell/footer labels fit. |
| 1 | `screen-1-400.png` | Hybrid face: date/time, complication glyphs, values, rim and wordmark are visible; hands and hub render in the foreground. |
| 2 | `screen-2-400.png` | Apps title and visible rows align; partial next control at bottom (P3). |
| 3 | `screen-3-400.png` | Unavailable state, explanatory copy and action fit the round safe area and remain readable. |
| 4 | `screen-4-400.png` | Empty notifications state has clear hierarchy and readable multiline copy. |
| 5 | `screen-5-400.png` | Quick settings rows are aligned and legible; partial next control at bottom (P3). |
| 6 | `screen-6-400.png` | Connection status/copy is readable; final “No pairing or network” line sits at the lower scroll edge, with continuation outside the visible content. |
| 7 | `screen-7-400.png` | Settings title and rows are aligned; partial next control at bottom (P3). |
| 8 | `screen-8-400.png` | All three compact device/platform information blocks fit completely and remain readable. |
| 9 | `screen-9-400.png` | Face picker preview, selection, shell navigation and Cancel/Apply controls fit; labels are readable. |
| 10 | `screen-10-400.png` | Theme preview and “Cyan / Violet” selection are centered and legible; action controls fit. |
| 11 | `screen-11-400.png` | Appearance controls are readable and consistently padded; partial next control at bottom (P3). |
| 12 | `screen-12-400.png` | Health rows have consistent icon/text alignment and clear unavailable values. |
| 13 | `screen-13-400.png` | Complication slot rows are aligned; visible rows and edit controls fit. |
| 14 | `screen-14-400.png` | Metric choices are readable, with selected tint and unavailable values visible. |
| 15 | `screen-15-400.png` | Metric title/icon/value/status are centered; the complete final explanatory sentence fits. |
| 16 | `screen-16-400.png` | Utility rows are evenly spaced and readable; no visible gap or cutoff in shown rows. |
| 17 | `screen-17-400.png` | Icon library’s 25 glyphs are evenly placed in the 5×5 grid; numeric diagnostic labels 0–24 are consistently sized and legible. |
| 18 | `screen-18-400.png` | Slot icon, source, accent and visible controls are legible; partial next control at bottom (P3). |

## Face variants

| Capture | Visual review |
|---|---|
| `hybrid-400.png` | Date/time, three complications, perimeter and wordmark are visible; hands and hub render in the foreground. |
| `minimal-400.png` | Large digital time and date read clearly; complication spacing and wordmark fit inside the circular display. |
| `classic-400.png` | Analog markers, date, digital time and wordmark are visible; hands and hub render in the foreground. |
| `dashboard-400.png` | Digital readout and three labeled rows are aligned and readable; unavailable values use clear dashes. |

## Limits

This is a pixel-level visual review of the listed stills. It does not verify behavior while scrolling, navigation, pointer/crown/keyboard actions, state transitions, accessibility tree/focus, other resolutions or themes, changing clock positions, physical hardware calibration, or performance. Current captures reflect the hand-layer ordering fix; final interaction reachability and hardware review remain open.


## Parent functional confirmation
Native wheel scrolling reached lower picker and appearance controls. Metric selection, accent, visibility, style, preview, Apply, process-restart reload, detail tap and Preview → Back were exercised. Full screen-reader certification and hardware checks remain unverified. Computer Use was later stopped by physical Escape; no further UI input was sent. See PHASE2-VERIFICATION.md for the final executed-check scope.
