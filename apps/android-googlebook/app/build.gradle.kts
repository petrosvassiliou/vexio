import org.gradle.api.tasks.Exec
import java.io.File

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

android {
    namespace = "com.vexio"
    compileSdk = 36
    ndkVersion = "28.0.13004108"

    defaultConfig {
        applicationId = "com.vexio.googlebook"
        minSdk = 29
        targetSdk = 36
        versionCode = 93
        versionName = "0.9.3-beta"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            // 100% Native Execution across ARM64 tablets/convertibles and x86_64 Googlebook workstations
            abiFilters += listOf("arm64-v8a", "x86_64")
        }

        vectorDrawables {
            useSupportLibrary = true
        }
    }

    testOptions {
        unitTests {
            isReturnDefaultValues = true
        }
    }

    signingConfigs {
        create("release") {
            val ksPath = System.getenv("ANDROID_KEYSTORE_PATH")?.trim().orEmpty()
            val ksFile = if (ksPath.isNotEmpty()) File(ksPath) else null
            if (ksFile != null && ksFile.exists()) {
                storeFile = ksFile
                storeType = "PKCS12"
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS") ?: "vexio-release"
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD") ?: System.getenv("ANDROID_KEYSTORE_PASSWORD")
                enableV1Signing = true
                enableV2Signing = true
                enableV3Signing = true
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            val relSigning = signingConfigs.getByName("release")
            signingConfig = if (relSigning.storeFile != null) relSigning else signingConfigs.getByName("debug")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isMinifyEnabled = false
            isJniDebuggable = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    kotlinOptions {
        jvmTarget = "21"
        freeCompilerArgs += listOf(
            "-opt-in=androidx.compose.material3.ExperimentalMaterial3Api",
            "-opt-in=androidx.compose.material3.ExperimentalMaterial3ExpressiveApi",
            "-opt-in=androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi",
            "-opt-in=androidx.compose.foundation.ExperimentalFoundationApi"
        )
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
            keepDebugSymbols += listOf("**/libvexio_jni.so")
        }
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

val rustWorkspaceDir = File(rootProject.projectDir, "../../").canonicalFile
val jniOutputDir = File(projectDir, "src/main/jniLibs")

val cargoBuildVexioJni by tasks.registering(Exec::class) {
    group = "rust-ndk"
    description = "Compiles crates/vexio-core into arm64-v8a and x86_64 libvexio_jni.so via cargo-ndk"
    workingDir = rustWorkspaceDir
    inputs.dir(File(rustWorkspaceDir, "crates"))
    inputs.file(File(rustWorkspaceDir, "Cargo.toml"))
    outputs.dir(jniOutputDir)

    commandLine(
        "cargo",
        "ndk",
        "--platform", "29",
        "-t", "arm64-v8a",
        "-t", "x86_64",
        "-o", jniOutputDir.absolutePath,
        "build",
        "--release",
        "-p", "vexio-core"
    )
    onlyIf {
        System.getenv("SKIP_CARGO_NDK") != "1"
    }
}

tasks.matching { it.name == "mergeDebugJniLibFolders" || it.name == "mergeReleaseJniLibFolders" }
    .configureEach {
        dependsOn(cargoBuildVexioJni)
    }

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.05.00")
    implementation(composeBom)
    androidTestImplementation(composeBom)

    // Core Jetpack Compose UI & Foundation (DragAndDrop, PointerIcon, Hardware KeyEvents)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material:material-icons-extended")

    // Material 3 Expressive (Expressive Motion Springs, Morphing Floating/Docked Toolbars, Rich Tokens)
    implementation("androidx.compose.material3:material3:1.4.0-alpha14")
    implementation("androidx.compose.material3:material3-window-size-class:1.4.0-alpha14")
    implementation("androidx.compose.material3:material3-adaptive-navigation-suite:1.4.0-alpha14")

    // Compose Adaptive Multi-Pane Scaffolds & Navigation 3
    implementation("androidx.compose.material3.adaptive:adaptive:1.2.0-alpha06")
    implementation("androidx.compose.material3.adaptive:adaptive-layout:1.2.0-alpha06")
    implementation("androidx.compose.material3.adaptive:adaptive-navigation:1.2.0-alpha06")
    implementation("androidx.navigation3:navigation3-runtime:1.0.0-alpha01")
    implementation("androidx.navigation3:navigation3-ui:1.0.0-alpha01")

    // USI / Active Stylus MotionPrediction & Low-Latency Front-Buffered / Vulkan Graphics
    implementation("androidx.input:input-motionprediction:1.0.0-beta05")
    implementation("androidx.graphics:graphics-core:1.0.2")
    implementation("androidx.ink:ink-authoring:1.0.0-alpha04")
    implementation("androidx.ink:ink-strokes:1.0.0-alpha04")

    // Freeform Desktop Windowing, Lifecycle, SAF File I/O & Native OS Printing
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.9.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.0")
    implementation("androidx.window:window:1.4.0")
    implementation("androidx.documentfile:documentfile:1.1.0")
    implementation("androidx.print:print:1.1.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.8.0")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlin:kotlin-test:2.1.20")
    debugImplementation("androidx.compose.ui:ui-tooling")
}
