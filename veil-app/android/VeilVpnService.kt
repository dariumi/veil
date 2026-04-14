package com.veilproject.veil

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.core.app.NotificationCompat

/**
 * VeilVpnService — Android VPN implementation.
 *
 * Lifecycle:
 *   1. MainActivity sends ACTION_START_VPN intent
 *   2. onStartCommand() calls VpnService.Builder to establish TUN interface
 *   3. TUN fd passed to Rust via JNI [onTunReady]
 *   4. Rust bridges TUN → local SOCKS5 → Veil QUIC tunnel
 *   5. ACTION_STOP_VPN tears down the service
 */
class VeilVpnService : VpnService() {

    companion object {
        private const val TAG = "VeilVpnService"
        private const val NOTIFICATION_ID = 1
        private const val CHANNEL_ID = "veil_vpn"

        const val ACTION_START_VPN = "com.veilproject.veil.START_VPN"
        const val ACTION_STOP_VPN  = "com.veilproject.veil.STOP_VPN"
        const val EXTRA_SERVER     = "server"
        const val EXTRA_TOKEN      = "token"

        // JNI: called by Rust → Kotlin (to start VPN from Tauri command)
        @JvmStatic
        external fun onTunReady(fd: Int)

        // JNI: called by Android system when VPN is revoked
        @JvmStatic
        external fun onVpnRevoked()
    }

    private var tunInterface: ParcelFileDescriptor? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START_VPN -> {
                val server = intent.getStringExtra(EXTRA_SERVER) ?: return START_NOT_STICKY
                val token  = intent.getStringExtra(EXTRA_TOKEN)  ?: return START_NOT_STICKY
                startVpn(server, token)
            }
            ACTION_STOP_VPN -> stopVpn()
        }
        return START_NOT_STICKY
    }

    private fun startVpn(server: String, token: String) {
        Log.i(TAG, "Starting VPN → $server")
        showNotification()

        // Build TUN interface
        val builder = Builder()
            .setSession("Veil VPN")
            // VPN IP address (Veil server assigns us 10.10.0.2)
            .addAddress("10.10.0.2", 24)
            // Route all traffic through VPN
            .addRoute("0.0.0.0", 0)
            .addRoute("::", 0)
            // Use Cloudflare DNS (Veil will intercept DNS via DoH)
            .addDnsServer("1.1.1.1")
            .addDnsServer("8.8.8.8")
            .setMtu(1500)
            // Exclude Veil app itself from VPN (avoid routing loop)
            .addDisallowedApplication(packageName)

        val pfd = builder.establish()
            ?: run {
                Log.e(TAG, "VpnService.Builder.establish() returned null — permission denied?")
                stopSelf()
                return
            }

        tunInterface = pfd

        // Detach fd: passes ownership to Rust (Rust will close it)
        val fd = pfd.detachFd()
        Log.i(TAG, "TUN fd ready: $fd, notifying Rust")

        // JNI call → Rust vpn_android::onTunReady
        onTunReady(fd)
    }

    private fun stopVpn() {
        Log.i(TAG, "Stopping VPN")
        onVpnRevoked()
        tunInterface?.close()
        tunInterface = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onRevoke() {
        Log.w(TAG, "VPN revoked by system")
        onVpnRevoked()
        tunInterface?.close()
        tunInterface = null
        stopForeground(STOP_FOREGROUND_REMOVE)
    }

    private fun showNotification() {
        val manager = getSystemService(NOTIFICATION_SERVICE) as NotificationManager

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Veil VPN",
                NotificationManager.IMPORTANCE_LOW
            ).apply { description = "Veil VPN tunnel active" }
            manager.createNotificationChannel(channel)
        }

        val stopIntent = PendingIntent.getService(
            this, 0,
            Intent(this, VeilVpnService::class.java).apply { action = ACTION_STOP_VPN },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Veil VPN")
            .setContentText("Connected — traffic is protected")
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .addAction(android.R.drawable.ic_media_pause, "Disconnect", stopIntent)
            .setOngoing(true)
            .build()

        startForeground(NOTIFICATION_ID, notification)
    }
}
