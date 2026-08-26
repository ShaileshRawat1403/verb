package com.example.verb.terminal

import android.annotation.SuppressLint
import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.example.R

/**
 * Keeps the process alive while a terminal session is live, so backgrounding Verb no longer hands
 * the PTY -- and whatever command or agent runs inside it -- to the low-memory killer. This is the
 * honest boundary from docs/DURABLE_SESSION.md: a foreground service lowers the probability of a
 * background kill; it is not immunity, and a force-stop still ends everything.
 *
 * The service owns nothing. The session stays where it is; this is only a priority claim, so there
 * is nothing to reattach to and nothing to clean up beyond the notification.
 */
class TerminalHoldService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    /**
     * Channels through the compat API, not the platform one. `NotificationChannel` arrived in API
     * 26 and `minSdk` is 24, so the platform call was an unguarded crash on Android 7 -- and a lint
     * error that failed CI. The compat builder is a no-op below 26, which is the correct behaviour:
     * there are no channels to declare there.
     */
    override fun onCreate() {
        super.onCreate()
        NotificationManagerCompat.from(this).createNotificationChannel(
            NotificationChannelCompat.Builder(CHANNEL_ID, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName(getText(R.string.terminal_hold_notification_title))
                .setDescription(getText(R.string.terminal_hold_notification_text).toString())
                .setShowBadge(false)
                .build()
        )
    }

    // START_STICKY rather than NOT_STICKY: if the system does kill us under real memory pressure,
    // a restart attempt costs nothing and re-claims the priority even though the session itself
    // would be gone.
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notification: Notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setContentTitle(getText(R.string.terminal_hold_notification_title))
            .setContentText(getText(R.string.terminal_hold_notification_text))
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            // Only when there is somewhere to go. A launch intent is normally present, but a
            // PendingIntent wrapping null throws, and a notification that cannot be tapped is a
            // smaller failure than a service that cannot start.
            .apply {
                packageManager.getLaunchIntentForPackage(packageName)?.let { launch ->
                    setContentIntent(
                        PendingIntent.getActivity(
                            this@TerminalHoldService,
                            0,
                            launch,
                            PendingIntent.FLAG_IMMUTABLE
                        )
                    )
                }
            }
            .build()
        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            notification,
            foregroundServiceTypeFor(Build.VERSION.SDK_INT)
        )
        return START_STICKY
    }

    companion object {
        private const val CHANNEL_ID = "terminal_hold"
        private const val NOTIFICATION_ID = 1

        /**
         * `specialUse` is an API 34 foreground-service type, and the platform requires the type
         * passed to `startForeground` to be a subset of what the manifest declares. An API 30
         * device cannot parse `android:foregroundServiceType="specialUse"`, so it reads no type at
         * all -- and asking for one anyway makes `startForeground` throw. The service then never
         * reaches the foreground, and the system kills the whole process with
         * `Context.startForegroundService() did not then call Service.startForeground()`.
         *
         * That crash took the app down on Android 11 the moment a session started. It was invisible
         * on the API 34 phone this was developed against, and was caught by CI's API 30 emulator.
         *
         * Below 34 the hold still works: a foreground service without a declared type is exactly
         * what the platform expected before types existed.
         */
        @SuppressLint("InlinedApi") // Compile-time constant; the call site is guarded by sdkInt.
        internal fun foregroundServiceTypeFor(sdkInt: Int): Int =
            if (sdkInt >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
            } else {
                0
            }

        fun start(context: Context) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, TerminalHoldService::class.java)
            )
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, TerminalHoldService::class.java))
        }
    }
}
