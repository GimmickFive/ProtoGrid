import org.gradle.api.GradleException
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.io.File

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val versionNameEnv = System.getenv("VERSION_NAME")?.takeIf { it.isNotBlank() } ?: "0.4.11"
val versionCodeEnv = System.getenv("VERSION_CODE")?.toIntOrNull() ?: 1

// Release builds are arm64-only. For an x86_64 emulator run:
//   PROTOGRID_ABIS=x86_64 ./gradlew :app:assembleStandardRelease :app:assembleLiteRelease
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
// CI sets this for tag builds: a published APK must never silently fall back
// to the debug certificate, otherwise the next build gets a different key and
// in-place updates stop working.
val requireReleaseKey = System.getenv("PROTOGRID_REQUIRE_RELEASE_KEY") == "1"

android {
    namespace = "app.protogrid.client"
    compileSdk = 35

    // Two cores, two APKs: `standard` links the full ProtoGrid-core, `lite`
    // links the same fork built with `protogrid_lite` (VLESS/Reality over TCP
    // only). The lite build gets its own applicationId so both can be
    // installed side by side and update independently.
    flavorDimensions += "core"

    productFlavors {
        create("standard") {
            dimension = "core"
            buildConfigField("boolean", "LITE_CORE", "false")
        }
        create("lite") {
            dimension = "core"
            applicationIdSuffix = ".lite"
            versionNameSuffix = "-lite"
            buildConfigField("boolean", "LITE_CORE", "true")
        }
    }

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
            if (releaseStorePassword.isNullOrBlank() ||
                releaseKeyAlias.isNullOrBlank() ||
                releaseKeyPassword.isNullOrBlank()
            ) {
                throw GradleException(
                    "PROTOGRID_KEYSTORE is set, but the password, alias or key " +
                        "password is missing",
                )
            }
            create("release") {
                storeFile = file(releaseStore)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        } else if (requireReleaseKey) {
            throw GradleException(
                "PROTOGRID_KEYSTORE must point to an existing release keystore " +
                    "when PROTOGRID_REQUIRE_RELEASE_KEY=1",
            )
        } else {
            project.logger.warn(
                "PROTOGRID_KEYSTORE is not set; the APK will be signed with the " +
                    "debug key and cannot be published as a stable release",
            )
        }
    }

    buildTypes {
        debug {
            // Installable next to the release APK, so UI fixes can be tested
            // over adb without uninstalling the user's profile.
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
            resValue("string", "app_name", "ProtoGrid Debug")
        }
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
    // The AAR is selected by flavor: the release script/CI builds both from
    // the same patched sing-box tree before Gradle runs.
    add("standardImplementation", files("libs/libbox.aar"))
    add("liteImplementation", files("libs/libbox-lite.aar"))

    implementation("androidx.core:core-ktx:1.15.0")
    // PackageManagerCompat.getUnusedAppRestrictionsStatus returns ListenableFuture.
    implementation("com.google.guava:listenablefuture:1.0")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")

    // QR: camera scanning through ZXing's capture activity and offline image
    // decoding through the same core library.
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
    implementation("com.google.zxing:core:3.5.3")

    implementation(platform("androidx.compose:compose-bom:2025.04.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")

    debugImplementation("androidx.compose.ui:ui-tooling")
}
