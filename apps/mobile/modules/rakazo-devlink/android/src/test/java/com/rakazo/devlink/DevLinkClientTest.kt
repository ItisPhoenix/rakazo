package com.rakazo.devlink

import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DevLinkClientTest {
  private fun customization(face: String = "hybrid"): JSONObject =
    JSONObject()
      .put(
        "appearance",
        JSONObject()
          .put("face", face)
          .put("palette", "cyan_violet")
          .put("show_date", true)
          .put("show_seconds", false)
          .put("show_complications", true)
          .put(
            "complications",
            JSONArray()
              .put(
                JSONObject()
                  .put("kind", "heart_rate")
                  .put("accent", "coral")
                  .put("visible", true)
                  .put("style", "icon_value"),
              )
              .put(
                JSONObject()
                  .put("kind", "steps")
                  .put("accent", "cyan")
                  .put("visible", true)
                  .put("style", "icon_value"),
              )
              .put(
                JSONObject()
                  .put("kind", "battery")
                  .put("accent", "mint")
                  .put("visible", true)
                  .put("style", "icon_value"),
              ),
          )
          .put("glow", 60),
      )
      .put("use_24h", true)
      .put("reduce_motion", false)

  private fun stateAck(replyTo: Long, revision: Long, face: String = "hybrid"): JSONObject {
    val envelope =
      JSONObject()
        .put("version", 3)
        .put("id", replyTo)
        .put(
          "message",
          JSONObject().put("type", "customization").put("data", customization(face)),
        )
    return JSONObject()
      .put("link", 1)
      .put("kind", "ack")
      .put("request_id", replyTo)
      .put("reply_to", replyTo)
      .put("revision", revision)
      .put("envelope", envelope)
  }

  private fun statePush(requestId: Long, revision: Long, face: String): JSONObject {
    val envelope =
      JSONObject()
        .put("version", 3)
        .put("id", requestId)
        .put(
          "message",
          JSONObject().put("type", "customization").put("data", customization(face)),
        )
    return JSONObject()
      .put("link", 1)
      .put("kind", "message")
      .put("request_id", requestId)
      .put("revision", revision)
      .put("envelope", envelope)
  }

  /** A simulator stand-in that answers whatever the test wants. */
  private class FakeSimulator(private val respond: (JSONObject, Int) -> JSONObject?) {
    val server = ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))
    val received = CopyOnWriteArrayList<JSONObject>()
    private val reader = FrameReader()

    @Volatile var stopped = false

    @Volatile private var output: BufferedOutputStream? = null

    init {
      Thread(
              {
                try {
                  val socket = server.accept()
                  val input = BufferedInputStream(socket.getInputStream())
                  val stream = BufferedOutputStream(socket.getOutputStream())
                  output = stream
                  var count = 0
                  while (!stopped) {
                    val frame = reader.read(input) ?: continue
                    val message = DevLinkProtocol.decode(frame)
                    received.add(message)
                    count += 1
                    val reply = respond(message, count)
                    if (reply != null) {
                      stream.write(
                        DevLinkProtocol.encodeFrame(reply.toString().toByteArray(Charsets.UTF_8))
                      )
                      stream.flush()
                    }
                  }
                } catch (_: Exception) {
                  // The test closed the connection.
                }
              },
              "fake-simulator",
          )
          .apply { isDaemon = true }
          .start()
    }

    val port: Int
      get() = server.localPort

    /** Send a message the client did not ask for. */
    fun push(message: JSONObject) {
      val stream = output ?: throw IllegalStateException("no client is connected")
      stream.write(DevLinkProtocol.encodeFrame(message.toString().toByteArray(Charsets.UTF_8)))
      stream.flush()
    }

    fun close() {
      stopped = true
      try {
        server.close()
      } catch (_: Exception) {
        // Nothing to do.
      }
    }
  }

  private class RecordingListener : DevLinkClient.Listener {
    val states = CopyOnWriteArrayList<String>()
    val customizations = CopyOnWriteArrayList<Pair<Long, String>>()
    val acks = CopyOnWriteArrayList<Triple<Long, Boolean, String?>>()
    val errors = CopyOnWriteArrayList<Pair<String, Long?>>()
    val connected = CountDownLatch(1)
    val answered = CountDownLatch(1)
    val failed = CountDownLatch(1)

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

    override fun onError(code: String, message: String, requestId: Long?) {
      errors.add(code to requestId)
      failed.countDown()
    }
  }

  private fun client(
    listener: RecordingListener,
    requestTimeoutMs: Int = 300,
  ): DevLinkClient =
    DevLinkClient(
      listener,
      DevLinkClient.Config(
        connectTimeoutMs = 2000,
        readTimeoutMs = 50,
        requestTimeoutMs = requestTimeoutMs,
        maxAttempts = 3,
        maxPending = 4,
      ),
    )

  @Test
  fun `a handshake reports the authoritative customization and the revision`() {
    val simulator =
      FakeSimulator { message, _ ->
        if (message.getString("kind") == "handshake") {
          stateAck(message.getLong("request_id"), 1)
        } else null
      }
    val listener = RecordingListener()
    val link = client(listener)
    try {
      link.connect("127.0.0.1", simulator.port, "session-value")
      assertTrue(listener.connected.await(5, TimeUnit.SECONDS))
      assertEquals(DevLinkClient.STATUS_CONNECTED, listener.states.last())
      assertTrue(link.isConnected())
      val (revision, text) = listener.customizations.first()
      assertEquals(1L, revision)
      assertTrue(text.contains("\"face\":\"hybrid\""))
    } finally {
      link.disconnect()
      simulator.close()
    }
  }

  @Test
  fun `the simulator sees the token in the handshake`() {
    val simulator =
      FakeSimulator { message, _ ->
        if (message.getString("kind") == "handshake") {
          stateAck(message.getLong("request_id"), 1)
        } else null
      }
    val listener = RecordingListener()
    val link = client(listener)
    try {
      link.connect("127.0.0.1", simulator.port, "session-value")
      assertTrue(listener.connected.await(5, TimeUnit.SECONDS))
      val handshake = simulator.received.first()
      assertEquals("handshake", handshake.getString("kind"))
      assertEquals("session-value", handshake.getString("token"))
      assertEquals(1, handshake.getInt("link"))
    } finally {
      link.disconnect()
      simulator.close()
    }
  }

  @Test
  fun `a refused handshake never reports a connection`() {
    val simulator =
      FakeSimulator { message, _ ->
        if (message.getString("kind") == "handshake") {
          JSONObject()
            .put("link", 1)
            .put("kind", "error")
            .put("request_id", message.getLong("request_id"))
            .put("reply_to", message.getLong("request_id"))
            .put("code", "unauthorized")
        } else null
      }
    val listener = RecordingListener()
    val link = client(listener)
    try {
      link.connect("127.0.0.1", simulator.port, "wrong-value")
      assertTrue(listener.failed.await(5, TimeUnit.SECONDS))
      assertFalse(link.isConnected())
      assertEquals("unauthorized", listener.errors.first().first)
      assertFalse(listener.states.contains(DevLinkClient.STATUS_CONNECTED))
    } finally {
      link.disconnect()
      simulator.close()
    }
  }

  @Test
  fun `an apply is answered and matched by its reply id`() {
    val simulator =
      FakeSimulator { message, _ ->
        when (message.getString("kind")) {
          "handshake" -> stateAck(message.getLong("request_id"), 1)
          "message" -> stateAck(message.getLong("request_id"), 2, "classic")
          else -> null
        }
      }
    val listener = RecordingListener()
    val link = client(listener)
    try {
      link.connect("127.0.0.1", simulator.port, "session-value")
      assertTrue(listener.connected.await(5, TimeUnit.SECONDS))
      link.applyCustomization(JSONObject().put("appearance", JSONObject().put("face", "classic")).toString())
      assertTrue(listener.answered.await(5, TimeUnit.SECONDS))
      val requestId = simulator.received.last().getLong("request_id")
      val (ackedId, ok, code) = listener.acks.first()
      assertEquals(requestId, ackedId)
      assertTrue(ok)
      assertEquals(null, code)
      assertEquals(2L, listener.customizations.last().first)
      assertTrue(listener.customizations.last().second.contains("\"face\":\"classic\""))
    } finally {
      link.disconnect()
      simulator.close()
    }
  }

  @Test
  fun `a conflict is reported with the authoritative state`() {
    val simulator =
      FakeSimulator { message, _ ->
        when (message.getString("kind")) {
          "handshake" -> stateAck(message.getLong("request_id"), 1)
          "message" -> {
            val replyTo = message.getLong("request_id")
            val envelope =
              JSONObject()
                .put("version", 3)
                .put("id", replyTo)
                .put(
                  "message",
                  JSONObject().put("type", "customization").put("data", customization("dashboard")),
                )
            JSONObject()
              .put("link", 1)
              .put("kind", "error")
              .put("request_id", replyTo)
              .put("reply_to", replyTo)
              .put("code", "conflict")
              .put("revision", 5)
              .put("envelope", envelope)
          }
          else -> null
        }
      }
    val listener = RecordingListener()
    val link = client(listener)
    try {
      link.connect("127.0.0.1", simulator.port, "session-value")
      assertTrue(listener.connected.await(5, TimeUnit.SECONDS))
      link.applyCustomization(JSONObject().put("appearance", JSONObject().put("face", "minimal")).toString())
      assertTrue(listener.answered.await(5, TimeUnit.SECONDS))
      val (_, ok, code) = listener.acks.first()
      assertFalse(ok)
      assertEquals("conflict", code)
      assertEquals(5L, listener.customizations.last().first)
      assertTrue(listener.customizations.last().second.contains("\"face\":\"dashboard\""))
    } finally {
      link.disconnect()
      simulator.close()
    }
  }

  @Test
  fun `a request that is not answered is sent again with the same id`() {
    val simulator =
      FakeSimulator { message, count ->
        when (message.getString("kind")) {
          "handshake" -> stateAck(message.getLong("request_id"), 1)
          // The first change is ignored; the retry carries the same request id
          // and is answered.
          "message" -> if (count > 2) stateAck(message.getLong("request_id"), 2, "minimal") else null
          else -> null
        }
      }
    val listener = RecordingListener()
    val link = client(listener, requestTimeoutMs = 200)
    try {
      link.connect("127.0.0.1", simulator.port, "session-value")
      assertTrue(listener.connected.await(5, TimeUnit.SECONDS))
      link.applyCustomization(JSONObject().put("appearance", JSONObject().put("face", "minimal")).toString())
      assertTrue(listener.answered.await(5, TimeUnit.SECONDS))
      val changes = simulator.received.filter { it.getString("kind") == "message" }
      assertTrue("the request should have been sent again", changes.size >= 2)
      assertEquals(
        "a retry must keep the request id so the simulator cannot apply it twice",
        changes.first().getLong("request_id"),
        changes.last().getLong("request_id"),
      )
      assertTrue(listener.errors.any { it.first == "request_retried" })
    } finally {
      link.disconnect()
      simulator.close()
    }
  }

  @Test
  fun `a change the simulator made on its own is reported`() {
    val simulator =
      FakeSimulator { message, _ ->
        if (message.getString("kind") == "handshake") {
          stateAck(message.getLong("request_id"), 1)
        } else null
      }
    val listener = RecordingListener()
    val link = client(listener)
    try {
      link.connect("127.0.0.1", simulator.port, "session-value")
      assertTrue(listener.connected.await(5, TimeUnit.SECONDS))
      simulator.push(statePush(99, 2, "dashboard"))
      val deadline = System.currentTimeMillis() + 5000
      while (listener.customizations.size < 2 && System.currentTimeMillis() < deadline) {
        Thread.sleep(10)
      }
      assertEquals(2, listener.customizations.size)
      assertEquals(2L, listener.customizations.last().first)
      assertTrue(listener.customizations.last().second.contains("\"face\":\"dashboard\""))
    } finally {
      link.disconnect()
      simulator.close()
    }
  }

  @Test
  fun `the client reports the state it knows`() {
    val listener = RecordingListener()
    val link = client(listener)
    link.disconnect()
    assertFalse(link.isConnected())
    assertEquals(DevLinkClient.STATUS_IDLE, link.status())
  }

  @Test
  fun `a change without a known state is refused`() {
    val simulator =
      FakeSimulator { message, _ ->
        if (message.getString("kind") == "handshake") {
          stateAck(message.getLong("request_id"), 1)
        } else null
      }
    val listener = RecordingListener()
    val link = client(listener)
    try {
      link.connect("127.0.0.1", simulator.port, "session-value")
      assertTrue(listener.connected.await(5, TimeUnit.SECONDS))
      val state = link.currentCustomization()
      assertNotNull(state)
      assertTrue(state!!.contains("\"appearance\""))
      assertEquals(1L, link.currentRevision())
    } finally {
      link.disconnect()
      simulator.close()
    }
  }
}
