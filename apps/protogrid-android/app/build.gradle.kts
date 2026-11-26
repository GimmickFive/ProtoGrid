import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.io.File

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val versionNameEnv = System.getenv("VERSION_NAME")?.takeIf { it.isNotBlank() } ?: "0.2.0"
val versionCodeEnv = System.getenv("VERSION_CODE")?.toIntOrNull() ?: 1

// Release builds are arm64-only. For an x86_64 emulator run:
//   PROTOGRID_ABIS=x86_64 ./gradlew :app:assembleRelease
val abis = (System.getenv("PROTOGRID_ABIS") ?: "arm64-v8a")
    .split(',')
    .map(String::trim)
    .filter(String::isNotEmpty)

// CI decodes the release keystore and exports these variables. Local builds
// without them fall back to the debug key so the APK stays installable.
val releaseStore = System.getenv("PROTOGRID_KEYSTORE")
val releaseStorePassword = System.getenv("PROTOGRID_KEYSTORE_PASSWORD")
val releaseKeyAlias = System.getenv("PROTOGRID_KEY_ALIAS")
val releaseKeyPassword = System.getenv("PROTOGRID_KEY_PASSWORD")

android {
    namespace = "app.protogrid.client"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.protogrid.client"
        minSdk = 26
        targetSdk = 35
        versionCode = versionCodeEnv
        versionName = versionNameEnv
        ndk {
            abiFilters.addAll(abis)
        }
    }

    signingConfigs {
        if (!releaseStore.isNullOrBlank() && File(releaseStore).exists()) {
            create("release") {
                storeFile = file(releaseStore)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release")
                ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    packaging {
        jniLibs {
            // Extracting the Go/Rust libraries keeps exec/mmap behaviour
            // predictable on Android 10+ and 16 KB page devices.
            useLegacyPackaging = true
        }
    }

    dependenciesInfo {
        includeInApk = false
    }

    lint {
        abortOnError = false
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(files("libs/libbox.aar"))

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")

    implementation(platform("androidx.compose:compose-bom:2025.04.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")

    debugImplementation("androidx.compose.ui:ui-tooling")
}
