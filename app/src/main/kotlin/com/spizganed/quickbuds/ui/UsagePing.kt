package com.spizganed.quickbuds.ui

import android.content.Context
import org.json.JSONObject

/** Usage telemetry is disabled in DeviceDeck. This object remains for compatibility with old preferences. */
object UsagePing {
    const val KEY_ENABLED = "usagePingEnabled"
    internal fun pingBody(version: String, id: String, ack: Long): String =
        JSONObject().put("platform", "Android").put("version", version).put("id", id).put("ack", ack).toString()
    internal fun shouldStoreReply(text: String, replyId: Long, storedReplyId: Long, waiting: Boolean): Boolean =
        text.isNotBlank() && replyId > 0 && replyId != storedReplyId && !waiting
    fun send(c: Context) = Unit
    fun takeReply(c: Context): String? = null
    fun resetId(c: Context) = Unit
    fun appId(c: Context): String = ""
}
