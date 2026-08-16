package com.example.verb.terminal

import java.io.File

/** App-private storage layout for replaceable Linux agent runtimes. */
class AgentRuntimePaths(filesDir: File) {
    val root: File = File(filesDir, "agent-runtime")
    val versions: File = File(root, "versions")
    val activeManifest: File = File(root, "active.properties")
    val homes: File = File(root, "homes")
    val cache: File = File(root, "cache")

    fun version(version: String): File = File(versions, version)

    fun versionRootfs(version: String): File = File(version(version), "rootfs")

    fun agentHome(agent: String): File = File(homes, agent)
}
