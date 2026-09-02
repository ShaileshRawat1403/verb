package com.example.verb.project

import android.content.Context
import java.io.File
import java.util.UUID

data class VerbProject(val id: String, val directory: File) {
    /**
     * The name the person typed, without the suffix Verb appends to keep ids unique.
     *
     * Shown wherever a person is choosing between their own projects. A full id
     * (`mobile-kit-30603ae7`) reads as machine output, and a list of them is genuinely hard to
     * recognise your own work in -- which is the state the project sheet was in.
     */
    val displayName: String
        get() = id.substringBeforeLast('-', id).ifEmpty { id }

    /**
     * The disambiguating suffix, kept visible because two projects may share a name and this is
     * the only thing that separates them. Empty when an id carries no suffix.
     */
    val shortId: String
        get() = id.substringAfterLast('-', "")
}

/** Owns the app-private project root; arbitrary external paths are never accepted. */
class ProjectWorkspace(filesDir: File) {
    val root: File = File(filesDir, "projects")

    init {
        check(root.exists() || root.mkdirs()) { "Could not create ${root.absolutePath}" }
    }

    fun list(): List<VerbProject> = root.listFiles()
        ?.filter { it.isDirectory && isContained(it) }
        ?.map { VerbProject(it.name, it) }
        ?.sortedBy { it.id }
        .orEmpty()

    fun create(name: String): VerbProject {
        val prefix = name.trim().lowercase().replace(Regex("[^a-z0-9]+"), "-").trim('-')
            .takeIf { it.isNotEmpty() } ?: "project"
        val id = "$prefix-${UUID.randomUUID().toString().take(8)}"
        val directory = File(root, id)
        // Validate the target before mutating the filesystem. A failed post-mkdir containment
        // check used to leave an orphan directory while the UI silently closed as if creation had
        // succeeded.
        check(isContained(directory)) { "Project path escaped the app-owned root" }
        check(directory.mkdir()) { "Could not create project" }
        return VerbProject(id, directory)
    }

    fun get(id: String): VerbProject? = id.takeIf(::isSafeId)
        ?.let { File(root, it) }
        ?.takeIf { it.isDirectory && isContained(it) }
        ?.let { VerbProject(id, it) }

    fun isContained(directory: File): Boolean = runCatching {
        directory.canonicalFile.parentFile == root.canonicalFile
    }.getOrDefault(false)

    private fun isSafeId(id: String): Boolean = id.matches(Regex("[a-z0-9][a-z0-9-]*"))
}

class ProjectRepository(context: Context) {
    private val workspace = ProjectWorkspace(context.filesDir)
    private val preferences = context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)

    fun list(): List<VerbProject> = workspace.list()
    fun create(name: String): VerbProject = workspace.create(name).also(::select)

    fun selected(): VerbProject? {
        val id = preferences.getString(KEY_SELECTED_ID, null) ?: return null
        val project = workspace.get(id) ?: return null
        val savedPath = preferences.getString(KEY_SELECTED_PATH, null) ?: return null
        return project.takeIf { it.directory.canonicalPath == savedPath }
    }

    fun select(id: String): VerbProject? = workspace.get(id)?.also(::select)

    private fun select(project: VerbProject) {
        preferences.edit()
            .putString(KEY_SELECTED_ID, project.id)
            .putString(KEY_SELECTED_PATH, project.directory.canonicalPath)
            .apply()
    }

    private companion object {
        const val PREFERENCES_NAME = "verb_projects"
        const val KEY_SELECTED_ID = "selected_id"
        const val KEY_SELECTED_PATH = "selected_path"
    }
}
