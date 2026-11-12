# UI design system — Charlie WatchOS

## Direction

**Luminous instrument.** Reference-led hybrid dial: near-black TFT-oriented surface, restrained cyan/violet perimeter illumination, white analog hour hand, violet minute hand, large live digital time, three compact lower complications and small neutral Charlie wordmark. Mode: Operate. DFII: impact 5 + context 5 + feasibility 4 + performance 4 − consistency risk 4 = **14**. The asymmetric moving hand positions come from actual local time, never a fixed 10:08 pose.

The owner attached the authoritative reference; `references/hybrid-reference.png` is read-only visual evidence. Its photographic watch case, strap, studio lighting and example health measurements are not part of the logical UI. Numeral 6 is deliberately small between the complication row and wordmark to satisfy the requested four cardinal numerals at 240 pixels.

## Tokens

All palette-dependent colors live in `crates/watchos-ui/ui/theme.slint`. Neutral shell material colors are separate simulator chrome.

| Preset | Primary | Secondary |
|---|---|---|
| Cyan / Violet | #24E4ED | #B982FA |
| Electric Blue | #55AAFF | #677DFF |
| Emerald / Mint | #62EBBA | #A6FFDA |
| Sunset Orange | #FFAD66 | #FF646C |
| Rose / Purple | #FF8BC7 | #BC86FF |

Foreground #F1F5FC; secondary text #ADBBCD; dial #020508; control surface #15202E; pressed surface #233346; warning #FFC585. Heart #FF8BA5 and battery #83F0A4 preserve semantic identity across themes. Availability is also communicated by text/dashes, not color alone. Dial black is a visual choice, not an AMOLED power-saving assumption.

## Typography

Self-hosted SIL OFL **Outfit** supplies geometric numerals, wordmark and control typography; **DM Sans** supplies concise calendar and simulator metadata. Fonts ship with licenses, no runtime web font request. At 240: digital Hybrid 36 px, Minimal 48 px, cardinal numerals 22 px, section titles 17 px, controls/body 13 px, date 10.5 px, complication values 14 px, wordmark 12 px. Tiny 6/ticks are deliberately secondary; the actual time is primary. No emoji or icon font is used. Health/complication icons are authored vector geometry with consistent strokes.

## Layout

240×240 is the reference coordinate system and default/primary acceptance target. Scale is actual logical dial diameter / 240. Native window starts at diameter+112 by diameter+196, leaving case/crown/footer outside display. Dial is circular; static masks and vector paths stay within its circumference. 320 and 400 preserve composition, not an assumed physical board configuration.

Round-screen pages use 35 px horizontal, 25 px top and 27 px bottom margins at 240, with 7–10 px group gaps. Shared controls remain at least 44 px high; the 48 dp Wear Material recommendation is a useful native UX reference, not a claim that simulator pixels equal Android dp. Root shell Back/Home/Apps are 44 px controls. Scrollable lists reset on page change; wheel simulates crown-list scrolling. Long secondary content is scrollable, never compressed into unreadable paragraphs.

## Components and states

Theme tokens → shared WatchButton / Caption / Icon / Complication → WatchFace → native shell. Four faces share actual time and telemetry bindings. Settings share the same controls and selected-state tint. Focus borders, accessible names, Tab and Enter/Space are provided for shared buttons. The dial and complications expose accessibility labels; health details remain available through Apps.

Unavailable data is an em dash. Permission required is Allow; disconnected is Off; stale values carry * and explicit Stale reading in detail/accessibility text. Live values come only from validated provider state. Design preview is explicit, isolated, marked on both shell and dial, and labels readings Example.

## Motion and rendering

Clock updates once per second; fractional angles include seconds. No 60 Hz idle loop, continuous shimmer or shader. Connecting state alone uses a restrained one-second opacity target with 550 ms transition; reduced motion disables it. Other connection states are static. Connected wordmark adds cyan/violet edge lettering; assistant-active intensity is allowed only with connected state. No connection dot.

Slint 1.18.1 software rendering did not honor gradient-rounded-rectangle corners/path gradient strokes consistently. Decorative rim uses two small colorless SVG alpha masks, tinted at runtime by Theme.primary/secondary. They contain only perimeter geometry, not dial typography, hands, time or data. Solid surfaces preserve circular complication/case shapes. Bounds are explicit for numeral text; arbitrary transforms are avoided. Unchanged data models are reused across clock ticks, preserving repeater focus and avoiding unnecessary rebuilds.

## References and limits

[Wear Material 3 API](https://developer.android.com/reference/kotlin/androidx/wear/compose/material3/package-summary) informed round-friendly components, spacing and interaction priorities. No proprietary screens/assets were copied. UI/UX Pro Max's generic web output (liquid glass/landing-page layout) was rejected for this native TFT brief. The reference-led direction overrides generic skill aesthetic suggestions.

Visual evidence and measured/unmeasured limits live in PHASE2-VERIFICATION.md. Font rasterization, memory budget, physical touch calibration and real controller rendering must be evaluated in a future confirmed hardware port.
Complication visuals remain compact; their touch areas extend to at least 44×44 at the primary 240-pixel size. Oversized provider strings elide visually and remain complete in accessible labels / health details.

## Final audit tokens and motion

Theme also owns font families, body/small/title/digital/numeral/wordmark sizes, native line-height factor 1.0, 44px controls, 7px row gaps and 35/25/27px page safe areas. Compact rows are 48px with explicit label/subtitle bounds. Date and digital columns use explicit alignment; the day is accented. Lucide SVGs share 24×24 viewBox and 2px rounded strokes, with explicit selected/unselected/unavailable appearances and accessible labels.

The final design includes the supplied casing artwork only outside the opaque live display. This supersedes the earlier minimal-case simulator treatment. Hands/hub paint last, as explicitly requested by the owner. Earlier text-protection stacking is superseded. Active motion is limited to smooth real-time hands, a traveling perimeter highlight and gentle lower waves; reduced motion stops decorative movement. The complete audit inspected every screen at 240/320/400 and fixed the demonstrated text fragments.

Token contrast calculation: minimum 4.74:1 for normal labels and the selected foreground/background combinations across five palettes; dim unavailable icons are excluded and remain paired with full-contrast status labels. This is not a screen-reader certification or a physical TFT calibration.

## Final owner polish override
Hybrid no longer renders filled lower waves, inner bottom arcs or the 6 numeral. It keeps Charlie and only the animated colored outer ring. The approved SVG glyphs are unchanged. Metric artwork is laid out inside 44×36 painted capsules with a 44×44 hit region, consistent icon centers and value baselines. Text dimensions follow measured reference proportions, not blanket enlargement. Final tokens and screenshots are documented in PHASE2-FINAL-POLISH.md.
