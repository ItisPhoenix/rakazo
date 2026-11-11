# Charlie Wrist: Android BLE foundation

**Branch:** `charlie-wrist/ble-foundation` (fork: `ItisPhoenix/rakazo`)

This is an **unreleased, unbuilt** Android-only foundation for the user's KaHa **WA49V3** boAt Lunar Discovery, firmware **v4.00.69**.

## Verified on the physical watch before this change

- Nordic UART service `6e400001-b5a3-f393-e0a9-e50e24dcca9e`
- Android -> watch writes to `6e400002-...`; watch -> Android notifications on `6e400003-...`
- Read time, battery, trigger vibration, display a small custom Charlie notification.
- Receive Find My Phone event; transmit its acknowledgement.

These observations do **not** demonstrate an unlockable bootloader, firmware backup, or custom OS compatibility.

## Implemented (source only, not yet validated in an APK)

- Android Expo local module `RakazoWrist`, with a TypeScript facade in `apps/mobile/lib/wrist.ts`.
- Short, opt-in scan for `LDISCOVERY*` advertisements; local device selection by BLE address.
- Android runtime permission helper; no hardcoded address and no Internet requests.
- Connect and verify Device Information model `WA49V3` before enabling writes.
- Subscribe to UART responses and handle battery, Find Phone and notification acknowledgements.
- Four **allowlisted** actions: read battery, vibrate, send short notification, acknowledge Find Phone.
- Fail closed on unrecognized models; no raw-opcode API, firmware flashing, reset, or OTA operations.

## Scope and safety

- Existing notifications, Android voice features, API, and Ubuntu services are unchanged.
- The module is not connected to any app screen or server API; it does nothing until explicitly called.
- It does not provide background auto-reconnection, foreground service, arbitrary-length text messages, or a watch operating system.
- BLE addresses stay local to the mobile UI. Do not send addresses, HCI recordings, arbitrary GATT data, or notification contents to server telemetry.
- Android permissions must be granted explicitly.
- To test on the actual watch, disconnect nRF Connect and force-stop Crest first.

## Next steps

1. Add a minimal settings interface to pair, connect and display watch status locally.
2. Build a signed Android development APK from this branch; validate manifest/permission merge and Kotlin compilation.
3. Test discovery, model check, battery, vibration, notifications and Find Phone on the real WA49V3. Capture only redacted outcomes.
4. Review background execution, battery use, reconnection, and per-action authorization before connecting it to Charlie conversations.
5. Do not merge to `main` or install on the live phone until the build and on-device checks pass.
