/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package org.mozilla.places

import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.io.Closeable

open class PlacesConnection(path: String, encryption_key: String = "") : Closeable {

    private var db: RawPlacesConnection? = LibPlacesFFI.INSTANCE.places_connection_new(path, encryption_key)

    override fun close() {
        synchronized(LibPlacesFFI.INSTANCE) {
            val db = this.db
            this.db = null
            if (db != null) {
                LibPlacesFFI.INSTANCE.places_connection_destroy(db)
            }
        }
    }

    // This is actually handled internally by kotlin, apparently. Not sure
    // why it doesn't need an `override` but it doesn't.
    @Suppress("Unused")
    fun finalize() {
        this.close()
    }


    fun noteObservation(data: VisitObservation) {
        val json = data.toJSON().toString()
        synchronized(LibPlacesFFI.INSTANCE) {
            LibPlacesFFI.INSTANCE.places_note_observation(this.db!!, json)
        }
    }

    fun queryAutocomplete(query: String, limit: Int = 10): List<SearchResult> {
        val resultText = synchronized(LibPlacesFFI.INSTANCE) {
            val results = LibPlacesFFI.INSTANCE.places_query_autocomplete(this.db!!, query, limit)
            // TODO: handle results being null
            val decoded = results!!.getString(0, "utf8")
            LibPlacesFFI.INSTANCE.places_destroy_string(results)
            decoded
        }
        return SearchResult.fromJSONArray(resultText)
    }

}

data class VisitObservation(
    val url: String,
    val visitType: Int? = null,
    val title: String? = null,
    val isError: Boolean? = null,
    val isRedirectSource: Boolean? = null,
    val isPermanentRedirectSource: Boolean? = null,
    /** Milliseconds */
    val at: Long? = null,
    val referrer: String? = null,
    val isRemote: Boolean? = null
) {
    fun toJSON(): JSONObject {
        val o = JSONObject()
        o.put("url", this.url)
        this.visitType?.let { o.put("visit_type", it) }
        this.isError?.let { o.put("is_error", it) }
        this.isRedirectSource?.let { o.put("is_redirect_source", it) }
        this.isPermanentRedirectSource?.let { o.put("is_permanent_redirect_source", it) }
        this.at?.let { o.put("at", it) }
        this.referrer?.let { o.put("referrer", it) }
        this.isRemote?.let { o.put("is_remot", it) }
        return o
    }
}

data class SearchResult(
    val searchString: String,
    val url: String,
    val title: String,
    val frecency: Long,
    val iconUrl: String? = null
    // Skipping `reasons` for now...
) {
    companion object {
        fun fromJSON(jsonObject: JSONObject): SearchResult {
            fun stringOrNull(key: String): String? {
                try {
                    return jsonObject.getString(key)
                } catch (e: JSONException) {
                    return null
                }
            }

            return SearchResult(
                searchString = jsonObject.getString("search_string"),
                url = jsonObject.getString("url"),
                title = jsonObject.getString("title"),
                frecency = jsonObject.getLong("frecency"),
                iconUrl = stringOrNull("icon_url")
            )
        }

        fun fromJSONArray(jsonArrayText: String): List<SearchResult> {
            val result: MutableList<SearchResult> = mutableListOf()
            val array = JSONArray(jsonArrayText)
            for (index in 0 until array.length()) {
                result.add(fromJSON(array.getJSONObject(index)))
            }
            return result
        }
    }
}
