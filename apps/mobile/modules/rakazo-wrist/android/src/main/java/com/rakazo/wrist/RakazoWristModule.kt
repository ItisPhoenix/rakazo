package com.rakazo.wrist

import android.Manifest
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import expo.modules.kotlin.Promise
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import java.util.Locale

/**
 * Opt-in, foreground-only Expo bridge for the owner's KaHa WA49V3 watch.
 *
 * Delegates singleton connection management, background persistence,
 * audio/ringtone handling, and bounded auto-reconnection to WristBridge.
 */
class RakazoWristModule : Module() {
  private val main = Handler(Looper.getMainLooper())
  private var scanCallback: ScanCallback? = null
  private var scanGeneration = 0
  private val seenScanAddresses = LinkedHashSet<String>()
  private var nearbyCandidateCount = 0

  override fun definition() = ModuleDefinition {
    Name("RakazoWrist")
    Events("onWristDevice", "onWristState", "onWristEvent", "onWristDeviceInfo")

    OnCreate {
      try {
        context()?.let { WristBridge.initContext(it) }
      } catch (_: Exception) {
      }
      WristBridge.eventEmitter = { eventName, payload ->
        try {
          sendEvent(eventName, payload)
        } catch (_: Exception) {
        }
      }
    }

    AsyncFunction("startScan") {
      requireScanPermissions()
      main.post { beginScan() }
      Unit
    }

    AsyncFunction("stopScan") {
      main.post { endScan() }
      Unit
    }

    AsyncFunction("connect") { address: String, readOnly: Boolean? ->
      requireConnectPermission()
      require(BluetoothAdapter.checkBluetoothAddress(address)) { "Invalid BLE address" }
      main.post { WristBridge.connect(context(), address, readOnly = readOnly ?: false, isInspection = false) }
      Unit
    }

    AsyncFunction("connectReadOnly") { address: String ->
      requireConnectPermission()
      require(BluetoothAdapter.checkBluetoothAddress(address)) { "Invalid BLE address" }
      main.post { WristBridge.connect(context(), address, readOnly = true, isInspection = false) }
      Unit
    }

    AsyncFunction("inspectDevice") { address: String, promise: Promise ->
      requireConnectPermission()
      require(BluetoothAdapter.checkBluetoothAddress(address)) { "Invalid BLE address" }
      main.post {
        WristBridge.connect(context(), address, readOnly = true, isInspection = true, promise = promise)
      }
    }

    AsyncFunction("disconnect") {
      main.post { WristBridge.disconnect(context()) }
      Unit
    }

    AsyncFunction("getState") {
      val ctx = context()
      ctx?.let { WristBridge.initContext(it) }
      WristBridge.getState(ctx)
    }

    AsyncFunction("getDeviceInfo") {
      WristBridge.getDeviceInfo()
    }

    AsyncFunction("readBattery") {
      requireConnectPermission()
      main.post {
        if (WristBridge.mode == WristBridge.WristMode.READ_ONLY) {
          WristBridge.state("error", "Cannot send packets in read-only mode")
          return@post
        }
        Log.i(TAG, "KaHa MKI: Submitting battery query [00 08 04 00]")
        WristBridge.sendKnownPacket(byteArrayOf(0x00, 0x08, 0x04, 0x00))
      }
      Unit
    }

    AsyncFunction("vibrate") {
      requireConnectPermission()
      main.post {
        if (WristBridge.mode == WristBridge.WristMode.READ_ONLY) {
          WristBridge.state("error", "Cannot send packets in read-only mode")
          return@post
        }
        WristBridge.sendKnownPacket(byteArrayOf(0x04, 0x81.toByte(), 0x08, 0x00, 0x01, 0x01, 0x50, 0x28))
      }
      Unit
    }

    AsyncFunction("sendNotification") { title: String, message: String ->
      requireConnectPermission()
      if (WristBridge.mode == WristBridge.WristMode.READ_ONLY) {
        WristBridge.state("error", "Cannot send packets in read-only mode")
        return@AsyncFunction
      }
      require(title.isNotBlank() && message.isNotBlank()) { "Notification must have text" }
      require(title.none { Character.isISOControl(it) } && message.none { Character.isISOControl(it) }) {
        "Notification contains control characters"
      }
      val titleBytes = title.toByteArray(Charsets.UTF_8)
      val messageBytes = message.toByteArray(Charsets.UTF_8)
      val length = 6 + titleBytes.size + messageBytes.size
      require(length <= 20) { "Single-packet notification exceeds 20 bytes" }
      val packet = byteArrayOf(0x02, 0x83.toByte(), length.toByte(), 0x00, 0x11) +
        titleBytes + byteArrayOf(0xff.toByte()) + messageBytes
      main.post { WristBridge.sendKnownPacket(packet) }
      Unit
    }

    AsyncFunction("stopFindPhone") {
      main.post { WristBridge.stopFindPhoneAlert() }
      Unit
    }

    AsyncFunction("isFindPhoneActive") {
      WristBridge.isFindPhoneActive
    }

    AsyncFunction("setPersistent") { enabled: Boolean ->
      main.post { WristBridge.setPersistentConfigured(context(), enabled) }
      Unit
    }

    AsyncFunction("isPersistentEnabled") {
      WristBridge.isPersistentConfigured(context())
    }

    AsyncFunction("reconnect") {
      requireConnectPermission()
      main.post { WristBridge.triggerManualReconnect(context()) }
      Unit
    }

    OnDestroy {
      main.post {
        endScan()
      }
    }
  }

  private fun context(): Context =
    appContext.reactContext ?: error("RakazoWrist requires an Android app context")

  private fun manager(): BluetoothManager =
    context().getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager

  private fun requireConnectPermission() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
      context().checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) != PackageManager.PERMISSION_GRANTED
    ) throw SecurityException("Bluetooth Nearby Devices permission is required")
  }

  private fun requireScanPermissions() {
    requireConnectPermission()
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      if (context().checkSelfPermission(Manifest.permission.BLUETOOTH_SCAN) != PackageManager.PERMISSION_GRANTED) {
        throw SecurityException("Bluetooth scanning permission is required")
      }
    } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M &&
      context().checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED
    ) {
      throw SecurityException("Location permission is required for BLE scanning on this Android version")
    }
  }

  private fun reportScanCandidate(result: ScanResult) {
    val address = result.device.address
    if (seenScanAddresses.contains(address)) return
    val rawName = result.scanRecord?.deviceName
      ?: runCatching { result.device.name }.getOrNull()
      ?: ""
    val displayName = rawName.trim().filterNot { Character.isISOControl(it) }.take(40)
    val name = displayName.uppercase(Locale.ROOT)
    val uartAdvertised = result.scanRecord?.serviceUuids?.any { it.uuid == WristBridge.UART_SERVICE } == true
    val likely = name.startsWith("LDISCOVERY") || name.contains("LUNAR") ||
      name.startsWith("WA49") || uartAdvertised

    if (!likely && result.rssi < -85) return
    if (!likely && nearbyCandidateCount >= 24) return
    if (seenScanAddresses.size >= 40) return
    if (!seenScanAddresses.add(address)) return
    if (!likely) nearbyCandidateCount++

    sendEvent(
      "onWristDevice",
      mapOf(
        "name" to (displayName.ifEmpty { "Unnamed nearby BLE device" }),
        "address" to address,
        "rssi" to result.rssi,
        "likelyWatch" to likely,
      ),
    )
  }

  private fun beginScan() {
    endScan()
    seenScanAddresses.clear()
    nearbyCandidateCount = 0
    val adapter = manager().adapter
    if (adapter == null || !adapter.isEnabled) {
      WristBridge.state("error", "Bluetooth is off or unavailable")
      return
    }
    val scanner = adapter.bluetoothLeScanner
    if (scanner == null) {
      WristBridge.state("error", "BLE scanner unavailable")
      return
    }
    val callback = object : ScanCallback() {
      override fun onScanResult(callbackType: Int, result: ScanResult) {
        main.post {
          if (scanCallback === this) reportScanCandidate(result)
        }
      }

      override fun onBatchScanResults(results: MutableList<ScanResult>) {
        main.post {
          if (scanCallback === this) results.forEach { reportScanCandidate(it) }
        }
      }

      override fun onScanFailed(errorCode: Int) {
        main.post {
          if (scanCallback !== this) return@post
          endScan()
          WristBridge.state("error", "BLE scan failed (code $errorCode)")
        }
      }
    }
    scanCallback = callback
    try {
      val settings = ScanSettings.Builder()
        .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
        .setReportDelay(0L)
        .build()
      scanner.startScan(null, settings, callback)
      WristBridge.state("scanning")
      val generation = ++scanGeneration
      main.postDelayed({
        if (scanGeneration == generation) {
          endScan()
          WristBridge.state("scan-complete")
        }
      }, 15_000L)
    } catch (_: SecurityException) {
      endScan()
      WristBridge.state("error", "Bluetooth permission revoked")
    } catch (_: IllegalStateException) {
      endScan()
      WristBridge.state("error", "Bluetooth scanner is unavailable")
    }
  }

  private fun endScan() {
    scanGeneration++
    val callback = scanCallback ?: return
    scanCallback = null
    try {
      manager().adapter?.bluetoothLeScanner?.stopScan(callback)
    } catch (_: SecurityException) {
      // Permission may have been revoked while scanning.
    }
  }

  companion object {
    private const val TAG = "RakazoWrist"
  }
}
