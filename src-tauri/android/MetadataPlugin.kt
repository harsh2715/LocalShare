package com.localshare.desktop

import android.app.Activity
import android.net.Uri
import android.provider.OpenableColumns
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class ResolveMetadataArgs {
  lateinit var uri: String
}

/** Resolves the display name supplied by Android's document/media provider. */
@TauriPlugin
class MetadataPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun resolve(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(ResolveMetadataArgs::class.java)
      val uri = Uri.parse(args.uri)
      var displayName: String? = null
      val projection = arrayOf(OpenableColumns.DISPLAY_NAME)

      activity.contentResolver.query(uri, projection, null, null, null)?.use { cursor ->
        if (cursor.moveToFirst()) {
          val column = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
          if (column >= 0 && !cursor.isNull(column)) {
            displayName = cursor.getString(column)
          }
        }
      }

      if (displayName.isNullOrBlank()) {
        displayName = uri.lastPathSegment
      }
      if (displayName.isNullOrBlank()) {
        invoke.reject("Android did not provide a filename for the selected file")
        return
      }

      val result = JSObject()
      result.put("name", displayName)
      result.put("mimeType", activity.contentResolver.getType(uri))
      invoke.resolve(result)
    } catch (error: Exception) {
      invoke.reject(error.message ?: "Unable to resolve Android file metadata")
    }
  }
}
