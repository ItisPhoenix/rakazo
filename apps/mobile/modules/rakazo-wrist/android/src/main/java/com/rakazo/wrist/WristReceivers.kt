package com.rakazo.wrist

import android.bluetooth.BluetoothAdapter
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Receiver to restore persistent watch connection upon device boot or app update.
 */
class WristBootReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent?) {
    val action = intent?.action ?: return
    if (action == Intent.ACTION_BOOT_COMPLETED || action == Intent.ACTION_MY_PACKAGE_REPLACED) {
      Log.i("RakazoWrist", "BootReceiver: system boot/update completed ($action)")
      if (WristBridge.isPersistentConfigured(context)) {
        Log.i("RakazoWrist", "BootReceiver: persistent watch connection enabled; starting WristService")
        WristService.start(context)
      }
    }
  }
}

/**
 * Receiver to trigger reconnection when Bluetooth adapter is re-enabled.
 */
class WristBluetoothReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent?) {
    if (intent?.action == BluetoothAdapter.ACTION_STATE_CHANGED) {
      val state = intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, BluetoothAdapter.ERROR)
      if (state == BluetoothAdapter.STATE_ON) {
        Log.i("RakazoWrist", "BluetoothReceiver: Bluetooth turned ON; evaluating reconnection")
        WristBridge.onBluetoothTurnedOn(context)
      } else if (state == BluetoothAdapter.STATE_TURNING_OFF || state == BluetoothAdapter.STATE_OFF) {
        Log.i("RakazoWrist", "BluetoothReceiver: Bluetooth turned OFF")
        WristBridge.onBluetoothTurnedOff()
      }
    }
  }
}
