package com.rakazo.devlink

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.net.SocketTimeoutException
import org.json.JSONObject

/**
 * The Charlie WatchOS development link, version 1.
 *
 * The wire format is the one the Rust simulator speaks and nothing else:
 *
 * ```
 * +---------------------+---------------------------+
 * | u32 length (BE)     | JSON link message         |
 * +---------------------+---------------------------+
 * ```
 *
 * The whole frame, length prefix included, may not exceed
 * [MAX_FRAME_BYTES]. The declared length is checked before a payload buffer is
 * allocated, exactly as the simulator requires.
 *
 * The link message shape mirrors the simulator's `LinkMessage`:
 *
 * ```json
 * {"link":1,"kind":"handshake","request_id":1,"token":"…"}
 * {"link":1,"kind":"message","request_id":7,"revision":4,"envelope":{"version":3,"id":7,"message":{"type":"customization","data":{…}}}}
 * ```
 *
 * Unknown fields are rejected by the simulator, so this class only ever writes
 * the fields the simulator expects.
 */
object DevLinkProtocol {
  /** Version carried by every link message. */
  const val LINK_VERSION = 1

  /** Bytes used by the frame length prefix. */
  const val FRAME_PREFIX_BYTES = 4

  /** Maximum bytes a single frame may occupy on the wire. */
  const val MAX_FRAME_BYTES = 4096

  /** Maximum JSON payload bytes that fit inside one frame. */
  const val MAX_PAYLOAD_BYTES = MAX_FRAME_BYTES - FRAME_PREFIX_BYTES

  /** Protocol version of the envelope that carries a customization. */
  const val ENVELOPE_VERSION = 3

  const val KIND_HANDSHAKE = "handshake"
  const val KIND_HELLO = "hello"
  const val KIND_MESSAGE = "message"
  const val KIND_ACK = "ack"
  const val KIND_ERROR = "error"
  const val KIND_BYE = "bye"

  const val MESSAGE_CUSTOMIZATION = "customization"

  /** A frame declared more bytes than the simulator accepts. */
  class FrameTooLarge(val declared: Int) :
    IOException("frame of $declared bytes exceeds the $MAX_FRAME_BYTES byte limit")

  /** Build the four-byte big-endian length prefix and the payload. */
  fun encodeFrame(payload: ByteArray): ByteArray {
    if (payload.isEmpty()) throw IOException("frame carried no payload")
    if (payload.size > MAX_PAYLOAD_BYTES) throw FrameTooLarge(payload.size)
    val framed = ByteArray(FRAME_PREFIX_BYTES + payload.size)
    val length = payload.size
    framed[0] = (length ushr 24).toByte()
    framed[1] = (length ushr 16).toByte()
    framed[2] = (length ushr 8).toByte()
    framed[3] = length.toByte()
    System.arraycopy(payload, 0, framed, FRAME_PREFIX_BYTES, payload.size)
    return framed
  }

  /** A handshake that presents the session token. */
  fun handshake(requestId: Long, token: String): JSONObject =
    JSONObject()
      .put("link", LINK_VERSION)
      .put("kind", KIND_HANDSHAKE)
      .put("request_id", requestId)
      .put("token", token)

  /** A request for the current state. */
  fun hello(requestId: Long): JSONObject =
    JSONObject()
      .put("link", LINK_VERSION)
      .put("kind", KIND_HELLO)
      .put("request_id", requestId)

  /** A customization request that is based on a known revision. */
  fun customization(requestId: Long, revision: Long, customization: JSONObject): JSONObject {
    val envelope =
      JSONObject()
        .put("version", ENVELOPE_VERSION)
        .put("id", requestId)
        .put(
          "message",
          JSONObject().put("type", MESSAGE_CUSTOMIZATION).put("data", customization),
        )
    return JSONObject()
      .put("link", LINK_VERSION)
      .put("kind", KIND_MESSAGE)
      .put("request_id", requestId)
      .put("revision", revision)
      .put("envelope", envelope)
  }

  /** Parse a link message and check the fields the simulator insists on. */
  fun decode(payload: ByteArray): JSONObject {
    if (payload.size > MAX_PAYLOAD_BYTES) throw FrameTooLarge(payload.size)
    val message = JSONObject(String(payload, Charsets.UTF_8))
    if (message.optInt("link", -1) != LINK_VERSION) {
      throw IOException("unsupported link version ${message.optInt("link", -1)}")
    }
    if (!message.has("kind")) throw IOException("link message without a kind")
    return message
  }

  /** Read the customization carried by a message, if it carries one. */
  fun customizationOf(message: JSONObject): JSONObject? {
    val envelope = message.optJSONObject("envelope") ?: return null
    val inner = envelope.optJSONObject("message") ?: return null
    if (inner.optString("type") != MESSAGE_CUSTOMIZATION) return null
    return inner.optJSONObject("data") ?: return null
  }

  /**
   * Merge a change into the known customization.
   *
   * The simulator rejects a customization that does not describe the whole
   * appearance, so a change is applied on top of the last state the simulator
   * reported. Unknown fields are dropped: the simulator refuses them.
   */
  fun mergeCustomization(current: JSONObject?, patch: JSONObject): JSONObject {
    val base = current?.let { JSONObject(it.toString()) } ?: JSONObject()
    for (key in patch.keys()) {
      if (!ALLOWED_CUSTOMIZATION_KEYS.contains(key)) continue
      val value = patch.get(key)
      if (value is JSONObject && base.optJSONObject(key) != null) {
        base.put(key, mergeAppearance(base.optJSONObject(key)!!, value))
      } else {
        base.put(key, value)
      }
    }
    return base
  }

  private fun mergeAppearance(current: JSONObject, patch: JSONObject): JSONObject {
    val merged = JSONObject(current.toString())
    for (key in patch.keys()) {
      if (!ALLOWED_APPEARANCE_KEYS.contains(key)) continue
      val value = patch.get(key)
      if (value is JSONObject && merged.optJSONObject(key) != null) {
        merged.put(key, mergeSlots(merged.optJSONObject(key)!!, value))
      } else {
        merged.put(key, value)
      }
    }
    return merged
  }

  private fun mergeSlots(current: JSONObject, patch: JSONObject): JSONObject {
    val merged = JSONObject(current.toString())
    for (key in patch.keys()) {
      if (!ALLOWED_SLOT_KEYS.contains(key)) continue
      merged.put(key, patch.get(key))
    }
    return merged
  }

  private val ALLOWED_CUSTOMIZATION_KEYS = setOf("appearance", "use_24h", "reduce_motion")
  private val ALLOWED_APPEARANCE_KEYS =
    setOf(
      "face",
      "palette",
      "show_date",
      "show_seconds",
      "show_complications",
      "complications",
      "glow",
    )
  private val ALLOWED_SLOT_KEYS = setOf("kind", "accent", "visible", "style")
}

/**
 * Reads length-prefixed frames from a stream that may deliver data in pieces.
 *
 * A read timeout on an idle stream reports `null` so the caller can keep
 * working. A timeout in the middle of a frame means the frame was truncated
 * and the session has to end.
 */
class FrameReader {
  private var buffer = ByteArray(0)
  private var expected = -1

  /** Read the next frame, or `null` when the stream was idle. */
  fun read(input: InputStream): ByteArray? {
    while (true) {
      if (expected < 0) {
        if (buffer.size < DevLinkProtocol.FRAME_PREFIX_BYTES) {
          if (!fill(input)) return null
          continue
        }
        val declared =
          ((buffer[0].toInt() and 0xff) shl 24) or
            ((buffer[1].toInt() and 0xff) shl 16) or
            ((buffer[2].toInt() and 0xff) shl 8) or
            (buffer[3].toInt() and 0xff)
        if (declared == 0) throw IOException("frame carried no payload")
        if (declared > DevLinkProtocol.MAX_PAYLOAD_BYTES) {
          throw DevLinkProtocol.FrameTooLarge(declared)
        }
        expected = declared
      }
      val total = DevLinkProtocol.FRAME_PREFIX_BYTES + expected
      if (buffer.size < total) {
        if (!fill(input)) throw IOException("the frame was truncated")
        continue
      }
      val payload = buffer.copyOfRange(DevLinkProtocol.FRAME_PREFIX_BYTES, total)
      buffer = buffer.copyOfRange(total, buffer.size)
      expected = -1
      return payload
    }
  }

  /** Read more bytes. Returns false when the stream was idle. */
  private fun fill(input: InputStream): Boolean {
    val chunk = ByteArray(512)
    val read =
      try {
        input.read(chunk)
      } catch (timeout: SocketTimeoutException) {
        if (buffer.isEmpty()) return false
        throw IOException("the frame was truncated")
      }
    if (read <= 0) throw IOException("the simulator closed the connection")
    if (buffer.size + read > DevLinkProtocol.FRAME_PREFIX_BYTES + DevLinkProtocol.MAX_PAYLOAD_BYTES) {
      throw DevLinkProtocol.FrameTooLarge(buffer.size + read)
    }
    val merged = ByteArrayOutputStream(buffer.size + read)
    merged.write(buffer, 0, buffer.size)
    merged.write(chunk, 0, read)
    buffer = merged.toByteArray()
    return true
  }
}
