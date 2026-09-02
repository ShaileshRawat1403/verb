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

    companion object {
        /**
         * The one agent home every Agent Runtime session binds to `/home/verb`.
         *
         * Named here because three separate call sites carried the bare string `"default"`, and an
         * install that writes into a different home than the session later reads is
         * indistinguishable, from the user's side, from an agent that lost its sign-in.
         */
        const val DEFAULT_AGENT = "default"
    }
}
