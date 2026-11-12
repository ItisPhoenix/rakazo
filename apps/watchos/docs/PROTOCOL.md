# Charlie protocol v3

Transport-neutral JSON types for future Charlie Android/watch integration. No Android synchronization or transport is active.

```json
{"version":3,"id":7,"message":{"type":"connection","data":"unavailable"}}
```

Messages retain the v2 shapes for connection, notification, assistant, action, and customization. Customization contains `appearance`, `use_24h`, and `reduce_motion`. Appearance keeps its existing face, palette, date, seconds, complication visibility, and glow fields; `complications` remains exactly three entries, now slot objects:

```json
{"kind":"heart_rate","accent":"coral","visible":true,"style":"icon_value"}
```

The 25 complication kinds have stable `repr(u8)` indices 0–24 in `ComplicationKind::ALL`; JSON uses snake_case. Accents are `theme`, `coral`, `cyan`, `mint`, `amber`, `violet`. Styles are `icon_value`, `icon_only`, and `value_only`. Defaults retain the original Heart Rate, Steps, Battery order and use Coral, Cyan, Mint accents, visible slots, and `icon_value` style.

Migration is lossless for existing preferences. An old settings file without `appearance` gets the established defaults. Older appearance objects whose three `complications` entries are kind strings are accepted and upgraded to slot objects when next serialized; the prior face, palette, toggles, glow, and other settings remain intact. Legacy Heart Rate, Steps, and Battery slots receive Coral, Cyan, and Mint accents respectively; other legacy kinds receive Theme. New producers serialize only slot objects.

New envelopes use version 3. Validation accepts versions 1, 2, and 3; v1 remains limited to its original connection/notification/assistant/action messages. V2 customization with legacy kind-string slots remains accepted. Unknown fields/types reject during serde decoding, and appearance glow must be at most 100. `Envelope::validate()` must run after bounded decoding and before changing application state.

Envelope `id` correlates messages within a future transport session; it is not an exactly-once delivery guarantee. Assistant `request_id` correlates requests/responses. Customization drafts are local state: Begin snapshots once, edits stay in memory, Apply requests persistence only when settings differ, and Cancel restores the snapshot. Defining a customization message does not send or synchronize it with Android.

Telemetry is a separate local core model, not a wire message. The original heart-rate, steps, and battery fields remain available; 22 additional readings map to kinds 3–24. Every reading defaults to `unavailable` with no value. Live and stale readings require values; other states require no value. Heart rate must be positive, battery and blood oxygen at most 100. Invalid updates are ignored and telemetry never changes persistent settings. No metric value is synthesized when hardware has not supplied one.

Bounds count UTF-8 bytes: app ≤64, title ≤128, body ≤1024, assistant text ≤2048, assistant request ≤1024 and nonblank. `MAX_FRAME_BYTES = 4096` is the ingress byte cap. A future adapter must enforce this **before allocating/parsing** and reject oversized frames; serde heap strings alone do not bound untrusted allocations. JSON escaping may make otherwise field-valid messages exceed the frame cap; reject those frames too. No unbounded queue or JSON parser is exposed by the simulator.

Future transport must supply authentication, explicit pairing/consent, session/reconnect semantics, timeouts, replay handling, payload privacy, and bounded queues before real notifications or assistant actions are enabled. These types do not provide those guarantees. Never map these messages to proprietary watch UART opcodes.

Core/protocol/platform can compile without std. Protocol still needs alloc (`String`); fixed-capacity payloads may replace it when board memory constraints are known. Embassy is an integration option, not a current dependency.
