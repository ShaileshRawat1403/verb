import java.util.Properties

plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.kotlin.compose)
  alias(libs.plugins.google.devtools.ksp)
  alias(libs.plugins.roborazzi)
  alias(libs.plugins.secrets)
}

/**
 * Read once, from `version.properties`, so there is exactly one literal in the repository that
 * says what this build is. `-PverbVersionName` / `-PverbVersionCode` override it, which is how the
 * release workflow states the version it was asked for explicitly rather than inheriting whatever
 * happened to be committed.
 */
val verbVersion = Properties().apply {
  rootProject.file("version.properties").inputStream().use(::load)
}
val verbVersionName: String =
  (project.findProperty("verbVersionName") as String?) ?: verbVersion.getProperty("versionName")
val verbVersionCode: Int =
  ((project.findProperty("verbVersionCode") as String?) ?: verbVersion.getProperty("versionCode"))
    .trim()
    .toInt()

android {
  namespace = "com.example"
  compileSdk { version = release(36) { minorApiLevel = 1 } }

  defaultConfig {
    applicationId = "com.aistudio.verb.app"
    minSdk = 24
    targetSdk = 36
    versionCode = verbVersionCode
    versionName = verbVersionName

    ndk {
      abiFilters.add("arm64-v8a")
    }

    testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
  }

  flavorDimensions += "distribution"
  productFlavors {
    create("fullCli") {
      dimension = "distribution"
      // Android blocks execution from writable app storage for newer targets. This distribution
      // keeps the Termux-compatible, proot-backed CLI available for direct distribution.
      targetSdk = 28
      buildConfigField("boolean", "FULL_CLI", "true")
    }
    create("play") {
      dimension = "distribution"
      applicationIdSuffix = ".play"
      versionNameSuffix = "-play"
      targetSdk = 36
      buildConfigField("boolean", "FULL_CLI", "false")
    }
  }

  externalNativeBuild {
    ndkBuild {
      path = file("src/main/jni/Android.mk")
    }
  }

  signingConfigs {
    create("release") {
      val keystorePath = System.getenv("KEYSTORE_PATH") ?: "${rootDir}/my-upload-key.jks"
      storeFile = file(keystorePath)
      storePassword = System.getenv("STORE_PASSWORD")
      keyAlias = "upload"
      keyPassword = System.getenv("KEY_PASSWORD")
    }
    // A checked-out clone has no debug.keystore -- it is deliberately gitignored -- so this is
    // configured only where the file actually exists. Where it does not, AGP's own generated debug
    // key signs the build, which is all CI needs: signing continuity matters on the device that
    // gets upgraded in place, not on a runner that builds once and throws the APK away.
    create("debugConfig") {
      val local = file("${rootDir}/debug.keystore")
      if (local.exists()) {
        storeFile = local
        storePassword = "android"
        keyAlias = "androiddebugkey"
        keyPassword = "android"
      }
    }
  }

  buildTypes {
    release {
      isCrunchPngs = false
      isMinifyEnabled = true
      proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
      signingConfig = signingConfigs.getByName("release")
    }
    debug {
      // Instrumentation owns and may uninstall its target package. Keep every debug target under
      // a disposable identity so `connected*AndroidTest` can never remove the installed release
      // app — and with it the user's private runtime, projects, agent credentials and sessions.
      applicationIdSuffix = ".debug"
      versionNameSuffix = "-debug"
      if (file("${rootDir}/debug.keystore").exists()) {
        signingConfig = signingConfigs.getByName("debugConfig")
      }
    }

    // The build that goes on a real phone for real use.
    //
    // Not debuggable, so `run-as` cannot read the userland -- which is where Claude's and Codex's
    // credentials live, and the one realistic way they leak off a device whose owner has USB
    // debugging on. Signed with the same key as `debug` on purpose: an install over an existing
    // Verb keeps the working world, and a signature change is exactly what forces the uninstall
    // that destroys it. One key, one application id, always `install -r`.
    //
    // Minification stays off: this is a build for using and reporting bugs against, and a
    // stack trace that names real classes is worth more here than a smaller APK.
    create("device") {
      // Do not init from `debug`: its `.debug` application-id suffix is deliberately disposable.
      // A device build retains the canonical application id and is installed only with `-r` and
      // a signing key matching the app already on the phone.
      isDebuggable = false
      isMinifyEnabled = false
      isJniDebuggable = false
      if (file("${rootDir}/debug.keystore").exists()) {
        signingConfig = signingConfigs.getByName("debugConfig")
      }
      matchingFallbacks += listOf("debug")
    }
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
    // minSdk is 24; java.time (used by VerbSession, see docs/VERB_SESSION_CONTRACT.md) needs API
    // 26 without this.
    isCoreLibraryDesugaringEnabled = true
  }
  buildFeatures {
    compose = true
    buildConfig = true
  }
  testOptions { unitTests { isIncludeAndroidResources = true } }
  dependenciesInfo {
    includeInApk = false
    includeInBundle = true
  }
}

// Configure the Secrets Gradle Plugin to use .env and .env.example files
// to match the convention used in Web projects.
secrets {
  propertiesFileName = ".env"
  defaultPropertiesFileName = ".env.example"
  ignoreList.add("FIREBASE_APPCHECK_DEBUG_TOKEN")
}

dependencies {
  coreLibraryDesugaring(libs.desugar.jdk.libs)
  implementation(platform(libs.androidx.compose.bom))
  // implementation(libs.accompanist.permissions)
  implementation(libs.androidx.activity.compose)
  // implementation(libs.androidx.camera.camera2)
  // implementation(libs.androidx.camera.core)
  // implementation(libs.androidx.camera.lifecycle)
  // implementation(libs.androidx.camera.view)
  implementation(libs.androidx.compose.material.icons.core)
  implementation(libs.androidx.compose.material.icons.extended)
  implementation(libs.androidx.compose.material3)
  implementation(libs.androidx.compose.ui)
  implementation(libs.androidx.compose.ui.graphics)
  implementation(libs.androidx.compose.ui.tooling.preview)
  implementation(libs.androidx.core.ktx)
  // implementation(libs.androidx.datastore.preferences)
  implementation(libs.androidx.lifecycle.runtime.compose)
  implementation(libs.androidx.lifecycle.runtime.ktx)
  implementation(libs.androidx.lifecycle.viewmodel.compose)
  // implementation(libs.androidx.navigation.compose)
  // implementation(libs.coil.compose)
  implementation(libs.converter.moshi)
  implementation(libs.kotlinx.coroutines.android)
  implementation(libs.kotlinx.coroutines.core)
  implementation(libs.logging.interceptor)
  implementation(libs.moshi.kotlin)
  implementation(libs.okhttp)
  // implementation(libs.play.services.location)
  implementation(libs.retrofit)
  testImplementation(libs.androidx.compose.ui.test.junit4)
  testImplementation(libs.androidx.core)
  testImplementation(libs.androidx.junit)
  testImplementation(libs.junit)
  testImplementation(libs.kotlinx.coroutines.test)
  testImplementation(libs.okhttp.mockwebserver)
  testImplementation(libs.robolectric)
  testImplementation(libs.roborazzi)
  testImplementation(libs.roborazzi.compose)
  testImplementation(libs.roborazzi.junit.rule)
  androidTestImplementation(platform(libs.androidx.compose.bom))
  androidTestImplementation(libs.androidx.compose.ui.test.junit4)
  androidTestImplementation(libs.androidx.espresso.core)
  androidTestImplementation(libs.androidx.junit)
  androidTestImplementation(libs.androidx.runner)
  debugImplementation(libs.androidx.compose.ui.test.manifest)
  debugImplementation(libs.androidx.compose.ui.tooling)
  "ksp"(libs.moshi.kotlin.codegen)
}

// A connected instrumentation deployment may uninstall its target package during cleanup. Debug
// already has a disposable application id, but a real phone is still never an acceptable Gradle
// test target: it contains work that no test owns. Keep this as an executable invariant instead of
// relying on a warning in documentation. It is a separate dependency so the check can be exercised
// without invoking instrumentation at all.
val enforceEmulatorOnlyConnectedTests = tasks.register<Exec>("enforceEmulatorOnlyConnectedTests") {
  group = "verification"
  description = "Refuses connected instrumentation while a physical Android device is attached."
  commandLine(
    "sh",
    rootProject.file("scripts/enforce-emulator-only-connected-tests.sh").absolutePath,
    androidComponents.sdkComponents.adb.get().asFile.absolutePath
  )
}

tasks.matching { it.name.startsWith("connected") && it.name.endsWith("AndroidTest") }.configureEach {
  dependsOn(enforceEmulatorOnlyConnectedTests)
}
