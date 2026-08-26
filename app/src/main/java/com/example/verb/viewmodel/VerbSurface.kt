package com.example.verb.viewmodel

/**
 * What Verb has deliberately put in front of the terminal, if anything.
 *
 * This replaces the five-tab `VerbTab` information architecture. The difference is not cosmetic:
 * a tab set is a permanent list of *product areas*, and `docs/UX_FOUNDATION.md` rejects exactly
 * that ("there is no permanent navigation chrome -- no tab bar, no sidebar, no menu strip. One key
 * opens everything"). The terminal is the workspace and the root; everything else is a task the
 * user asked for, which is why every case here is either "nothing" or "the thing the user opened".
 *
 * `docs/BACKLOG.md` D0 states the same constraint from the other side: anything agent-related is a
 * source underneath a human moment, never a top-level surface.
 */
sealed interface VerbSurface {

    /** The terminal owns the screen and the keyboard. This is the resting state. */
    data object None : VerbSurface

    /** The searchable Verb sheet: the touch equivalent of the desktop leader and palette. */
    data object Sheet : VerbSurface

    /** One named task, opened deliberately. */
    data class Task(val task: VerbTask) : VerbSurface
}

/**
 * Where a back gesture lands, or `null` when the terminal already owns the screen and back belongs
 * to the system.
 *
 * Pure, so the whole chain -- task to sheet to terminal to exit -- is testable without an Activity,
 * a ViewModel or a Compose tree. The rule it encodes: Verb's surfaces dismiss innermost first, and a
 * task returns to the sheet only when the sheet is where the user came from. Sending them to a sheet
 * they never opened would be Verb navigating on its own initiative, which is the one thing no
 * surface here is allowed to do.
 */
fun VerbSurface.afterBack(taskOpenedFromSheet: Boolean): VerbSurface? = when (this) {
    is VerbSurface.Task -> if (taskOpenedFromSheet) VerbSurface.Sheet else VerbSurface.None
    VerbSurface.Sheet -> VerbSurface.None
    VerbSurface.None -> null
}

/**
 * The named tasks reachable from the Verb sheet.
 *
 * These are *human tasks*, not subsystems: the title is what a person would call the thing they are
 * trying to do, and [keywords] exist so the sheet can be searched rather than browsed
 * (`docs/UX_FOUNDATION.md`, "Configuration is searched, not browsed"). Several tasks deliberately
 * share a destination -- "Add a provider key" and "Install a runtime" both land on the system
 * surface -- because the point of a named task is that the user can *find* it, not that it has its
 * own screen. Splitting that surface is a separate, visual piece of work.
 *
 * Every entry here maps to a capability that already exists. Nothing in this list is a promise.
 */
enum class VerbTask(
    val title: String,
    val subtitle: String,
    /** Extra search terms, so a person who types the word they know finds the task Verb calls it. */
    val keywords: String
) {
    ASK_VERB(
        title = "Ask Verb",
        subtitle = "Run a supported action on this device",
        keywords = "ask question query assistant ai natural language explain intent action run"
    ),
    AGENTS(
        title = "Start or install an agent",
        subtitle = "Claude Code, Codex or OpenCode",
        keywords = "agent agents claude codex opencode start launch open install new"
    ),
    SESSIONS(
        title = "Sessions and recovery",
        subtitle = "Resume what was interrupted, or start over",
        keywords = "session sessions resume recover recoverable interrupted ended restore continue"
    ),
    EVIDENCE(
        title = "What Verb knows",
        subtitle = "The environment and session facts Verb has observed",
        keywords = "evidence knows observed diagnostics environment context facts state truth"
    ),
    RUNS(
        title = "Command runs",
        subtitle = "The commands this session recorded, and how they ended",
        keywords = "runs commands history exit code duration boundaries recorded"
    ),
    PROVIDER(
        title = "Add or change a provider key",
        subtitle = "Only used when you ask Verb a question; nothing else sends anything",
        keywords = "provider key api model ai settings token configure interpretation"
    ),
    RUNTIMES(
        title = "Install a runtime",
        subtitle = "The toolchains an agent needs inside the guest",
        keywords = "runtime runtimes install node python toolchain guest userland profile packages"
    ),
    AGENT_RUNTIME(
        title = "Import an agent runtime",
        subtitle = "An archive, its checksum and its manifest",
        keywords = "agent runtime import archive checksum manifest rootfs bring own"
    ),
    WORKING_WORLD(
        title = "Save or restore my world",
        subtitle = "The working world archive for this device",
        keywords = "world working archive save restore backup export import downloads"
    ),
    CONTINUITY(
        title = "Move a session to another device",
        subtitle = "Manual, evidence-only continuity file",
        keywords = "continuity vcont move transfer another device export preview apply migrate"
    ),
    SYSTEM(
        title = "System and environment",
        subtitle = "How this install of Verb is set up",
        keywords = "system environment distribution setup about build flavor"
    );

    /** Case-insensitive match over the title, the subtitle and the extra search terms. */
    fun matches(query: String): Boolean {
        val trimmed = query.trim()
        if (trimmed.isEmpty()) return true
        val haystack = "$title $subtitle $keywords".lowercase()
        // Every whitespace-separated term must appear, so "install agent" narrows rather than widens.
        return trimmed.lowercase().split(' ').filter { it.isNotBlank() }.all { haystack.contains(it) }
    }
}
