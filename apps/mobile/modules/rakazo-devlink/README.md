# Charlie WatchOS: Android development-link client

The Android side of the Charlie WatchOS development link. The client speaks the
same link version 1 as the Rust simulator over a plain TCP socket on the
loopback interface.

This is a **development tool**. It is not Charlie's assistant backend and it
has nothing to do with the physical watch: connecting to a simulator never
means Charlie is online and never means a watch is paired. The BLE module
(`rakazo-wrist`) stays separate and untouched.

## How the socket reaches the simulator

The simulator listens on `127.0.0.1` on the Windows host. On a device or
emulator the tunnel is made with ADB:

```powershell
adb reverse tcp:4599 tcp:4599
```

The client then connects to `127.0.0.1:4599` on the device.

## Wire format

Exactly the simulator's format, nothing else:

- a four-byte big-endian payload length, then
- a JSON link message.

The whole frame may not exceed 4096 bytes. The declared length is checked
before a payload buffer is allocated, and the simulator refuses anything
larger.

```json
{"link":1,"kind":"handshake","request_id":1,"token":"…"}
{"link":1,"kind":"message","request_id":7,"revision":4,"envelope":{"version":3,"id":7,"message":{"type":"customization","data":{…}}}}
{"link":1,"kind":"ack","request_id":8,"reply_to":7,"revision":5,"envelope":{…}}
{"link":1,"kind":"error","request_id":9,"reply_to":7,"code":"conflict","revision":5,"envelope":{…}}
```

`revision` is the link layer's own field: it records which committed state a
request was based on, so a stale request is refused instead of overwriting a
newer change. The protocol envelope inside is the simulator's own protocol v3
`Envelope` and is never reshaped.

## What the client does

- connects, presents the session token and waits for the simulator to answer
  the handshake; only then does the link count as connected;
- keeps the authoritative customization and its revision;
- applies a change on top of the last state the simulator reported, so a
  partial change never clears the parts it does not mention;
- matches acknowledgements by `reply_to`;
- reports a conflict with the authoritative state;
- receives changes the simulator committed on its own;
- sends a request again with the **same** request id when the simulator does
  not answer, so a retry can never apply a change twice;
- keeps the number of waiting requests and the number of attempts bounded;
- closes the socket and clears the token when it is stopped or destroyed.

The session token is supplied by the caller at connection time. It is kept in
memory for the length of the session, never written to disk and never logged.

## Build scope

The module only works in the Charlie Wrist Lab build (`com.rakazo.charliewristlab`).
In any other build every call is refused, so the production app cannot talk to
a development simulator. The module needs no permission of its own: the app
already declares `android.permission.INTERNET` and the link only talks to a
loopback address.

## JavaScript interface

`apps/mobile/lib/devlink.ts` is the typed entry point:

```ts
import { devLink, watchDevLinkState, watchDevLinkCustomization } from "../lib/devlink";

await devLink.connect({ port: 4599, token });
watchDevLinkCustomization((event) => {
  // event.customization is the typed watch customization
});
await devLink.applyCustomization({ appearance: { face: "classic" } });
```

`requestSync` and `applyCustomization` answer with the request id they used, so
the answer to a change can be told apart from the answer to a sync. The
`watchLinkState` reducer in `apps/mobile/lib/watch-link-state.ts` uses that to
tell a remote change apart from the answer to the screen's own request.

## The watch customization screen

`apps/mobile/app/(settings)/watch-face.tsx` configures the simulator: the four
watch faces, the five palettes, the three complication slots, the glow, the
date, the seconds, the complication visibility, the 24-hour clock and the
reduced motion.

The screen keeps the state the simulator reported and, on top of it, only the
values that were changed. A change is therefore always sent on top of the
newest state the simulator holds, which is what makes a stale revision
recoverable: the refused change is kept on the screen and the next attempt
carries it on top of the newer state.

The screen only exists in the Charlie Wrist Lab build. `isLabBuild` answers
which build is running, and the entry point in the Charlie Wrist screen is
hidden in every other build.

## Tests

- `android/src/test` holds JVM tests for the framing, the merge rules and the
  client: handshake, refused handshake, acknowledgement matching, conflicts,
  retries with the same request id, simulator initiated changes and cleanup.
- `android/src/androidTest` holds the on-device test that talks to a running
  simulator through `adb reverse`.
