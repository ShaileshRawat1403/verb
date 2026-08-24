package com.example

import com.example.verb.ui.SystemSection
import com.example.verb.viewmodel.VerbTask
import org.junit.Assert.assertEquals
import org.junit.Test

class SystemTaskRoutingTest {

    @Test
    fun `every named system task lands on the matching section`() {
        val expected = mapOf(
            VerbTask.PROVIDER to SystemSection.PROVIDER,
            VerbTask.WORKING_WORLD to SystemSection.WORKING_WORLD,
            VerbTask.CONTINUITY to SystemSection.CONTINUITY,
            VerbTask.RUNTIMES to SystemSection.RUNTIMES,
            VerbTask.AGENT_RUNTIME to SystemSection.AGENT_RUNTIME,
            VerbTask.SYSTEM to SystemSection.OVERVIEW
        )

        expected.forEach { (task, section) ->
            assertEquals(task.name, section, systemSectionFor(task))
        }
    }
}
