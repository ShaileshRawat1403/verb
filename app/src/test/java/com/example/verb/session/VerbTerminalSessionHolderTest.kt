package com.example.verb.session

import com.example.verb.terminal.TerminalRuntime
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertSame
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class VerbTerminalSessionHolderTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @After
    fun tearDown() {
        VerbTerminalSessionHolder.resetForTests()
    }

    private fun newRuntime(): TerminalRuntime {
        val filesDir = temporaryFolder.newFolder("files-${System.nanoTime()}")
        return TerminalRuntime(workingDir = filesDir, useFakeForTesting = true)
    }

    @Test
    fun `getOrCreate returns the same instance on a second call`() {
        val first = VerbTerminalSessionHolder.getOrCreate { newRuntime() }
        val second = VerbTerminalSessionHolder.getOrCreate { newRuntime() }

        assertSame("a second VerbViewModel must reattach, not spawn a duplicate session", first, second)
    }

    @Test
    fun `the factory does not run once a runtime already exists`() {
        VerbTerminalSessionHolder.getOrCreate { newRuntime() }
        var factoryRan = false

        VerbTerminalSessionHolder.getOrCreate {
            factoryRan = true
            newRuntime()
        }

        assertFalse("reattaching must not construct a second TerminalRuntime", factoryRan)
    }

    @Test
    fun `resetForTests clears the held instance`() {
        val first = VerbTerminalSessionHolder.getOrCreate { newRuntime() }

        VerbTerminalSessionHolder.resetForTests()
        val second = VerbTerminalSessionHolder.getOrCreate { newRuntime() }

        assertNotSame(first, second)
    }
}
