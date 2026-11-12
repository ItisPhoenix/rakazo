package com.rakazo.devlink

import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import org.json.JSONObject

/**
 * The Android side of the Charlie WatchOS development link.
 *
 * The client speaks the simulator's link version 1 over a plain TCP socket on
 * the loopback interface. On a device or emulator the socket reaches the
 * Windows host through `adb reverse tcp:<port> tcp:<port>`.
 *
 * Everything here runs on one worker thread: the Android UI thread never
 * touches the socket. The session token lives in memory for the length of the
 * session and is never written to disk or to a log.
 *
 * The class knows nothing about Expo, so it can be exercised on its own.
 */
class DevLinkClient(private val listener: Listener, private val config: Config = Config()) {
  /** Deadlines and limits of one client. */
  class Config(
    val connectTimeoutMs: Int = CONNECT_TIMEOUT_MS,
    val readTimeoutMs: Int = READ_TIMEOUT_MS,
    val requestTimeoutMs: Int = REQUEST_TIMEOUT_MS,
    val maxAttempts: Int = MAX_ATTEMPTS,
    val maxPending: Int = MAX_PENDING,
  )
  /** Callbacks the client reports. They run on the worker thread. */
  interface Listener {
    /** The link state changed. */
    fun onState(status: String, reason: String?)

    /** The simulator reported the committed customization and its revision. */
    fun onCustomization(revision: Long, customizationJson: String)

    /** A request was answered. */
    fun onAck(requestId: Long, ok: Boolean, revision: Long?, code: String?)

    /** Something went wrong. */
    fun onError(code: String, message: String, requestId: Long?)
  }

  companion object {
    const val STATUS_IDLE = "idle"
    const val STATUS_CONNECTING = "connecting"
    const val STATUS_AUTHENTICATING = "authenticating"
    const val STATUS_CONNECTED = "connected"
    const val STATUS_DISCONNECTED = "disconnected"
    const val STATUS_ERROR = "error"

    /** The loopback address the ADB reverse tunnel forwards to the host. */
    const val DEFAULT_HOST = "127.0.0.1"

    /** Default port of the development link on the host. */
    const val DEFAULT_PORT = 4599

    /** How long the socket may take to connect. */
    const val CONNECT_TIMEOUT_MS = 4000

    /** How long a single read may block before the client checks its work. */
    const val READ_TIMEOUT_MS = 200

    /** How long the simulator may take to answer a request. */
    const val REQUEST_TIMEOUT_MS = 4000

    /** How often a request is sent again before it is given up on. */
    const val MAX_ATTEMPTS = 3

    /** How many requests may wait for an answer at the same time. */
    const val MAX_PENDING = 8
  }

  private sealed class Command {
    class Connect(val host: String, val port: Int, val token: String) : Command()

    class Disconnect : Command()

    class Apply(val patchJson: String) : Command()

    class Sync : Command()
  }

  private class Pending(
    val requestId: Long,
    val frame: ByteArray,
    val kind: String,
    var attempts: Int,
    var deadline: Long,
  )

  private class Session {
    var socket: Socket? = null
    var input: InputStream? = null
    var output: OutputStream? = null
    val reader = FrameReader()
    var token: String? = null
    var handshakeRequestId = 0L
    var nextRequestId = 0L
    val pending = LinkedHashMap<Long, Pending>()
    var revision: Long? = null
    var customizationJson: String? = null
    var authenticated = false

    fun close() {
      authenticated = false
      token = null
      pending.clear()
      try {
        socket?.close()
      } catch (_: IOException) {
        // Closing a broken socket is not an error worth reporting.
      }
      socket = null
      input = null
      output = null
    }
  }

  private val commands = LinkedBlockingQueue<Command>()
  private val running = AtomicBoolean(false)
  private val requestCounter = AtomicLong(0)
  private var worker: Thread? = null

  /** True while the simulator answered the handshake. */
  @Volatile private var connected = false

  /** The state the link is in right now. */
  @Volatile private var currentStatus: String = STATUS_IDLE

  /** The state the link is in right now. */
  fun status(): String = currentStatus

  /** Report a state change and remember it. */
  private fun emitState(status: String, reason: String?) {
    currentStatus = status
    listener.onState(status, reason)
  }

  /** Start the client. Calling it again replaces the running session. */
  fun connect(host: String, port: Int, token: String) {
    if (host.isBlank()) {
      listener.onError("invalid_host", "the development link host must not be empty", null)
      return
    }
    if (port <= 0 || port > 65535) {
      listener.onError("invalid_port", "the development link port must be between 1 and 65535", null)
      return
    }
    if (token.isEmpty()) {
      listener.onError("invalid_token", "the development link needs a session token", null)
      return
    }
    commands.clear()
    commands.offer(Command.Connect(host, port, token))
    startWorker()
  }

  /** Stop the client and release the socket. */
  fun disconnect() {
    commands.clear()
    commands.offer(Command.Disconnect())
    running.set(false)
    connected = false
    worker?.interrupt()
  }

  /** Ask the simulator for the committed customization. */
  fun requestSync() {
    commands.offer(Command.Sync())
    startWorker()
  }

  /**
   * Change the customization.
   *
   * The change is applied on top of the last state the simulator reported, so
   * a partial change never clears the parts it does not mention.
   */
  fun applyCustomization(patchJson: String) {
    commands.offer(Command.Apply(patchJson))
    startWorker()
  }

  /** True once the simulator acknowledged the handshake. */
  fun isConnected(): Boolean = connected

  /** The committed customization the simulator last reported. */
  fun currentCustomization(): String? = currentCustomization

  /** The revision of the committed customization the simulator last reported. */
  fun currentRevision(): Long? = currentRevision

  @Volatile private var currentCustomization: String? = null

  @Volatile private var currentRevision: Long? = null

  private fun startWorker() {
    if (running.getAndSet(true)) {
      worker?.interrupt()
      return
    }
    val thread =
      Thread(
          {
            try {
              workerLoop()
            } finally {
              running.set(false)
            }
          },
          "rakazo-dev-link",
      )
    thread.isDaemon = true
    worker = thread
    thread.start()
  }

  private fun workerLoop() {
    var session: Session? = null
    try {
      while (running.get()) {
        val command = commands.poll()
        if (command != null) {
          session = handleCommand(session, command)
          continue
        }
        val current = session
        val input = current?.input
        if (current == null || current.socket == null || input == null) {
          Thread.sleep(20)
          continue
        }
        val frame =
          try {
            current.reader.read(input)
          } catch (error: IOException) {
            listener.onError("connection_lost", error.message ?: "the connection was lost", null)
            current.close()
            session = null
            connected = false
            emitState(STATUS_DISCONNECTED, error.message)
            continue
          }
        if (frame != null) {
          handleFrame(current, frame)
        } else {
          expirePending(current)
        }
      }
    } catch (_: InterruptedException) {
      // The client is stopping.
    } finally {
      session?.close()
      connected = false
      currentCustomization = null
      currentRevision = null
    }
  }

  private fun handleCommand(session: Session?, command: Command): Session? {
    return when (command) {
      is Command.Connect -> {
        session?.close()
        connected = false
        val fresh = Session()
        emitState(STATUS_CONNECTING, null)
        try {
          val socket = Socket()
          socket.connect(InetSocketAddress(command.host, command.port), config.connectTimeoutMs)
          socket.soTimeout = config.readTimeoutMs
          socket.tcpNoDelay = true
          fresh.socket = socket
          fresh.input = BufferedInputStream(socket.getInputStream())
          fresh.output = BufferedOutputStream(socket.getOutputStream())
          fresh.token = command.token
          fresh.handshakeRequestId = nextRequestId(fresh)
          emitState(STATUS_AUTHENTICATING, null)
          send(fresh, DevLinkProtocol.handshake(fresh.handshakeRequestId, command.token))
          fresh
        } catch (error: Exception) {
          // A missing permission or a refused connection must not take the
          // worker down; the caller is told instead.
          fresh.close()
          listener.onError("connect_failed", error.message ?: "the simulator could not be reached", null)
          emitState(STATUS_ERROR, error.message)
          null
        }
      }
      is Command.Disconnect -> {
        session?.close()
        connected = false
        currentCustomization = null
        currentRevision = null
        emitState(STATUS_DISCONNECTED, null)
        null
      }
      is Command.Sync -> {
        val current = session
        if (current == null || current.socket == null) {
          listener.onError("not_connected", "the development link is not connected", null)
          return session
        }
        val requestId = nextRequestId(current)
        val frame = encode(DevLinkProtocol.hello(requestId))
        if (frame == null) return session
        send(current, frame)
        register(current, requestId, frame, DevLinkProtocol.KIND_HELLO)
        session
      }
      is Command.Apply -> {
        val current = session
        if (current == null || current.socket == null) {
          listener.onError("not_connected", "the development link is not connected", null)
          return session
        }
        val known = current.customizationJson
        val revision = current.revision
        if (known == null || revision == null) {
          listener.onError(
            "state_unknown",
            "the simulator state is not known yet, wait for the handshake to complete",
            null,
          )
          return session
        }
        val patch =
          try {
            JSONObject(command.patchJson)
          } catch (error: Exception) {
            listener.onError("invalid_change", error.message ?: "the change is not valid JSON", null)
            return session
          }
        val customization = DevLinkProtocol.mergeCustomization(JSONObject(known), patch)
        val requestId = nextRequestId(current)
        val frame = encode(DevLinkProtocol.customization(requestId, revision, customization))
        if (frame == null) return session
        send(current, frame)
        register(current, requestId, frame, DevLinkProtocol.KIND_MESSAGE)
        session
      }
    }
  }

  private fun encode(message: JSONObject): ByteArray? {
    val payload = message.toString().toByteArray(Charsets.UTF_8)
    return try {
      DevLinkProtocol.encodeFrame(payload)
    } catch (error: IOException) {
      listener.onError("frame_too_large", error.message ?: "the change does not fit in one frame", null)
      null
    }
  }

  private fun nextRequestId(session: Session): Long {
    session.nextRequestId += 1
    return session.nextRequestId
  }

  private fun register(session: Session, requestId: Long, frame: ByteArray, kind: String) {
    if (session.pending.size >= config.maxPending) {
      listener.onError("queue_full", "too many requests are waiting for an answer", requestId)
      return
    }
    session.pending[requestId] =
      Pending(
        requestId = requestId,
        frame = frame,
        kind = kind,
        attempts = 1,
        deadline = System.currentTimeMillis() + config.requestTimeoutMs,
      )
  }

  private fun expirePending(session: Session) {
    val now = System.currentTimeMillis()
    val expired = session.pending.values.filter { now >= it.deadline }
    for (pending in expired) {
      if (pending.attempts < config.maxAttempts) {
        // The simulator answers a repeated request id with the reply it already
        // gave, so a retry can never apply the change twice.
        pending.attempts += 1
        pending.deadline = now + config.requestTimeoutMs
        try {
          send(session, pending.frame)
        } catch (error: IOException) {
          listener.onError(
            "retry_failed",
            error.message ?: "the request could not be sent again",
            pending.requestId,
          )
          session.pending.remove(pending.requestId)
          continue
        }
        listener.onError("request_retried", "the simulator did not answer in time", pending.requestId)
      } else {
        session.pending.remove(pending.requestId)
        listener.onError("request_timeout", "the simulator did not answer the request", pending.requestId)
        listener.onAck(pending.requestId, false, null, "timeout")
      }
    }
  }

  private fun send(session: Session, message: JSONObject) {
    val frame = encode(message) ?: throw IOException("the message could not be encoded")
    send(session, frame)
  }

  private fun send(session: Session, frame: ByteArray) {
    val output = session.output ?: throw IOException("the connection is not open")
    output.write(frame)
    output.flush()
  }

  private fun handleFrame(session: Session, frame: ByteArray) {
    val message =
      try {
        DevLinkProtocol.decode(frame)
      } catch (error: IOException) {
        listener.onError("invalid_frame", error.message ?: "the simulator sent an invalid frame", null)
        return
      }
    val kind = message.optString("kind")
    val replyTo = if (message.has("reply_to")) message.optLong("reply_to") else -1L
    val revision = if (message.has("revision")) message.optLong("revision") else null
    when (kind) {
      DevLinkProtocol.KIND_ACK -> {
        if (replyTo == session.handshakeRequestId) {
          session.authenticated = true
          connected = true
          // The authoritative state is reported first: the link only counts as
          // connected once the simulator told the client what it holds.
          reportCustomization(session, message, revision)
          emitState(STATUS_CONNECTED, null)
          return
        }
        val pending = session.pending.remove(replyTo)
        if (pending == null) {
          listener.onError("unexpected_reply", "the simulator answered a request that is not known", replyTo)
          return
        }
        reportCustomization(session, message, revision)
        listener.onAck(replyTo, true, revision, null)
      }
      DevLinkProtocol.KIND_ERROR -> {
        val code = message.optString("code", "internal")
        if (replyTo == session.handshakeRequestId) {
          connected = false
          session.authenticated = false
          listener.onError(code, "the simulator refused the session", replyTo)
          emitState(STATUS_ERROR, code)
          return
        }
        val pending = session.pending.remove(replyTo)
        if (pending == null) {
          listener.onError(code, "the simulator reported an error for an unknown request", replyTo)
          return
        }
        // A conflict carries the authoritative state, so the client can show
        // what the simulator actually holds.
        reportCustomization(session, message, revision)
        listener.onAck(replyTo, false, revision, code)
        listener.onError(code, "the simulator refused the change", replyTo)
      }
      DevLinkProtocol.KIND_MESSAGE -> {
        // A change the simulator committed on its own.
        reportCustomization(session, message, revision)
      }
      DevLinkProtocol.KIND_BYE -> {
        connected = false
        session.authenticated = false
        emitState(STATUS_DISCONNECTED, "the simulator closed the session")
      }
      else -> {
        listener.onError("unknown_message", "the simulator sent an unknown message", replyTo)
      }
    }
  }

  private fun reportCustomization(session: Session, message: JSONObject, revision: Long?) {
    val customization = DevLinkProtocol.customizationOf(message) ?: return
    val text = customization.toString()
    val reported = revision ?: session.revision
    if (reported != null) {
      session.revision = reported
      currentRevision = reported
    }
    session.customizationJson = text
    currentCustomization = text
    if (reported != null) {
      listener.onCustomization(reported, text)
    }
  }
}
