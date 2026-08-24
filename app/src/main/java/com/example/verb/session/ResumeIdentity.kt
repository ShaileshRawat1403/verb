package com.example.verb.session

/** The only syntax Verb will accept for an agent-owned conversation reference. */
object ResumeIdentity {
    private val SAFE = Regex("^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")

    fun isValid(value: String): Boolean = SAFE.matches(value)

    fun validOrNull(value: String?): String? = value?.takeIf(::isValid)
}
