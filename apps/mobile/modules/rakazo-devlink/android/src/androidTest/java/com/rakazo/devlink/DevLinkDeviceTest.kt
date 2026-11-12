package com.rakazo.devlink

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The Android client against the running Rust simulator.
 *
 * The test runs on a real device or emulator. The simulator is reached through
 * the loopback interface, which `adb reverse tcp:<port> tcp:<port>` forwards to
 * the Windows host.
 *
 * The port and the token are given as instrumentation arguments:
 *
 * ```
 * adb reverse tcp:4599 tcp:4599
 * adb shell am instrument -w -e port 4599 -e token <session-token> \
 *   com.rakazo.devlink.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class DevLinkDeviceTest {
  private class Listener : DevLinkClient.Listener {
    val states = CopyOnWriteArrayList<String>()
    val customizations = CopyOnWriteArrayList<Pair<Long, String>>()
    val acks = CopyOnWriteArrayList<Triple<Long, Boolean, String?>>()
    val connected = CountDownLatch(1)
    val answered = CountDownLatch(1)

    override fun onState(status: String, reason: String?) {
      states.add(status)
      if (status == DevLinkClient.STATUS_CONNECTED) connected.countDown()
    }

    override fun onCustomization(revision: Long, customizationJson: String) {
      customizations.add(revision to customizationJson)
    }

    override fun onAck(requestId: Long, ok: Boolean, revision: Long?, code: String?) {
      acks.add(Triple(requestId, ok, code))
      answered.countDown()
    }

    override fun onError(code: String, message: String, requestId: Long?) {}
  }

  @Test
  fun theAndroidClientReadsAndChangesTheSimulator() {
    val arguments = InstrumentationRegistry.getArguments()
    val port = arguments.getString("port")?.toIntOrNull()
    val token = arguments.getString("token")
    assumeTrue("no development link port was given", port != null)
    assumeTrue("no session token was given", !token.isNullOrEmpty())

    val listener = Listener()
    val link = DevLinkClient(listener, DevLinkClient.Config(requestTimeoutMs = 5000))
    try {
      link.connect("127.0.0.1", port!!, token!!)
      assertTrue("the simulator did not answer the handshake", listener.connected.await(15, TimeUnit.SECONDS))
      assertTrue(link.isConnected())
      assertTrue(listener.customizations.isNotEmpty())
      val (revision, text) = listener.customizations.first()
      assertTrue(text.contains("\"appearance\""))
      assertTrue(revision >= 1)

      // Change the face on the simulator and wait for the acknowledgement.
      link.applyCustomization(
        JSONObject()
          .put("appearance", JSONObject().put("face", "minimal"))
          .toString()
      )
      assertTrue("the simulator did not answer the change", listener.answered.await(15, TimeUnit.SECONDS))
      val (_, ok, code) = listener.acks.first()
      assertTrue("the change was refused: $code", ok)
    } finally {
      link.disconnect()
    }
  }
}
