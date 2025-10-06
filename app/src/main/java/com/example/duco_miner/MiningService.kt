package com.example.duco_miner

import android.app.Service
import android.content.Intent
import android.os.IBinder
import androidx.localbroadcastmanager.content.LocalBroadcastManager

class MiningService : Service() {

    private var isMining = false

    companion object {
        const val ACTION_MINING_UPDATE = "com.example.duco_miner.MINING_UPDATE"
        const val EXTRA_MESSAGE = "extra_message"

        init {
            System.loadLibrary("miner")
        }
    }

    private external fun startMining(username: String, cores: Int, threads: Int)
    private external fun stopMining()

    override fun onBind(intent: Intent): IBinder? {
        return null
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent != null && !isMining) {
            val username = intent.getStringExtra("username")
            val cores = intent.getIntExtra("cores", 1)
            val threads = intent.getIntExtra("threads", 1)

            if (username != null) {
                isMining = true
                startMining(username, cores, threads)
                sendMessage("Mining service started.")
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        super.onDestroy()
        if (isMining) {
            isMining = false
            stopMining()
            sendMessage("Mining service stopped.")
        }
    }

    // This method is called from the native Rust code
    fun onMiningEvent(message: String) {
        if (message == "STOPPED") {
            isMining = false
            stopSelf()
        }
        sendMessage(message)
    }

    private fun sendMessage(message: String) {
        val intent = Intent(ACTION_MINING_UPDATE).apply {
            putExtra(EXTRA_MESSAGE, message)
        }
        LocalBroadcastManager.getInstance(this).sendBroadcast(intent)
    }
}