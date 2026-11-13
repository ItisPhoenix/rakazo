package com.rakazo.wrist

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.content.Context
import android.content.SharedPreferences
import android.media.AudioAttributes
import android.media.Ringtone
import android.media.RingtoneManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.util.Log
import expo.modules.kotlin.Promise
import java.util.ArrayDeque
import java.util.Locale
import java.util.UUID

/**
 * Singleton Bluetooth & Audio Engine for Charlie Wrist.
 *
 * Ensures:
 * 1. Exactly ONE native connection owner exists (no duplicate GATT connections).
 * 2. Background persistence via WristService (FOREGROUND_SERVICE_CONNECTED_DEVICE).
 * 3. Bounded exponential backoff reconnection; zero continuous scanning.
 * 4. Local audible ringtone & vibration for Find Phone with safe deduplication.
 * 5. Strict model verification (WA49V3) and read-only mode safety guards.
 */
object WristBridge {
  private const val TAG = "RakazoWrist"
  private const val PREFS_NAME = "charlie_wrist_prefs"
  private const val KEY_SAVED_ADDRESS = "saved_address"
  private const val KEY_PERSISTENT_ENABLED = "persistent_enabled"

  private val main = Handler(Looper.getMainLooper())

  @Volatile private var gatt: BluetoothGatt? = null
  @Volatile private var writeCharacteristic: BluetoothGattCharacteristic? = null
  @Volatile private var notifyCharacteristic: BluetoothGattCharacteristic? = null

  @Volatile private var ready = false
  @Volatile private var modelVerified = false
  @Volatile private var currentState = "disconnected"
  @Volatile private var stateGeneration = 0L
  @Volatile private var lastDeviceInfo: Map<String, Any?>? = null

  enum class WristMode {
    NONE,
    READ_ONLY,
    ACTIVE,
  }

  private data class PendingRead(
    val field: String,
    val characteristic: BluetoothGattCharacteristic,
    val isString: Boolean,
  )

  @Volatile var mode = WristMode.NONE
    private set

  private var isOneShotInspection = false
  @Volatile var currentAddress: String? = null
    private set

  private var activeInspectionPromise: Promise? = null

  private var connectionTimeoutRunnable: Runnable? = null
  private var discoveryTimeoutRunnable: Runnable? = null
  private var readTimeoutRunnable: Runnable? = null

  private val readQueue = ArrayDeque<PendingRead>()
  private var currentPendingRead: PendingRead? = null
  private val inspectionData = LinkedHashMap<String, Any?>()
  private val unsupportedReasons = LinkedHashMap<String, String>()
  private val readErrors = LinkedHashMap<String, String>()

  // Reconnection state machine
  private val BACKOFF_DELAYS = listOf(2_000L, 5_000L, 10_000L, 30_000L, 60_000L)
  @Volatile private var reconnectAttempts = 0
  private var reconnectRunnable: Runnable? = null
  @Volatile private var isExplicitDisconnect = false

  // Find Phone Audio state
  @Volatile private var findPhoneRingtone: Ringtone? = null
  @Volatile private var findPhoneVibrator: Vibrator? = null
  @Volatile var isFindPhoneActive = false
    private set

  // Service & Event listener
  @Volatile private var appContext: Context? = null
  private var activeService: WristService? = null
  var eventEmitter: ((eventName: String, payload: Map<String, Any?>) -> Unit)? = null

  fun initContext(context: Context) {
    if (appContext == null) {
      appContext = context.applicationContext
    }
  }

  private fun resolveContext(context: Context? = null): Context? =
    context ?: appContext ?: activeService

  private fun getPrefs(context: Context? = null): SharedPreferences? {
    val ctx = resolveContext(context) ?: return null
    return ctx.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
  }

  fun isPersistentConfigured(context: Context? = null): Boolean =
    getPrefs(context)?.getBoolean(KEY_PERSISTENT_ENABLED, false) ?: false

  fun getSavedAddress(context: Context? = null): String? =
    getPrefs(context)?.getString(KEY_SAVED_ADDRESS, null)

  fun setPersistentConfigured(context: Context, enabled: Boolean) {
    initContext(context)
    getPrefs(context)?.edit()?.putBoolean(KEY_PERSISTENT_ENABLED, enabled)?.apply()
    if (!enabled) {
      stopAutoReconnect()
      WristService.stop(context)
    } else if (currentAddress != null) {
      WristService.start(context)
    }
    state(currentState)
  }

  fun registerServiceInstance(service: WristService) {
    activeService = service
    initContext(service)
  }

  fun unregisterServiceInstance(service: WristService) {
    if (activeService === service) {
      activeService = null
    }
  }

  fun onServiceStarted(context: Context) {
    initContext(context)
    if (gatt == null && isPersistentConfigured(context)) {
      val saved = getSavedAddress(context)
      if (saved != null) {
        Log.i(TAG, "WristService started with persistent watch; initiating connection to $saved")
        main.post { connect(context, saved, readOnly = false, isInspection = false) }
      }
    }
  }

  fun getForegroundNotificationText(): String {
    return when (currentState) {
      "ready" -> "Connected to Lunar Discovery WA49V3"
      "read-only-ready" -> "Connected (Read-Only Mode)"
      "connecting" -> "Connecting to watch..."
      "discovering" -> "Configuring watch link..."
      "reading-info" -> "Reading watch telemetry..."
      "reconnecting" -> "Reconnecting to watch (${reconnectAttempts + 1}/${BACKOFF_DELAYS.size})..."
      "reconnect-paused" -> "Watch not in range. Tap to reconnect."
      else -> "Watch link idle"
    }
  }

  private fun updateServiceNotification() {
    activeService?.updateNotification(getForegroundNotificationText())
  }

  private fun emit(eventName: String, payload: Map<String, Any?>) {
    eventEmitter?.invoke(eventName, payload)
  }

  fun state(status: String, reason: String? = null) {
    stateGeneration++
    currentState = status
    updateServiceNotification()
    val payload = mutableMapOf<String, Any>(
      "status" to status,
      "generation" to stateGeneration,
      "persistentEnabled" to isPersistentConfigured(),
    )
    if (reason != null) payload["reason"] = reason
    emit("onWristState", payload)
  }

  private fun fail(message: String) {
    state("error", message)
  }

  fun getState(context: Context? = null): Map<String, Any> = mapOf(
    "status" to currentState,
    "generation" to stateGeneration,
    "address" to (currentAddress ?: getSavedAddress(context) ?: ""),
    "ready" to ready,
    "mode" to mode.name.lowercase(Locale.ROOT),
    "modelVerified" to modelVerified,
    "isRinging" to isFindPhoneActive,
    "reconnectAttempt" to reconnectAttempts,
    "persistentEnabled" to isPersistentConfigured(context),
  )

  fun getDeviceInfo(): Map<String, Any?>? = lastDeviceInfo

  // ==================== BLE Connection Management ====================

  fun connect(
    context: Context,
    address: String,
    readOnly: Boolean,
    isInspection: Boolean,
    promise: Promise? = null,
  ) {
    stopAutoReconnect()
    isExplicitDisconnect = false
    closeConnection(null)

    val manager = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
    val adapter = manager?.adapter
    if (adapter == null || !adapter.isEnabled) {
      fail("Bluetooth is off or unavailable")
      promise?.reject("BLUETOOTH_OFF", "Bluetooth is off or unavailable", null)
      return
    }

    mode = if (readOnly) WristMode.READ_ONLY else WristMode.ACTIVE
    isOneShotInspection = isInspection
    currentAddress = address
    activeInspectionPromise = promise
    modelVerified = false
    lastDeviceInfo = null

    inspectionData.clear()
    unsupportedReasons.clear()
    readErrors.clear()
    readQueue.clear()
    currentPendingRead = null

    if (!readOnly && !isInspection) {
      // Save address for persistent auto-reconnect
      getPrefs(context)?.edit()?.putString(KEY_SAVED_ADDRESS, address)?.apply()
      if (isPersistentConfigured(context)) {
        WristService.start(context)
      }
    }

    try {
      val device = adapter.getRemoteDevice(address)
      state("connecting")

      val timeoutRunnable = Runnable {
        main.post {
          if (gatt != null && !ready && mode != WristMode.NONE) {
            fail("BLE connection timed out")
            closeConnection("disconnected", context)
          }
        }
      }
      connectionTimeoutRunnable = timeoutRunnable
      main.postDelayed(timeoutRunnable, CONNECTION_TIMEOUT_MS)

      gatt = device.connectGatt(context, false, gattCallback, BluetoothDevice.TRANSPORT_LE)
      if (gatt == null) {
        cancelConnectionTimeout()
        fail("Could not create BLE connection")
        closeConnection("disconnected", context)
      }
    } catch (_: SecurityException) {
      cancelConnectionTimeout()
      fail("Bluetooth permission revoked")
      closeConnection("disconnected", context)
    } catch (_: IllegalArgumentException) {
      cancelConnectionTimeout()
      fail("Invalid watch address")
      closeConnection("disconnected", context)
    }
  }

  fun disconnect(context: Context) {
    isExplicitDisconnect = true
    stopAutoReconnect()
    setPersistentConfigured(context, false)
    closeConnection("disconnected", context)
    WristService.stop(context)
  }

  private fun closeConnection(status: String?, context: Context? = null) {
    cancelConnectionTimeout()
    cancelDiscoveryTimeout()
    cancelReadTimeout()
    readQueue.clear()
    currentPendingRead = null
    ready = false
    modelVerified = false
    writeCharacteristic = null
    notifyCharacteristic = null
    currentAddress = null

    val promise = activeInspectionPromise
    activeInspectionPromise = null
    if (promise != null && status != "inspected") {
      promise.reject("DISCONNECTED", status ?: "Disconnected", null)
    }

    val oldGatt = gatt
    gatt = null
    if (oldGatt != null) {
      try {
        oldGatt.disconnect()
      } catch (_: SecurityException) {
      }
      try {
        oldGatt.close()
      } catch (_: Exception) {
      }
    }

    mode = WristMode.NONE
    if (status != null) {
      state(status)
    } else {
      currentState = "disconnected"
    }

    if (status == "disconnected") {
      lastDeviceInfo = null
      val ctx = resolveContext(context)
      if (ctx != null && !isExplicitDisconnect && isPersistentConfigured(ctx)) {
        scheduleAutoReconnect(ctx)
      }
    }
  }

  private fun scheduleAutoReconnect(context: Context) {
    if (isExplicitDisconnect || !isPersistentConfigured(context)) return
    val saved = getSavedAddress(context) ?: return

    if (reconnectAttempts < BACKOFF_DELAYS.size) {
      val delay = BACKOFF_DELAYS[reconnectAttempts]
      Log.i(TAG, "Watch disconnected; scheduling bounded reconnect #${reconnectAttempts + 1} in ${delay}ms")
      state("reconnecting")
      val task = Runnable {
        main.post {
          reconnectAttempts++
          connect(context, saved, readOnly = false, isInspection = false)
        }
      }
      reconnectRunnable = task
      main.postDelayed(task, delay)
    } else {
      Log.w(TAG, "Max reconnect attempts (${BACKOFF_DELAYS.size}) reached. Pausing auto-reconnect to protect battery.")
      state("reconnect-paused")
    }
  }

  fun stopAutoReconnect() {
    reconnectRunnable?.let { main.removeCallbacks(it) }
    reconnectRunnable = null
    reconnectAttempts = 0
  }

  fun triggerManualReconnect(context: Context) {
    val saved = getSavedAddress(context)
    if (saved != null) {
      stopAutoReconnect()
      connect(context, saved, readOnly = false, isInspection = false)
    }
  }

  fun onBluetoothTurnedOn(context: Context) {
    if (gatt == null && isPersistentConfigured(context) && !isExplicitDisconnect) {
      val saved = getSavedAddress(context)
      if (saved != null) {
        stopAutoReconnect()
        connect(context, saved, readOnly = false, isInspection = false)
      }
    }
  }

  fun onBluetoothTurnedOff() {
    stopAutoReconnect()
    closeConnection("disconnected")
  }

  private fun cancelConnectionTimeout() {
    connectionTimeoutRunnable?.let { main.removeCallbacks(it) }
    connectionTimeoutRunnable = null
  }

  private fun cancelDiscoveryTimeout() {
    discoveryTimeoutRunnable?.let { main.removeCallbacks(it) }
    discoveryTimeoutRunnable = null
  }

  private fun cancelReadTimeout() {
    readTimeoutRunnable?.let { main.removeCallbacks(it) }
    readTimeoutRunnable = null
  }

  // ==================== GATT Callbacks ====================

  private val gattCallback = object : BluetoothGattCallback() {
    override fun onConnectionStateChange(connection: BluetoothGatt, status: Int, newState: Int) {
      main.post {
        if (connection !== gatt) return@post
        cancelConnectionTimeout()
        Log.d(TAG, "GATT: Connection state changed (status=$status, newState=$newState)")
        if (status == BluetoothGatt.GATT_SUCCESS && newState == BluetoothProfile.STATE_CONNECTED) {
          reconnectAttempts = 0
          state("discovering")
          val timeoutRunnable = Runnable {
            main.post {
              if (connection === gatt && mode != WristMode.NONE) {
                fail("Service discovery timed out")
                closeConnection("disconnected")
              }
            }
          }
          discoveryTimeoutRunnable = timeoutRunnable
          main.postDelayed(timeoutRunnable, DISCOVERY_TIMEOUT_MS)
          try {
            if (!connection.discoverServices()) {
              cancelDiscoveryTimeout()
              fail("Service discovery could not start")
              closeConnection("disconnected")
            }
          } catch (_: SecurityException) {
            cancelDiscoveryTimeout()
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
        cancelDiscoveryTimeout()
        if (status != BluetoothGatt.GATT_SUCCESS) {
          Log.w(TAG, "GATT: Service discovery failed (status=$status)")
          fail("Service discovery failed (status $status)")
          closeConnection("disconnected")
          return@post
        }

        Log.i(TAG, "GATT: Services discovered (${connection.services.size} services found)")
        state("reading-info")
        buildReadQueue(connection)
        processNextRead(connection)
      }
    }

    @Suppress("DEPRECATION")
    override fun onCharacteristicRead(
      connection: BluetoothGatt,
      characteristic: BluetoothGattCharacteristic,
      status: Int,
    ) {
      handleCharacteristicRead(connection, characteristic, characteristic.value ?: byteArrayOf(), status)
    }

    override fun onCharacteristicRead(
      connection: BluetoothGatt,
      characteristic: BluetoothGattCharacteristic,
      value: ByteArray,
      status: Int,
    ) {
      handleCharacteristicRead(connection, characteristic, value, status)
    }

    override fun onDescriptorWrite(
      connection: BluetoothGatt,
      descriptor: BluetoothGattDescriptor,
      status: Int,
    ) {
      main.post {
        if (connection !== gatt || descriptor.uuid != CCC_DESCRIPTOR) return@post
        if (mode == WristMode.READ_ONLY) {
          closeConnection("disconnected")
          return@post
        }
        if (status == BluetoothGatt.GATT_SUCCESS) {
          Log.i(TAG, "GATT: UART notifications subscribed successfully (ready)")
          ready = true
          state("ready")
        } else {
          Log.w(TAG, "GATT: Failed to subscribe to UART notifications (status $status)")
          fail("Could not subscribe to watch notifications (status $status)")
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

  // ==================== Characteristic Reading Queue ====================

  private fun buildReadQueue(connection: BluetoothGatt) {
    readQueue.clear()
    inspectionData.clear()
    unsupportedReasons.clear()
    readErrors.clear()

    val dis = connection.getService(DEVICE_INFORMATION)
    if (dis == null) {
      unsupportedReasons["model"] = "Device Information Service (0x180A) not found"
      unsupportedReasons["firmware"] = "Device Information Service (0x180A) not found"
      unsupportedReasons["hardware"] = "Device Information Service (0x180A) not found"
    } else {
      checkAndQueue(dis, MODEL_NUMBER, "model", isString = true)
      checkAndQueue(dis, FIRMWARE_REVISION, "firmware", isString = true)
      checkAndQueue(dis, HARDWARE_REVISION, "hardware", isString = true)
    }

    val bas = connection.getService(BATTERY_SERVICE)
    if (bas == null) {
      unsupportedReasons["battery"] = "Battery Service (0x180F) not found"
    } else {
      checkAndQueue(bas, BATTERY_LEVEL, "battery", isString = false)
    }
  }

  private fun checkAndQueue(
    service: android.bluetooth.BluetoothGattService,
    uuid: UUID,
    field: String,
    isString: Boolean,
  ) {
    val characteristic = service.getCharacteristic(uuid)
    if (characteristic == null) {
      unsupportedReasons[field] = "Characteristic $uuid not found in service ${service.uuid}"
      return
    }
    val hasRead = (characteristic.properties and BluetoothGattCharacteristic.PROPERTY_READ) != 0
    if (!hasRead) {
      val hexProp = Integer.toHexString(characteristic.properties)
      unsupportedReasons[field] = "Characteristic $uuid does not support READ property (properties=0x$hexProp)"
      return
    }
    readQueue.add(PendingRead(field, characteristic, isString))
  }

  private fun processNextRead(connection: BluetoothGatt) {
    cancelReadTimeout()
    if (readQueue.isEmpty()) {
      currentPendingRead = null
      onAllReadsCompleted(connection)
      return
    }

    val next = readQueue.removeFirst()
    currentPendingRead = next

    val timeout = Runnable {
      main.post {
        if (connection === gatt && currentPendingRead === next) {
          Log.w(TAG, "GATT: Read timed out for characteristic ${next.field}")
          readErrors[next.field] = "GATT read timed out after ${READ_TIMEOUT_MS}ms"
          currentPendingRead = null
          processNextRead(connection)
        }
      }
    }
    readTimeoutRunnable = timeout
    main.postDelayed(timeout, READ_TIMEOUT_MS)

    Log.d(TAG, "GATT: Reading characteristic: ${next.field} (${next.characteristic.uuid})")
    try {
      @Suppress("DEPRECATION")
      val ok = connection.readCharacteristic(next.characteristic)
      if (!ok) {
        cancelReadTimeout()
        Log.w(TAG, "GATT: readCharacteristic rejected request for ${next.field}")
        readErrors[next.field] = "BluetoothGatt.readCharacteristic rejected request"
        currentPendingRead = null
        processNextRead(connection)
      }
    } catch (_: SecurityException) {
      cancelReadTimeout()
      fail("Bluetooth permission revoked")
      closeConnection("disconnected")
    }
  }

  private fun handleCharacteristicRead(
    connection: BluetoothGatt,
    characteristic: BluetoothGattCharacteristic,
    value: ByteArray,
    status: Int,
  ) {
    main.post {
      if (connection !== gatt) return@post
      val pending = currentPendingRead ?: return@post
      if (pending.characteristic.uuid != characteristic.uuid) return@post

      currentPendingRead = null
      cancelReadTimeout()

      Log.d(TAG, "GATT: Read finished for ${pending.field} (status=$status)")
      if (status == BluetoothGatt.GATT_SUCCESS) {
        if (pending.isString) {
          val text = value.toString(Charsets.UTF_8).trim { it <= ' ' || it == '\u0000' }
          inspectionData[pending.field] = text
        } else {
          if (value.isNotEmpty()) {
            val level = value[0].toInt() and 0xff
            inspectionData[pending.field] = level
          } else {
            readErrors[pending.field] = "Received empty battery value"
          }
        }
      } else {
        readErrors[pending.field] = "GATT read failed with status $status"
      }

      processNextRead(connection)
    }
  }

  private fun onAllReadsCompleted(connection: BluetoothGatt) {
    val model = inspectionData["model"] as? String
    modelVerified = (model == VERIFIED_MODEL_NAME)

    val payload = mapOf(
      "address" to (currentAddress ?: ""),
      "model" to model,
      "modelVerified" to modelVerified,
      "firmwareRevision" to (inspectionData["firmware"] as? String),
      "hardwareRevision" to (inspectionData["hardware"] as? String),
      "batteryLevel" to (inspectionData["battery"] as? Int),
      "discoveredServices" to connection.services.map { it.uuid.toString() },
      "unsupported" to unsupportedReasons.toMap(),
      "readErrors" to readErrors.toMap(),
    )

    lastDeviceInfo = payload
    Log.i(TAG, "GATT: All characteristic reads completed. modelVerified=$modelVerified")
    emit("onWristDeviceInfo", payload)

    val promise = activeInspectionPromise
    activeInspectionPromise = null

    if (mode == WristMode.READ_ONLY) {
      if (isOneShotInspection) {
        state("inspected")
        promise?.resolve(payload)
        closeConnection("disconnected")
      } else {
        state("read-only-ready")
        promise?.resolve(payload)
      }
      return
    }

    if (!modelVerified) {
      fail("Device model '$model' does not match verified $VERIFIED_MODEL_NAME")
      promise?.reject("MODEL_MISMATCH", "Device model is not $VERIFIED_MODEL_NAME", null)
      closeConnection("disconnected")
      return
    }

    promise?.resolve(payload)

    val uart = connection.getService(UART_SERVICE)
    writeCharacteristic = uart?.getCharacteristic(UART_RX)
    notifyCharacteristic = uart?.getCharacteristic(UART_TX)
    if (writeCharacteristic == null || notifyCharacteristic == null) {
      fail("Required watch UART service unavailable")
      closeConnection("disconnected")
      return
    }

    state("verifying-model")
    subscribeToUartNotifications(connection)
  }

  private fun subscribeToUartNotifications(connection: BluetoothGatt) {
    if (mode == WristMode.READ_ONLY) {
      fail("Cannot subscribe to notifications in read-only mode")
      closeConnection("disconnected")
      return
    }
    if (!modelVerified) {
      fail("Cannot subscribe to notifications: model not verified")
      closeConnection("disconnected")
      return
    }
    val notifier = notifyCharacteristic ?: return
    val descriptor = notifier.getDescriptor(CCC_DESCRIPTOR)
    if (descriptor == null) {
      fail("Watch notification descriptor unavailable")
      closeConnection("disconnected")
      return
    }
    try {
      if (!connection.setCharacteristicNotification(notifier, true)) {
        Log.w(TAG, "GATT: setCharacteristicNotification returned false")
        fail("Unable to enable notifications")
        closeConnection("disconnected")
        return
      }
      Log.d(TAG, "GATT: Writing CCCD descriptor to enable UART TX notifications")
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

  // ==================== Incoming Notification Handler ====================

  private fun handleNotification(
    connection: BluetoothGatt,
    characteristic: BluetoothGattCharacteristic,
    value: ByteArray,
  ) {
    main.post {
      if (!ready || connection !== gatt || characteristic.uuid != UART_TX) return@post
      if (mode == WristMode.READ_ONLY) return@post
      if (value.size < 4) return@post
      val declaredLength = (value[2].toInt() and 0xff) or ((value[3].toInt() and 0xff) shl 8)
      if (declaredLength != value.size) return@post

      when {
        // Battery query response (0x80 0x08)
        value.size == 5 && value[0] == 0x80.toByte() && value[1] == 0x08.toByte() -> {
          val percentage = value[4].toInt() and 0xff
          if (percentage <= 100) {
            val rawHex = value.joinToString("") { "%02X".format(it) }
            Log.i(TAG, "KaHa MKI: RX battery response ($percentage%, raw: 0x$rawHex)")
            emit(
              "onWristEvent",
              mapOf(
                "type" to "battery",
                "percent" to percentage,
                "rawHex" to rawHex,
                "timestamp" to System.currentTimeMillis(),
              ),
            )
          }
        }
        // Find Phone event from watch (0x01 0x05 0x06 0x00 0x01 0x01 / 0x00)
        value.size == 6 && value[0] == 0x01.toByte() && value[1] == 0x05.toByte() &&
          value[4] == 0x01.toByte() -> {
          val active = value[5] == 0x01.toByte()
          Log.i(TAG, "KaHa MKI: RX find-phone event (active=$active)")
          handleFindPhonePacket(active)
          if (active && mode == WristMode.ACTIVE) {
            sendKnownPacket(byteArrayOf(0x81.toByte(), 0x05, 0x05, 0x00, 0x01))
          }
        }
        // Single-packet notification ACK (0x82 0x83)
        value.size == 5 && value[0] == 0x82.toByte() && value[1] == 0x83.toByte() -> {
          val accepted = value[4] == 0x01.toByte()
          Log.i(TAG, "KaHa MKI: RX notification-ack (accepted=$accepted)")
          emit("onWristEvent", mapOf("type" to "notification-ack", "accepted" to accepted))
        }
        else -> {
          val rawHex = value.joinToString("") { "%02X".format(it) }
          Log.d(TAG, "KaHa MKI: RX notification (${value.size} bytes): 0x$rawHex")
        }
      }
    }
  }

  // ==================== Find My Phone Audio & Vibration Engine ====================

  private fun handleFindPhonePacket(active: Boolean) {
    if (active) {
      startFindPhoneAlert()
    } else {
      stopFindPhoneAlert()
    }
  }

  fun startFindPhoneAlert(context: Context? = null) {
    if (isFindPhoneActive) {
      Log.d(TAG, "Find Phone: Already ringing; ignoring duplicate trigger")
      return
    }
    isFindPhoneActive = true
    emit("onWristEvent", mapOf("type" to "find-phone", "active" to true))

    val ctx = resolveContext(context) ?: return
    try {
      val alarmUri = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM)
        ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE)

      val ringtone = RingtoneManager.getRingtone(ctx, alarmUri)
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
        ringtone?.audioAttributes = AudioAttributes.Builder()
          .setUsage(AudioAttributes.USAGE_ALARM)
          .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
          .build()
      }
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        ringtone?.isLooping = true
      }
      ringtone?.play()
      findPhoneRingtone = ringtone

      val vibrator = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        val manager = ctx.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager
        manager?.defaultVibrator
      } else {
        @Suppress("DEPRECATION")
        ctx.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
      }
      val pattern = longArrayOf(0, 500, 400, 500, 400, 800)
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        vibrator?.vibrate(VibrationEffect.createWaveform(pattern, 0))
      } else {
        @Suppress("DEPRECATION")
        vibrator?.vibrate(pattern, 0)
      }
      findPhoneVibrator = vibrator
      Log.i(TAG, "Find Phone: Ringtone and vibration started")
    } catch (e: Exception) {
      Log.e(TAG, "Find Phone: Failed to play audio alert: ${e.message}")
    }
  }

  fun stopFindPhoneAlert() {
    if (!isFindPhoneActive && findPhoneRingtone == null && findPhoneVibrator == null) return
    isFindPhoneActive = false
    try {
      findPhoneRingtone?.stop()
      findPhoneRingtone = null
      findPhoneVibrator?.cancel()
      findPhoneVibrator = null
      Log.i(TAG, "Find Phone: Ringtone and vibration stopped")
    } catch (e: Exception) {
      Log.w(TAG, "Find Phone: Error stopping alert: ${e.message}")
    }
    emit("onWristEvent", mapOf("type" to "find-phone", "active" to false))
  }

  // ==================== Outgoing Commands ====================

  fun sendKnownPacket(data: ByteArray) {
    if (mode == WristMode.READ_ONLY) {
      fail("Cannot send BLE packets in read-only mode")
      return
    }
    if (!modelVerified) {
      fail("Cannot send BLE packets: watch model not verified")
      return
    }
    val connection = gatt
    val characteristic = writeCharacteristic
    if (!ready || connection == null || characteristic == null) {
      fail("Watch is not ready")
      return
    }
    val hex = data.joinToString("") { "%02X".format(it) }
    Log.d(TAG, "KaHa MKI: TX packet (${data.size} bytes): 0x$hex")
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
      if (!submitted) {
        Log.w(TAG, "KaHa MKI: Watch rejected BLE write submission")
        fail("Watch rejected BLE write submission")
      }
    } catch (_: SecurityException) {
      fail("Bluetooth permission revoked")
    }
  }

  val UART_SERVICE: UUID = UUID.fromString("6e400001-b5a3-f393-e0a9-e50e24dcca9e")
  val UART_RX: UUID = UUID.fromString("6e400002-b5a3-f393-e0a9-e50e24dcca9e")
  val UART_TX: UUID = UUID.fromString("6e400003-b5a3-f393-e0a9-e50e24dcca9e")
  val DEVICE_INFORMATION: UUID = UUID.fromString("0000180a-0000-1000-8000-00805f9b34fb")
  val MODEL_NUMBER: UUID = UUID.fromString("00002a24-0000-1000-8000-00805f9b34fb")
  val FIRMWARE_REVISION: UUID = UUID.fromString("00002a26-0000-1000-8000-00805f9b34fb")
  val HARDWARE_REVISION: UUID = UUID.fromString("00002a27-0000-1000-8000-00805f9b34fb")
  val BATTERY_SERVICE: UUID = UUID.fromString("0000180f-0000-1000-8000-00805f9b34fb")
  val BATTERY_LEVEL: UUID = UUID.fromString("00002a19-0000-1000-8000-00805f9b34fb")
  val CCC_DESCRIPTOR: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

  const val VERIFIED_MODEL_NAME = "WA49V3"
  const val CONNECTION_TIMEOUT_MS = 15_000L
  const val DISCOVERY_TIMEOUT_MS = 10_000L
  const val READ_TIMEOUT_MS = 5_000L
}
