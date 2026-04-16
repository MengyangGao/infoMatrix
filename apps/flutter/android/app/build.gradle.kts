import java.io.FileInputStream
import java.util.Properties
import org.gradle.api.GradleException

fun Properties.requireSigningProperty(name: String): String {
    return getProperty(name)?.takeIf { it.isNotBlank() }
        ?: throw GradleException("Android release signing requires '$name' in android/key.properties.")
}

plugins {
    id("com.android.application")
    id("kotlin-android")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "com.infomatrix.reader"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    val keystoreProperties = Properties()
    val keystorePropertiesFile = rootProject.file("key.properties")
    val hasReleaseKeystore = keystorePropertiesFile.exists()
    val releaseBuildRequested = gradle.startParameter.taskNames.any {
        it.contains("release", ignoreCase = true)
    }
    val allowDebugReleaseSigning = providers.environmentVariable("INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING")
        .orNull == "1"

    if (releaseBuildRequested && !hasReleaseKeystore && !allowDebugReleaseSigning) {
        throw GradleException(
            "Android release signing requires ${keystorePropertiesFile.path}. " +
                "Copy key.properties.example to key.properties, or set INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING=1 " +
                "for an explicit smoke-only build."
        )
    }

    if (hasReleaseKeystore) {
        FileInputStream(keystorePropertiesFile).use { keystoreProperties.load(it) }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = JavaVersion.VERSION_17.toString()
    }

    defaultConfig {
        applicationId = "com.infomatrix.reader"
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        if (releaseBuildRequested && hasReleaseKeystore) {
            create("release") {
                storeFile = rootProject.file(keystoreProperties.requireSigningProperty("storeFile"))
                storePassword = keystoreProperties.requireSigningProperty("storePassword")
                keyAlias = keystoreProperties.requireSigningProperty("keyAlias")
                keyPassword = keystoreProperties.requireSigningProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            signingConfig = if (releaseBuildRequested && hasReleaseKeystore) {
                signingConfigs.getByName("release")
            } else if (allowDebugReleaseSigning) {
                signingConfigs.getByName("debug")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }
}

flutter {
    source = "../.."
}
