package com.rakazo.devlink

import android.os.Handler
import android.os.Looper
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * Expo module that exposes the Charlie WatchOS development link to JavaScript.
 *
 * The module is a thin shell: every socket operation happens on the worker's
 * own thread, and the results are handed to JavaScript as events.
 *
 * The development link only makes sense in the Charlie Wrist Lab build. In any
 * other build the module refuses to connect, so the production app cannot talk
 * to a development simulator.
 */
class RakazoDevLinkModule : Module() {
  private val main = Handler(Looper.getMainLooper())
  private var client: DevLinkClient? = null

  override fun definition() = ModuleDefinition {
    Name("RakazoDevLink")
    Events("onDevLinkState", "onDevLinkCustomization", "onDevLinkAck", "onDevLinkError")

    AsyncFunction("connect") { host: String, port: Int, token: String ->
      require(labBuild()) { "the development link is only available in the Charlie Wrist Lab build" }
      require(token.isNotEmpty()) { "the development link needs a session token" }
      val link = client ?: DevLinkClient(listener()).also { client = it }
      link.connect(host, port, token)
      // The handshake is answered by the link itself, so there is nothing to
      // hand back to the caller.
      Unit
    }

    AsyncFunction("disconnect") {
      client?.disconnect()
    }

    AsyncFunction("requestSync") {
      require(labBuild()) { "the development link is only available in the Charlie Wrist Lab build" }
      client?.requestSync() ?: DevLinkClient.NO_REQUEST
    }

    AsyncFunction("applyCustomization") { patchJson: String ->
      require(labBuild()) { "the development link is only available in the Charlie Wrist Lab build" }
      client?.applyCustomization(patchJson) ?: DevLinkClient.NO_REQUEST
    }

    /**
     * Whether this build may talk to a development simulator.
     *
     * The answer is a plain fact about the running build, so the user interface
     * can hide the development link instead of offering a button that fails.
     */
    Function("isLabBuild") { labBuild() }

    AsyncFunction("currentState") {
      mapOf(
        "status" to (client?.status() ?: DevLinkClient.STATUS_IDLE),
        "connected" to (client?.isConnected() ?: false),
        "revision" to (client?.currentRevision() ?: -1L),
        "customization" to (client?.currentCustomization() ?: ""),
      )
    }

    OnDestroy {
      client?.disconnect()
      client = null
    }
  }

  /** The lab build is the only build allowed to talk to a development simulator. */
  private fun labBuild(): Boolean {
    val context = appContext.reactContext ?: return false
    return DevLinkLab.isLabPackage(context.packageName)
  }

  private fun listener() =
    object : DevLinkClient.Listener {
      override fun onState(status: String, reason: String?) {
        val payload = mutableMapOf<String, Any?>("status" to status)
        reason?.let { payload["reason"] = it }
        main.post { sendEvent("onDevLinkState", payload) }
      }

      override fun onCustomization(revision: Long, customizationJson: String) {
        val payload =
          mapOf<String, Any?>("revision" to revision, "customization" to customizationJson)
        main.post { sendEvent("onDevLinkCustomization", payload) }
      }

      override fun onAck(requestId: Long, ok: Boolean, revision: Long?, code: String?) {
        val payload =
          mutableMapOf<String, Any?>("requestId" to requestId, "ok" to ok)
        revision?.let { payload["revision"] = it }
        code?.let { payload["code"] = it }
        main.post { sendEvent("onDevLinkAck", payload) }
      }

      override fun onError(code: String, message: String, requestId: Long?) {
        val payload = mutableMapOf<String, Any?>("code" to code, "message" to message)
        requestId?.let { payload["requestId"] = it }
        main.post { sendEvent("onDevLinkError", payload) }
      }
    }
}
