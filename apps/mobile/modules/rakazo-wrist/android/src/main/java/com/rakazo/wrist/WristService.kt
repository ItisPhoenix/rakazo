package com.rakazo.wrist

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log

/**
 * Foreground Service for Charlie Wrist.
 *
 * Keeps the BLE connection alive when the app is in the background or minimized,
 * in compliance with Android 14+ (API 34+) FOREGROUND_SERVICE_CONNECTED_DEVICE restrictions.
 */
class WristService : Service() {

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() {
    super.onCreate()
    createNotificationChannel()
    val notification = buildNotification(WristBridge.getForegroundNotificationText())
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
      } else {
        ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
      }
      startForeground(NOTIFICATION_ID, notification, type)
    } else {
      startForeground(NOTIFICATION_ID, notification)
    }
    WristBridge.registerServiceInstance(this)
    Log.i(TAG, "WristService foreground service created")
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val action = intent?.action
    if (action == ACTION_STOP) {
      stopSelf()
      return START_NOT_STICKY
    }

    val text = WristBridge.getForegroundNotificationText()
    updateNotification(text)

    // Ensure connection engine is active if persistent mode is enabled
    WristBridge.onServiceStarted(this)
    return START_STICKY
  }

  override fun onDestroy() {
    WristBridge.unregisterServiceInstance(this)
    Log.i(TAG, "WristService destroyed")
    super.onDestroy()
  }

  fun updateNotification(contentText: String) {
    try {
      val notification = buildNotification(contentText)
      val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
      manager.notify(NOTIFICATION_ID, notification)
    } catch (e: Exception) {
      Log.w(TAG, "Could not update foreground notification: ${e.message}")
    }
  }

  private fun createNotificationChannel() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
      val existing = manager.getNotificationChannel(CHANNEL_ID)
      if (existing == null) {
        val channel = NotificationChannel(
          CHANNEL_ID,
          "Charlie Wrist Watch Link",
          NotificationManager.IMPORTANCE_LOW,
        ).apply {
          description = "Maintains persistent Bluetooth connection to your Lunar Discovery watch"
          setShowBadge(false)
        }
        manager.createNotificationChannel(channel)
      }
    }
  }

  private fun buildNotification(contentText: String): Notification {
    val launchIntent = packageManager.getLaunchIntentForPackage(packageName)?.apply {
      flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
    }
    val pendingIntent = if (launchIntent != null) {
      val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
      } else {
        PendingIntent.FLAG_UPDATE_CURRENT
      }
      PendingIntent.getActivity(this, 0, launchIntent, flags)
    } else null

    var iconRes = resources.getIdentifier("notification_icon", "drawable", packageName)
    if (iconRes == 0) {
      iconRes = android.R.drawable.stat_sys_data_bluetooth
    }

    val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      Notification.Builder(this, CHANNEL_ID)
    } else {
      @Suppress("DEPRECATION")
      Notification.Builder(this)
    }

    builder.setContentTitle("Charlie Wrist")
      .setContentText(contentText)
      .setSmallIcon(iconRes)
      .setOngoing(true)
      .setAutoCancel(false)

    if (pendingIntent != null) {
      builder.setContentIntent(pendingIntent)
    }

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
      builder.setCategory(Notification.CATEGORY_SERVICE)
    }

    return builder.build()
  }

  companion object {
    private const val TAG = "RakazoWrist"
    const val CHANNEL_ID = "charlie_wrist_link"
    const val NOTIFICATION_ID = 14901
    const val ACTION_STOP = "com.rakazo.wrist.ACTION_STOP"

    fun start(context: Context) {
      try {
        val intent = Intent(context, WristService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
          context.startForegroundService(intent)
        } else {
          context.startService(intent)
        }
      } catch (e: Exception) {
        Log.e(TAG, "Failed to start WristService: ${e.message}")
      }
    }

    fun stop(context: Context) {
      try {
        val intent = Intent(context, WristService::class.java)
        context.stopService(intent)
      } catch (e: Exception) {
        Log.e(TAG, "Failed to stop WristService: ${e.message}")
      }
    }
  }
}
