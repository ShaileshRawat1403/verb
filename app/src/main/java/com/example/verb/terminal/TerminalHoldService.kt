package com.example.verb.terminal

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.NotificationCompat
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

    override fun onCreate() {
        super.onCreate()
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                getText(R.string.terminal_hold_notification_title),
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = getText(R.string.terminal_hold_notification_text).toString()
                setShowBadge(false)
            }
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
            .setContentIntent(
                PendingIntent.getActivity(
                    this,
                    0,
                    packageManager.getLaunchIntentForPackage(packageName),
                    PendingIntent.FLAG_IMMUTABLE
                )
            )
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
