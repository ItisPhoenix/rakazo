package com.rakazo.devlink

import java.io.ByteArrayOutputStream
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DevLinkProtocolTest {
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
            org.json.JSONArray()
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

  @Test
  fun `frames carry a four byte big endian length`() {
    val payload = "{}".toByteArray(Charsets.UTF_8)
    val framed = DevLinkProtocol.encodeFrame(payload)
    assertEquals(4 + payload.size, framed.size)
    assertEquals(0, framed[0].toInt())
    assertEquals(0, framed[1].toInt())
    assertEquals(0, framed[2].toInt())
    assertEquals(payload.size.toByte(), framed[3])
    assertEquals("{}", String(framed, 4, payload.size, Charsets.UTF_8))
  }

  @Test
  fun `a frame larger than the limit is refused`() {
    val payload = ByteArray(DevLinkProtocol.MAX_PAYLOAD_BYTES + 1)
    try {
      DevLinkProtocol.encodeFrame(payload)
      throw AssertionError("an oversized frame must be refused")
    } catch (error: DevLinkProtocol.FrameTooLarge) {
      assertEquals(DevLinkProtocol.MAX_PAYLOAD_BYTES + 1, error.declared)
    }
  }

  @Test
  fun `an empty frame is refused`() {
    try {
      DevLinkProtocol.encodeFrame(ByteArray(0))
      throw AssertionError("an empty frame must be refused")
    } catch (expected: java.io.IOException) {
      assertTrue(expected.message!!.contains("no payload"))
    }
  }

  @Test
  fun `a reader reassembles a frame that arrives in pieces`() {
    val payload = "{\"link\":1,\"kind\":\"bye\"}".toByteArray(Charsets.UTF_8)
    val framed = DevLinkProtocol.encodeFrame(payload)
    val reader = FrameReader()
    val input = ChunkedInput(framed, 1)
    val frame = reader.read(input)
    assertNotNull(frame)
    assertEquals("{\"link\":1,\"kind\":\"bye\"}", String(frame!!, Charsets.UTF_8))
  }

  @Test
  fun `a reader refuses a frame that declares more than the limit`() {
    val prefix = ByteArray(4)
    val declared = DevLinkProtocol.MAX_PAYLOAD_BYTES + 1
    prefix[0] = (declared ushr 24).toByte()
    prefix[1] = (declared ushr 16).toByte()
    prefix[2] = (declared ushr 8).toByte()
    prefix[3] = declared.toByte()
    try {
      FrameReader().read(ChunkedInput(prefix, 4))
      throw AssertionError("an oversized frame must be refused")
    } catch (error: DevLinkProtocol.FrameTooLarge) {
      assertEquals(declared, error.declared)
    }
  }

  @Test
  fun `the handshake carries the token and the link version`() {
    val message = DevLinkProtocol.handshake(7, "session-value")
    assertEquals(1, message.getInt("link"))
    assertEquals("handshake", message.getString("kind"))
    assertEquals(7, message.getLong("request_id"))
    assertEquals("session-value", message.getString("token"))
  }

  @Test
  fun `a customization request carries the base revision and a protocol envelope`() {
    val message = DevLinkProtocol.customization(9, 4, customization("classic"))
    assertEquals("message", message.getString("kind"))
    assertEquals(9, message.getLong("request_id"))
    assertEquals(4, message.getLong("revision"))
    val envelope = message.getJSONObject("envelope")
    assertEquals(3, envelope.getInt("version"))
    assertEquals(9, envelope.getLong("id"))
    val inner = envelope.getJSONObject("message")
    assertEquals("customization", inner.getString("type"))
    assertEquals("classic", inner.getJSONObject("data").getJSONObject("appearance").getString("face"))
  }

  @Test
  fun `a change is applied on top of the known customization`() {
    val current = customization("hybrid")
    val merged = DevLinkProtocol.mergeCustomization(current, JSONObject().put("appearance", JSONObject().put("face", "dashboard")))
    assertEquals("dashboard", merged.getJSONObject("appearance").getString("face"))
    assertEquals("cyan_violet", merged.getJSONObject("appearance").getString("palette"))
    assertEquals(60, merged.getJSONObject("appearance").getInt("glow"))
    assertEquals(3, merged.getJSONObject("appearance").getJSONArray("complications").length())
    assertTrue(merged.getBoolean("use_24h"))
  }

  @Test
  fun `a change never touches the part it does not mention`() {
    val current = customization("hybrid")
    val merged = DevLinkProtocol.mergeCustomization(current, JSONObject().put("use_24h", false))
    assertTrue(!merged.getBoolean("use_24h"))
    assertEquals("hybrid", merged.getJSONObject("appearance").getString("face"))
    assertEquals(3, merged.getJSONObject("appearance").getJSONArray("complications").length())
  }

  @Test
  fun `fields the simulator would refuse are dropped`() {
    val current = customization("hybrid")
    val merged =
      DevLinkProtocol.mergeCustomization(
        current,
        JSONObject()
          .put("appearance", JSONObject().put("face", "dashboard").put("unexpected", 1))
          .put("also_unexpected", true),
      )
    assertEquals("dashboard", merged.getJSONObject("appearance").getString("face"))
    assertTrue(!merged.has("also_unexpected"))
    assertTrue(!merged.getJSONObject("appearance").has("unexpected"))
  }

  @Test
  fun `decoding refuses a message from another link version`() {
    try {
      DevLinkProtocol.decode("{\"link\":9,\"kind\":\"bye\"}".toByteArray(Charsets.UTF_8))
      throw AssertionError("another link version must be refused")
    } catch (expected: java.io.IOException) {
      assertTrue(expected.message!!.contains("link version"))
    }
  }

  @Test
  fun `decoding reads the customization out of an envelope`() {
    val message = DevLinkProtocol.decode(DevLinkProtocol.customization(3, 1, customization()).toString().toByteArray(Charsets.UTF_8))
    val found = DevLinkProtocol.customizationOf(message)
    assertNotNull(found)
    assertEquals("hybrid", found!!.getJSONObject("appearance").getString("face"))
    assertNull(DevLinkProtocol.customizationOf(JSONObject().put("kind", "ack")))
  }

  /** An input stream that hands out at most `chunk` bytes per read. */
  private class ChunkedInput(private val data: ByteArray, private val chunk: Int) :
    java.io.InputStream() {
    private var position = 0

    override fun read(): Int {
      if (position >= data.size) return -1
      val value = data[position].toInt() and 0xff
      position += 1
      return value
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
      if (position >= data.size) return -1
      val count = minOf(length, chunk, data.size - position)
      System.arraycopy(data, position, buffer, offset, count)
      position += count
      return count
    }
  }

  @Test
  fun `a reader reports an idle stream`() {
    val reader = FrameReader()
    val input =
      object : java.io.InputStream() {
        override fun read(): Int = throw java.net.SocketTimeoutException()

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int =
          throw java.net.SocketTimeoutException()
      }
    assertNull(reader.read(input))
  }

  @Test
  fun `the payload of a frame survives a round trip`() {
    val payload = ByteArrayOutputStream()
    payload.write("{\"link\":1}".toByteArray(Charsets.UTF_8))
    val framed = DevLinkProtocol.encodeFrame(payload.toByteArray())
    val reader = FrameReader()
    val frame = reader.read(ChunkedInput(framed, 3))
    assertNotNull(frame)
    assertEquals("{\"link\":1}", String(frame!!, Charsets.UTF_8))
  }
}
