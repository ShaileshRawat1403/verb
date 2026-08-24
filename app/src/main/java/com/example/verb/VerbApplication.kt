package com.example.verb

import android.app.Application
import com.example.verb.privacy.LegacyPrivateDataPurge

/** Process entry point. Privacy migrations run before any screen or agent runtime is created. */
class VerbApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        LegacyPrivateDataPurge.run(this)
    }
}
