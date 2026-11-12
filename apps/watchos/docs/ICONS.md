# Icons

The UI bundles the official Lucide SVGs from [`lucide-icons/lucide`](https://github.com/lucide-icons/lucide), pinned to release [`1.72.0`](https://github.com/lucide-icons/lucide/releases/tag/1.72.0), commit `03eae6069edfc373673bd05d723491f05ba936e3`. The source repository's ISC license is retained in [`ui/assets/icons/LICENSE`](../crates/watchos-ui/ui/assets/icons/LICENSE). The SVG files are unmodified upstream files and render locally without network access. Their 24 by 24 viewBox, rounded stroke caps and joins, and 2px stroke provide the unified visual style; Slint `Image.colorize` applies UI colors.

`ui/icons.slint` maps the integer kinds to bundled assets:

| Kind | Category | Lucide asset |
| ---: | --- | --- |
| 0 | Heart rate | `heart.svg` |
| 1 | Steps | `sport-shoe.svg` |
| 2 | Battery | `battery.svg` |
| 3 | Calories | `flame.svg` |
| 4 | Distance | `route.svg` |
| 5 | Sleep | `moon.svg` |
| 6 | Blood oxygen | `droplets.svg` |
| 7 | Workout | `dumbbell.svg` |
| 8 | Running | `activity.svg` |
| 9 | Walking | `person-standing.svg` |
| 10 | Cycling | `bike.svg` |
| 11 | Activity goals | `target.svg` |
| 12 | Exercise duration | `timer.svg` |
| 13 | Bluetooth | `bluetooth.svg` |
| 14 | Notifications | `bell.svg` |
| 15 | Weather | `cloud-sun.svg` |
| 16 | Alarm | `alarm-clock.svg` |
| 17 | Timer | `timer.svg` (shared with kind 12) |
| 18 | Stopwatch | `timer-reset.svg` |
| 19 | Music | `headphones.svg` (the pinned upstream release has no `music-2.svg` or `music.svg`) |
| 20 | Settings | `settings-2.svg` |
| 21 | Phone finder | `smartphone.svg` |
| 22 | Brightness | `sun.svg` |
| 23 | Do not disturb | `moon-star.svg` |
| 24 | Charlie | `audio-lines.svg` |
| Other | Help fallback | `circle-question-mark.svg` (Lucide renamed `circle-help`) |

The `disabled` state uses `Theme.muted` at 45% opacity. The `selected` state uses `Theme.primary`; otherwise `ink` is used. Each kind has an accessible label, and unknown kinds are labeled Help. Only the SVG assets listed above and the license are bundled; no remote icon service, bitmap, font, or emoji is used.

## Shared reference icon assets

The requested [heart](https://lucide.dev/icons/heart), [sport-shoe](https://lucide.dev/icons/sport-shoe) and [battery](https://lucide.dev/icons/battery) come from official Lucide commit `c55aab36b553a7de50a0050e7042fd7b2d64664e` (2026-11-13). The shared `Icon` component supplies them across the watch face, app launcher, pickers, detail screens and settings. Remaining icons stay pinned to Lucide 1.72.0. Both sources use ISC; the notice is bundled locally.

