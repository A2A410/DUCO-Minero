package com.example.duco_miner

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Bundle
import android.widget.SeekBar
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.localbroadcastmanager.content.LocalBroadcastManager
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.switchmaterial.SwitchMaterial

class MainActivity : AppCompatActivity() {

    private var isMining = false

    // UI Components
    private lateinit var themeSwitch: SwitchMaterial
    private lateinit var usernameEditText: com.google.android.material.textfield.TextInputEditText
    private lateinit var coresLabel: TextView
    private lateinit var coresSeekbar: SeekBar
    private lateinit var threadsLabel: TextView
    private lateinit var threadsSeekbar: SeekBar
    private lateinit var controlButton: com.google.android.material.button.MaterialButton
    private lateinit var debugButton: com.google.android.material.button.MaterialButton
    private lateinit var logView: TextView

    private val miningUpdateReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            val message = intent?.getStringExtra(MiningService.EXTRA_MESSAGE) ?: return
            if (message == "STOPPED") {
                isMining = false
                updateButtonUI()
                add_string_to_console("Mining stopped.")
            } else {
                add_string_to_console(message)
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        // Initialize UI Components
        themeSwitch = findViewById(R.id.theme_switch)
        usernameEditText = findViewById(R.id.username_edit_text)
        coresLabel = findViewById(R.id.cores_label)
        coresSeekbar = findViewById(R.id.cores_seekbar)
        threadsLabel = findViewById(R.id.threads_label)
        threadsSeekbar = findViewById(R.id.threads_seekbar)
        controlButton = findViewById(R.id.control_button)
        debugButton = findViewById(R.id.debug_button)
        logView = findViewById(R.id.log_view)

        setupThemeSwitch()
        setupSeekBars()

        controlButton.setOnClickListener {
            controlMining()
        }

        debugButton.setOnClickListener {
            showDebugInfo()
        }
    }

    override fun onResume() {
        super.onResume()
        LocalBroadcastManager.getInstance(this).registerReceiver(
            miningUpdateReceiver,
            IntentFilter(MiningService.ACTION_MINING_UPDATE)
        )
    }

    override fun onPause() {
        super.onPause()
        LocalBroadcastManager.getInstance(this).unregisterReceiver(miningUpdateReceiver)
    }

    private external fun getDebugInfo(): String

    companion object {
        init {
            System.loadLibrary("miner")
        }
    }

    private fun showDebugInfo() {
        // In a real app, we would get this from the service, but for now, we call the native function directly.
        val debugInfo = getDebugInfo()
        MaterialAlertDialogBuilder(this)
            .setTitle("Debug Info")
            .setMessage(debugInfo)
            .setPositiveButton("OK", null)
            .show()
    }

    private fun controlMining() {
        val serviceIntent = Intent(this, MiningService::class.java)
        if (!isMining) {
            val cores = coresSeekbar.progress
            val threadsPerCore = threadsSeekbar.progress

            if (cores * threadsPerCore == 0) {
                add_string_to_console("Please select at least one core and one thread.")
                return
            }

            val username = usernameEditText.text.toString()
            if (username.isEmpty()) {
                add_string_to_console("Please enter a username.")
                return
            }

            serviceIntent.putExtra("username", username)
            serviceIntent.putExtra("cores", cores)
            serviceIntent.putExtra("threads", threadsPerCore)
            startService(serviceIntent)

            isMining = true
            updateButtonUI()
            logView.text = "" // Clear logs
            add_string_to_console("Starting mining...")

        } else {
            stopService(serviceIntent)
            isMining = false
            updateButtonUI()
            add_string_to_console("Stopping mining...")
        }
    }

    private fun updateButtonUI() {
        if (isMining) {
            controlButton.text = "Stop Mining"
        } else {
            controlButton.text = "Start Mining"
        }
    }

    private fun setupThemeSwitch() {
        themeSwitch.setOnCheckedChangeListener { _, isChecked ->
            if (isChecked) {
                AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_YES)
            } else {
                AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_NO)
            }
        }
    }

    private fun setupSeekBars() {
        val maxCores = Runtime.getRuntime().availableProcessors()
        coresSeekbar.max = if (maxCores > 0) maxCores else 8

        coresSeekbar.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                val p = if (progress > 0) progress else 1
                coresSeekbar.progress = p
                coresLabel.text = "Number of Cores: $p"
            }
            override fun onStartTrackingTouch(seekBar: SeekBar?) {}
            override fun onStopTrackingTouch(seekBar: SeekBar?) {}
        })

        threadsSeekbar.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                val p = if (progress > 0) progress else 1
                threadsSeekbar.progress = p
                threadsLabel.text = "Threads per Core: $p"
            }
            override fun onStartTrackingTouch(seekBar: SeekBar?) {}
            override fun onStopTrackingTouch(seekBar: SeekBar?) {}
        })
    }

    private fun add_string_to_console(string: String) {
        runOnUiThread {
            logView.append("$string\n")
        }
    }
}