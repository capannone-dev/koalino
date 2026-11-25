import java.util.Properties

plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "io.github.capannone_dev.koalino"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // The permanent ID (#168). Android IDs allow no hyphens, so the GitHub owner
        // `capannone-dev` is written with an underscore; macOS uses the hyphen.
        applicationId = "io.github.capannone_dev.koalino"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    // The Whisper model staged by tools/cross/fetch-model.sh ships as an APK asset (Issue #45:
    // the app must transcribe out of the box). It is stored uncompressed so WhisperModel.kt can
    // read its length and copy it quickly; it compresses poorly anyway.
    sourceSets["main"].assets.srcDir("../../native/android-assets")
    androidResources {
        noCompress += listOf("bin", "onnx")
    }

    // The release key (#168) is the owner's. A properties file outside the repository (so it is
    // never committed or copied with the checkout) names the keystore and its passwords:
    // `$KOALINO_SIGNING_PROPERTIES`, else `~/.android/koalino-key.properties`. Without it, release
    // builds are signed with the debug key, so any checkout still builds (F-Droid signs its own).
    val releaseKeyFile = File(
        System.getenv("KOALINO_SIGNING_PROPERTIES")
            ?: "${System.getProperty("user.home")}/.android/koalino-key.properties",
    )
    val releaseKey = Properties().apply {
        if (releaseKeyFile.exists()) releaseKeyFile.inputStream().use { load(it) }
    }
    signingConfigs {
        if (releaseKeyFile.exists()) {
            create("release") {
                storeFile = file(releaseKey.getProperty("storeFile"))
                storePassword = releaseKey.getProperty("storePassword")
                keyAlias = releaseKey.getProperty("keyAlias")
                keyPassword = releaseKey.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            signingConfig = if (releaseKeyFile.exists()) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
