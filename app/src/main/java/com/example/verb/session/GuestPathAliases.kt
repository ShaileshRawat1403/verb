package com.example.verb.session

import java.io.File

/**
 * Android exposes app-private storage under both `/data/user/0/<pkg>` and `/data/data/<pkg>`, and
 * which spelling an agent recorded in its own session metadata depends on how its process was
 * exec'd, not on anything Verb controls. Comparing those strings literally makes a genuinely
 * recoverable session look unrecoverable, so every adapter compares through here instead.
 */
object GuestPathAliases {

    fun aliasesOf(path: String): Set<String> = buildSet {
        add(path)
        add(path.replace("/data/user/0/", "/data/data/"))
        add(path.replace("/data/data/", "/data/user/0/"))
    }

    fun aliasesOf(file: File): Set<File> = aliasesOf(file.absolutePath).mapTo(mutableSetOf(), ::File)

    /**
     * True when [recordedPath] and [project] name the same directory under either Android alias.
     * The suffix comparison keeps this resilient to the two spellings without treating unrelated
     * projects as equal: both sides must be inside the app's `files` tree for it to apply.
     */
    fun sameDirectory(recordedPath: String, project: File): Boolean {
        val expected = project.absolutePath
        if (recordedPath in aliasesOf(expected)) return true

        val marker = "/files/"
        return recordedPath.substringAfter(marker, "") == expected.substringAfter(marker, "") &&
            marker in recordedPath && marker in expected
    }
}
