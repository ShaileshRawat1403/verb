package com.example.verb.terminal

import android.content.Context
import android.content.pm.ServiceInfo
import android.os.Build
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Shadows
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The two ways the hold can fail the platform's `startForegroundService` contract.
 *
 * Android gives a service started that way a few seconds to reach the foreground, and kills the
 * whole process otherwise: `Context.startForegroundService() did not then call
 * Service.startForeground()`. That crash took the app down on Android 11 whenever a session
 * started, and was invisible on the API 34 phone this was developed against.
 *
 * **The cause was a race.** The hold follows the session, and a session that goes STARTING ->
 * FAILED quickly had its `stopService` land before `onStartCommand` had promoted the service, so
 * the countdown expired with nothing to show for it. CI's loaded emulator hits that window every
 * time because there is no real userland there for a session to start into. Releasing is now a
 * message to the service, which promotes first and then stands down.
 *
 * **The type was a second hazard, found while chasing the first and guarded anyway.** `specialUse`
 * is an API 34 type, the platform requires the type passed to `startForeground` to be a subset of
 * what the manifest declares, and an API 30 device cannot parse `specialUse` from the manifest at
 * all. Asking for it there was never going to be right, whether or not it was what crashed.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class TerminalHoldServiceTest {

    /**
     * Releasing must travel through the service, not around it.
     *
     * `stopService` from outside could land before `onStartCommand` had promoted the service, so
     * the platform's startForegroundService countdown expired and it killed the process. A session
     * that goes STARTING -> FAILED quickly hits that window every time, which is what CI's loaded
     * emulator reproduced and an API 34 phone never did.
     */
    @Test
    fun `releasing the hold is an action on the service, not a stop from outside`() {
        val context: Context = ApplicationProvider.getApplicationContext()

        TerminalHoldService.stop(context)

        val started = Shadows.shadowOf(context as android.app.Application).nextStartedService
        assertEquals(
            "release must be delivered to the service itself",
            TerminalHoldService::class.java.name,
            started.component?.className
        )
        assertEquals(TerminalHoldService.ACTION_RELEASE, started.action)
    }

    /** Holding is the same entry point with no action, so the two cannot diverge. */
    @Test
    fun `holding the process starts the same service with no action`() {
        val context: Context = ApplicationProvider.getApplicationContext()

        TerminalHoldService.start(context)

        val started = Shadows.shadowOf(context as android.app.Application).nextStartedService
        assertEquals(
            TerminalHoldService::class.java.name,
            started.component?.className
        )
        assertEquals(null, started.action)
    }

    @Test
    fun `API 34 and above asks for the special-use type it declares`() {
        assertEquals(
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE,
            TerminalHoldService.foregroundServiceTypeFor(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
        )
        assertEquals(
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE,
            TerminalHoldService.foregroundServiceTypeFor(35)
        )
    }

    /**
     * Every platform below 34 asks for no type. Not "a different type" — none. A foreground service
     * without a declared type is exactly what the platform expected before types existed, and it is
     * the only value guaranteed to be a subset of a manifest the platform could not parse.
     */
    @Test
    fun `below API 34 asks for no type at all`() {
        listOf(
            Build.VERSION_CODES.N,          // 24, the minSdk
            Build.VERSION_CODES.O,          // 26, where channels arrive
            Build.VERSION_CODES.Q,          // 29, where startForeground gained a type argument
            Build.VERSION_CODES.R,          // 30, the emulator CI runs and the crash was found on
            Build.VERSION_CODES.TIRAMISU    // 33, the last release before specialUse
        ).forEach { sdk ->
            assertEquals("SDK $sdk must request no foreground service type", 0,
                TerminalHoldService.foregroundServiceTypeFor(sdk))
        }
    }
}
