package com.example.verb.terminal

/**
 * Whether the Agent Runtime artifact is present on disk. Says nothing about whether it can run:
 * an archive can verify, extract, and expose every command the manifest declares while still being
 * unable to execute a single one of them inside this app's sandbox.
 */
enum class AgentArtifactState {
    NOT_INSTALLED,
    INSTALLED
}

/**
 * Whether the installed runtime has been observed to actually execute here.
 *
 * The distinction from [AgentArtifactState] is the point: Verb previously showed "Installed" and
 * offered a launch button purely because files were on disk, and the session then died immediately
 * with no explanation. Compatibility is only ever [COMPATIBLE] after a real bounded command has
 * run in the real environment and exited 0.
 */
enum class AgentCompatibilityState {
    /** Installed, but nothing has been run yet. Not an assertion either way. */
    NOT_CHECKED,

    /** A check is in flight. */
    CHECKING,

    /** A bounded command ran inside the runtime and exited 0. */
    COMPATIBLE,

    /** A bounded command ran and failed. The artifact is fine; this device cannot execute it. */
    INCOMPATIBLE,

    /** The check could not be carried out (the probe process never started, environment missing). */
    CHECK_FAILED,

    /** The check exceeded its bound. Distinct from [CHECK_FAILED]: nothing was concluded. */
    CHECK_TIMED_OUT
}

/**
 * The two facts, kept apart, plus the artifact they describe.
 *
 * [canOpen] is the single authority for whether an Agent Runtime session may be started. Both the
 * UI's enablement and the view-model's programmatic guard read it, so a caller cannot reach the
 * runtime by a path that skips the check.
 */
data class AgentRuntimeStatus(
    val artifact: AgentArtifactState = AgentArtifactState.NOT_INSTALLED,
    val compatibility: AgentCompatibilityState = AgentCompatibilityState.NOT_CHECKED,
    val runtime: AgentRuntimeInstaller.InstalledRuntime? = null
) {
    val isInstalled: Boolean get() = artifact == AgentArtifactState.INSTALLED && runtime != null

    /** Only a runtime proven to execute may be opened. Every other state is a refusal. */
    val canOpen: Boolean
        get() = isInstalled && compatibility == AgentCompatibilityState.COMPATIBLE

    /** A check may be started (or retried) whenever one is installed and none is already running. */
    val canCheck: Boolean
        get() = isInstalled && compatibility != AgentCompatibilityState.CHECKING
}
