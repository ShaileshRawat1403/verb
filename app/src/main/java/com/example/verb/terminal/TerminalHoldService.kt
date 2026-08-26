package com.example.verb.terminal

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
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
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
        )
        return START_STICKY
    }

    companion object {
        private const val CHANNEL_ID = "terminal_hold"
        private const val NOTIFICATION_ID = 1

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
