package com.example.verb.terminal

import java.io.File

/**
 * Whether an agent has an authenticated session, as distinct from whether it runs.
 *
 * "Ready" has only ever meant the binary executes. That is not the question a user is asking when
 * they look at the Agents tab -- Codex and Claude Code were signed in while OpenCode and `dsh` were
 * not, and nothing in the UI distinguished them, so the tab looked identical in two states that
 * behave completely differently the moment you tap Open.
 */
enum class AgentSignInState {
    /** A credential this agent writes on sign-in is present. */
    SIGNED_IN,

    /** The agent declares where it keeps credentials, and nothing is there. */
    SIGNED_OUT,

    /**
     * Verb does not know where this agent keeps its credentials, so it says nothing.
     *
     * Deliberately a state rather than a default of "signed out". Guessing a path and reporting
     * absence from it would be inventing a fact -- the same mistake as the reverted `claude install`
     * command. A marker is added to the catalog only once it has been observed on a real device.
     */
    UNKNOWN
}

/**
 * Reports [AgentSignInState] from the presence of an agent's credential file.
 *
 * **Presence only.** These files are read never, opened never, logged never, included in the
 * diagnostics report never, and sent to a provider never -- exactly the boundary the API keys card
 * already holds. `File.exists()` is the whole of the check, and it is all that is needed: an agent
 * writes its credential file when a sign-in completes and removes it on sign-out.
 */
class AgentSignInDetector(private val filesDir: File) {

    fun stateFor(profile: RuntimeProfile): AgentSignInState {
        if (profile.signedInMarkers.isEmpty()) return AgentSignInState.UNKNOWN
        val home = when (profile.environment) {
            ProfileEnvironment.LOCAL_USERLAND -> File(filesDir, "home")
            ProfileEnvironment.AGENT_RUNTIME -> AgentRuntimePaths(filesDir).agentHome("default")
        }
        val signedIn = profile.signedInMarkers.any { marker ->
            runCatching { File(home, marker).exists() }.getOrDefault(false)
        }
        return if (signedIn) AgentSignInState.SIGNED_IN else AgentSignInState.SIGNED_OUT
    }
}
