package com.example.verb.terminal

import android.content.pm.ServiceInfo
import android.os.Build
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The foreground-service type the hold asks for, per platform version.
 *
 * This is a unit test for a one-line decision because the bug it pins was a process kill. The
 * service asked for `specialUse` — an API 34 type — on every platform. The system requires the type
 * passed to `startForeground` to be a subset of what the manifest declares, and an API 30 device
 * cannot parse `specialUse` from the manifest at all, so the call threw, the service never reached
 * the foreground, and Android killed the app with `Context.startForegroundService() did not then
 * call Service.startForeground()`.
 *
 * It took the app down on Android 11 the moment a session started, and was invisible on the API 34
 * phone this was developed against. CI's API 30 emulator caught it.
 */
class TerminalHoldServiceTest {

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
