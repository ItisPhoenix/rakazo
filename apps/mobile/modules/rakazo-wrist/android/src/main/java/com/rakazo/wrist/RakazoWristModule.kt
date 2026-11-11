package com.rakazo.wrist

import android.Manifest
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import java.util.Locale
import java.util.UUID

/**
 * Opt-in, foreground-only bridge for the owner's KaHa WA49V3 watch.
 *
 * Never flashes firmware, transmits raw user-supplied opcodes, or uploads addresses/packets.
 * A successful GATT connection is NOT authorization to write: model validation and notification
 * subscription must finish first. Other watches are rejected.
 */
class RakazoWristModule : Module() {
  private val main = Handler(Looper.getMainLooper())
  private var scanCallback: ScanCallback? = null
  private var scanGeneration = 0
  // Kept in memory for this short, owner-initiated scan only.
  private val seenScanAddresses = LinkedHashSet<String>()
  private var nearbyCandidateCount = 0
  private var gatt: BluetoothGatt? = null
  private var writeCharacteristic: BluetoothGattCharacteristic? = null
  private var notifyCharacteristic: BluetoothGattCharacteristic? = null
  private var ready = false

  override fun definition() = ModuleDefinition {
    Name("RakazoWrist")
    Events("onWristDevice", "onWristState", "onWristEvent")

    AsyncFunction("startScan") {
      requireScanPermissions()
      main.post { beginScan() }
      Unit
    }

    AsyncFunction("stopScan") {
      main.post { endScan() }
      Unit
    }

    AsyncFunction("connect") { address: String ->
      requireConnectPermission()
      require(BluetoothAdapter.checkBluetoothAddress(address)) { "Invalid BLE address" }
      main.post { connectTo(address) }
      Unit
    }

    AsyncFunction("disconnect") {
      main.post { closeConnection("disconnected") }
      Unit
    }

    AsyncFunction("readBattery") {
      requireConnectPermission()
      main.post { sendKnownPacket(byteArrayOf(0x00, 0x08, 0x04, 0x00)) }
      Unit
    }

    AsyncFunction("vibrate") {
      requireConnectPermission()
      main.post {
        sendKnownPacket(byteArrayOf(0x04, 0x81.toByte(), 0x08, 0x00, 0x01, 0x01, 0x50, 0x28))
      }
      Unit
    }

    AsyncFunction("sendNotification") { title: String, message: String ->
      requireConnectPermission()
      // Initial milestone uses a single small GATT packet; no unverified fragmentation.
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
      main.post { sendKnownPacket(packet) }
      Unit
    }

    OnDestroy {
      main.post {
        endScan()
        closeConnection("disconnected")
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

  private fun state(status: String) {
    sendEvent("onWristState", mapOf("status" to status))
  }

  private fun fail(message: String) {
    sendEvent("onWristState", mapOf("status" to "error", "reason" to message))
  }

  /**
   * Many KaHa watches omit the local name in advertisements. Show nearby candidates locally,
   * but NEVER treat an advertisement as proof of watch identity. GATT model WA49V3 must match.
   * No addresses, names or scan records leave this Android app.
   */
  private fun reportScanCandidate(result: ScanResult) {
    val address = result.device.address
    if (seenScanAddresses.contains(address)) return
    val rawName = result.scanRecord?.deviceName
      ?: runCatching { result.device.name }.getOrNull()
      ?: ""
    val displayName = rawName.trim().filterNot { Character.isISOControl(it) }.take(40)
    val name = displayName.uppercase(Locale.ROOT)
    val uartAdvertised = result.scanRecord?.serviceUuids?.any { it.uuid == UART_SERVICE } == true
    val likely = name.startsWith("LDISCOVERY") || name.contains("LUNAR") ||
      name.startsWith("WA49") || uartAdvertised

    // Discard weak, unrecognized ambient BLE traffic and bound UI/device scanning work.
    // Keep extra room for positively named/advertised watch candidates.
    if (!likely && result.rssi < -85) return
    if (!likely && nearbyCandidateCount >= 24) return
    if (seenScanAddresses.size >= 40) return
    if (!seenScanAddresses.add(address)) return
    if (!likely) nearbyCandidateCount++

    sendEvent(
      "onWristDevice",
      mapOf(
        "name" to (displayName.ifEmpty { "Unnamed nearby BLE device" }),
        "address" to address, // local JS state only, never included in displayed text or sent to API
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
      fail("Bluetooth is off or unavailable")
      return
    }
    val scanner = adapter.bluetoothLeScanner
    if (scanner == null) {
      fail("BLE scanner unavailable")
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
          fail("BLE scan failed (code $errorCode)")
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
      state("scanning")
      val generation = ++scanGeneration
      main.postDelayed({
        if (scanGeneration == generation) {
          endScan()
          state("scan-complete")
        }
      }, 15_000L)
    } catch (_: SecurityException) {
      endScan()
      fail("Bluetooth permission revoked")
    } catch (_: IllegalStateException) {
      endScan()
      fail("Bluetooth scanner is unavailable")
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

  private fun connectTo(address: String) {
    endScan()
    closeConnection(null)
    val adapter = manager().adapter
    if (adapter == null || !adapter.isEnabled) {
      fail("Bluetooth is off or unavailable")
      return
    }
    try {
      val device = adapter.getRemoteDevice(address)
      state("connecting")
      gatt = device.connectGatt(context(), false, callback, BluetoothDevice.TRANSPORT_LE)
      if (gatt == null) fail("Could not create BLE connection")
    } catch (_: SecurityException) {
      fail("Bluetooth permission revoked")
    } catch (_: IllegalArgumentException) {
      fail("Invalid watch address")
    }
  }

  private fun closeConnection(status: String?) {
    ready = false
    writeCharacteristic = null
    notifyCharacteristic = null
    val old = gatt
    gatt = null
    if (old != null) {
      try {
        old.disconnect()
      } catch (_: SecurityException) {
        // Disconnect may still be permitted after permissions are revoked.
      }
      old.close()
    }
    if (status != null) state(status)
  }

  private val callback = object : BluetoothGattCallback() {
    override fun onConnectionStateChange(connection: BluetoothGatt, status: Int, newState: Int) {
      main.post {
        if (connection !== gatt) return@post
        if (status == BluetoothGatt.GATT_SUCCESS && newState == BluetoothProfile.STATE_CONNECTED) {
          state("discovering")
          try {
            if (!connection.discoverServices()) {
              fail("Service discovery could not start")
              closeConnection("disconnected")
            }
          } catch (_: SecurityException) {
            fail("Bluetooth permission revoked")
            closeConnection("disconnected")
          }
        } else {
          closeConnection("disconnected")
        }
      }
    }

    override fun onServicesDiscovered(connection: BluetoothGatt, status: Int) {
      main.post {
        if (connection !== gatt) return@post
        if (status != BluetoothGatt.GATT_SUCCESS) {
          fail("Service discovery failed")
          closeConnection("disconnected")
          return@post
        }
        val uart = connection.getService(UART_SERVICE)
        writeCharacteristic = uart?.getCharacteristic(UART_RX)
        notifyCharacteristic = uart?.getCharacteristic(UART_TX)
        val model = connection.getService(DEVICE_INFORMATION)?.getCharacteristic(MODEL_NUMBER)
        if (writeCharacteristic == null || notifyCharacteristic == null || model == null) {
          fail("Required watch services unavailable")
          closeConnection("disconnected")
          return@post
        }
        // Fail closed: the model must match before any command can be transmitted.
        state("verifying-model")
        try {
          if (!connection.readCharacteristic(model)) {
            fail("Could not read watch model")
            closeConnection("disconnected")
          }
        } catch (_: SecurityException) {
          fail("Bluetooth permission revoked")
          closeConnection("disconnected")
        }
      }
    }

    @Suppress("DEPRECATION")
    override fun onCharacteristicRead(
      connection: BluetoothGatt,
      characteristic: BluetoothGattCharacteristic,
      status: Int,
    ) {
      handleModelRead(connection, characteristic, characteristic.value ?: byteArrayOf(), status)
    }

    override fun onCharacteristicRead(
      connection: BluetoothGatt,
      characteristic: BluetoothGattCharacteristic,
      value: ByteArray,
      status: Int,
    ) {
      handleModelRead(connection, characteristic, value, status)
    }

    override fun onDescriptorWrite(
      connection: BluetoothGatt,
      descriptor: BluetoothGattDescriptor,
      status: Int,
    ) {
      main.post {
        if (connection !== gatt || descriptor.uuid != CCC_DESCRIPTOR) return@post
        if (status == BluetoothGatt.GATT_SUCCESS) {
          ready = true
          state("ready")
        } else {
          fail("Could not subscribe to watch notifications")
          closeConnection("disconnected")
        }
      }
    }

    @Suppress("DEPRECATION")
    override fun onCharacteristicChanged(
      connection: BluetoothGatt,
      characteristic: BluetoothGattCharacteristic,
    ) {
      handleNotification(connection, characteristic, characteristic.value ?: byteArrayOf())
    }

    override fun onCharacteristicChanged(
      connection: BluetoothGatt,
      characteristic: BluetoothGattCharacteristic,
      value: ByteArray,
    ) {
      handleNotification(connection, characteristic, value)
    }
  }

  private fun handleModelRead(
    connection: BluetoothGatt,
    characteristic: BluetoothGattCharacteristic,
    value: ByteArray,
    status: Int,
  ) {
    main.post {
      if (connection !== gatt || characteristic.uuid != MODEL_NUMBER) return@post
      val model = value.toString(Charsets.UTF_8).trim('\u0000', ' ', '\n', '\r')
      if (status != BluetoothGatt.GATT_SUCCESS || model != "WA49V3") {
        fail("Device model is not the verified WA49V3")
        closeConnection("disconnected")
        return@post
      }
      val notifier = notifyCharacteristic ?: return@post
      val descriptor = notifier.getDescriptor(CCC_DESCRIPTOR)
      if (descriptor == null) {
        fail("Watch notification descriptor unavailable")
        closeConnection("disconnected")
        return@post
      }
      try {
        if (!connection.setCharacteristicNotification(notifier, true)) {
          fail("Unable to enable notifications")
          closeConnection("disconnected")
          return@post
        }
        val submitted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
          connection.writeDescriptor(
            descriptor,
            BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE,
          ) == BluetoothStatusCodes.SUCCESS
        } else {
          @Suppress("DEPRECATION")
          descriptor.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
          @Suppress("DEPRECATION")
          connection.writeDescriptor(descriptor)
        }
        if (!submitted) {
          fail("Cannot subscribe to notifications")
          closeConnection("disconnected")
        }
      } catch (_: SecurityException) {
        fail("Bluetooth permission revoked")
        closeConnection("disconnected")
      }
    }
  }

  private fun handleNotification(
    connection: BluetoothGatt,
    characteristic: BluetoothGattCharacteristic,
    value: ByteArray,
  ) {
    main.post {
      if (!ready || connection !== gatt || characteristic.uuid != UART_TX) return@post
      if (value.size < 4) return@post
      val declaredLength = (value[2].toInt() and 0xff) or ((value[3].toInt() and 0xff) shl 8)
      if (declaredLength != value.size) return@post
      when {
        value.size == 5 && value[0] == 0x80.toByte() && value[1] == 0x08.toByte() -> {
          val percentage = value[4].toInt() and 0xff
          if (percentage <= 100) {
            sendEvent("onWristEvent", mapOf("type" to "battery", "percent" to percentage))
          }
        }
        value.size == 6 && value[0] == 0x01.toByte() && value[1] == 0x05.toByte() &&
          value[4] == 0x01.toByte() -> {
          val active = value[5] == 0x01.toByte()
          sendEvent("onWristEvent", mapOf("type" to "find-phone", "active" to active))
          if (active) {
            // Exact acknowledgement previously sent successfully by the owner.
            sendKnownPacket(byteArrayOf(0x81.toByte(), 0x05, 0x05, 0x00, 0x01))
          }
        }
        value.size == 5 && value[0] == 0x82.toByte() && value[1] == 0x83.toByte() -> {
          sendEvent("onWristEvent", mapOf("type" to "notification-ack", "accepted" to
            (value[4] == 0x01.toByte())))
        }
      }
    }
  }

  private fun sendKnownPacket(data: ByteArray) {
    val connection = gatt
    val characteristic = writeCharacteristic
    if (!ready || connection == null || characteristic == null) {
      fail("Watch is not ready")
      return
    }
    try {
      val submitted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        connection.writeCharacteristic(
          characteristic,
          data,
          BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE,
        ) == BluetoothStatusCodes.SUCCESS
      } else {
        @Suppress("DEPRECATION")
        characteristic.writeType = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
        @Suppress("DEPRECATION")
        characteristic.value = data
        @Suppress("DEPRECATION")
        connection.writeCharacteristic(characteristic)
      }
      if (!submitted) fail("Watch rejected BLE write submission")
    } catch (_: SecurityException) {
      fail("Bluetooth permission revoked")
    }
  }

  private companion object {
    val UART_SERVICE: UUID = UUID.fromString("6e400001-b5a3-f393-e0a9-e50e24dcca9e")
    val UART_RX: UUID = UUID.fromString("6e400002-b5a3-f393-e0a9-e50e24dcca9e")
    val UART_TX: UUID = UUID.fromString("6e400003-b5a3-f393-e0a9-e50e24dcca9e")
    val DEVICE_INFORMATION: UUID = UUID.fromString("0000180a-0000-1000-8000-00805f9b34fb")
    val MODEL_NUMBER: UUID = UUID.fromString("00002a24-0000-1000-8000-00805f9b34fb")
    val CCC_DESCRIPTOR: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
  }
}
