package com.example

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The smoke test for the instrumentation itself: it proves the harness is talking to Verb, and not
 * to whatever else happens to be installed on the emulator.
 *
 * It arrived from the project template asserting `com.example`, which stopped being true the moment
 * the application id became the real one -- so every device-truth run on CI failed here, before any
 * test about Verb's behaviour got the chance to say anything. A prefix, because `play` appends a
 * suffix to the id and both flavours are the same app.
 */
@RunWith(AndroidJUnit4::class)
class ExampleInstrumentedTest {
  @Test
  fun theInstrumentationIsAttachedToVerb() {
    val appContext = InstrumentationRegistry.getInstrumentation().targetContext

    assertTrue(
      "instrumentation attached to ${appContext.packageName}",
      appContext.packageName.startsWith("com.aistudio.verb.app")
    )
  }
}
