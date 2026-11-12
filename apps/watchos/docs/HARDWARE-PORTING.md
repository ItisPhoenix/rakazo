# Future hardware port — separate from Phase 1 simulator

## Owner-supplied evidence (2026-11-12)

The owner reports WA49V3 Bluetooth recordings contain:

| Field | Reported value |
|---|---|
| Manufacturer/company ID | `0x05D6` — Zhuhai JieLi Technology |
| BLE subversion | `0x0772` |
| Bluetooth Classic subversion | `0x22BB` |

Treat **JieLi as the leading Bluetooth hardware vendor**. These values were supplied in chat; this implementation session did not independently inspect the recordings. They do not establish the exact MCU, main application processor, CPU architecture, memory map or peripheral configuration. Bluetooth vendor identity alone does not prove which processor runs the watch application.

This newer evidence supersedes any assumption that Realtek is the leading Bluetooth vendor. Historical reverse-engineering material remains unchanged, as instructed. No Realtek or JieLi driver exists in this project.

## Future JieLi SDK integration task

This is a separate hardware-porting phase, not a dependency or acceptance condition for the working Windows simulator:

1. Correlate the reported Bluetooth evidence with authoritative board/chip identity and application-processor ownership, using separately authorized research.
2. Obtain the correct vendor SDK, toolchain and documentation legally; review licensing and Rust/C interoperability against the confirmed processor and SDK ABI.
3. Establish actual display geometry, memory limits, input wiring, storage, time source, vibration and connectivity semantics. Do not derive these from Bluetooth subversions.
4. Implement the existing watchos-platform contracts against verified vendor APIs in a separate adapter. Select a Rust compilation target only after architecture is established. Evaluate Embassy only if that target/SDK supports it.
5. Select a compatible Slint embedded backend/renderer and licensing path; budget resources using measured board limits.
6. Require a separately authorized, verified recovery/deployment procedure before any device write. This document authorizes no flashing, OTA, bootloader change or firmware transfer.

The `thumbv7em-none-eabi` check in Phase 1 is only a generic no_std portability probe. It is **not** a proposed WA49V3 target. Core/protocol/HAL remain hardware-independent; Windows time, file storage and desktop rendering remain simulator adapters.
