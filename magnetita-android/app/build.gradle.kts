import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// The author's signing key lives outside the repository: keystore.properties
// at the project root (gitignored) names it. Without it the release build is
// signed with the debug key, so the artifact always exists under one name.
val keystoreProperties = rootProject.file("keystore.properties").takeIf { it.isFile }?.let { file ->
    Properties().apply { file.inputStream().use { load(it) } }
}

// The Rust core: cross-compiled and bound by scripts/build-native.sh into
// build outputs Gradle then packs. Nothing generated is committed.
val nativeOut = layout.buildDirectory.dir("native")
val buildNative by tasks.registering(Exec::class) {
    description = "Cross-compiles magnetita-mobile for arm64 and generates its Kotlin bindings."
    workingDir = rootProject.projectDir
    inputs.dir(rootProject.file("../celestina-rs/crates/magnetita-mobile/src"))
    inputs.dir(rootProject.file("../celestina-rs/crates/magnetita-link/src"))
    inputs.dir(rootProject.file("../celestina-rs/crates/magnetita-proto/src"))
    outputs.dir(nativeOut)
    commandLine("sh", "scripts/build-native.sh", nativeOut.get().asFile.absolutePath)
}

android {
    namespace = "org.celestina.magnetita"
    compileSdk {
        version = release(36)
    }

    defaultConfig {
        applicationId = "org.celestina.magnetita"
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    signingConfigs {
        if (keystoreProperties != null) {
            create("author") {
                storeFile = rootProject.file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            optimization {
                enable = false
            }
            signingConfig = if (keystoreProperties != null) signingConfigs.getByName("author") else signingConfigs.getByName("debug")
        }
    }

    lint {
        // The generated UniFFI bindings choose java.lang.ref.Cleaner behind a
        // runtime check lint cannot follow; lint.xml scopes that finding to them.
        lintConfig = file("lint.xml")
        abortOnError = true
        warningsAsErrors = false
        checkReleaseBuilds = true
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
    }
}

tasks.named("preBuild") { dependsOn(buildNative) }

// The generated bindings and the native library, registered the way AGP 9
// wants generated sources registered: through the variant's source sets.
androidComponents {
    onVariants { variant ->
        variant.sources.kotlin?.addStaticSourceDirectory("build/native/kotlin")
        variant.sources.jniLibs?.addStaticSourceDirectory("build/native/jniLibs")
    }
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.material3)
    // Every Material icon, in every style, as MilaHub carries them.
    implementation(libs.androidx.compose.material.icons.extended)
    // The glass of the tab pill: what scrolls beneath it is blurred through it.
    implementation(libs.haze)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    // UniFFI bindings call the native library through JNA.
    implementation(libs.jna) { artifact { type = "aar" } }
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.androidx.camera.mlkit.vision)
    implementation(libs.mlkit.barcode.scanning)
    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    debugImplementation(libs.androidx.compose.ui.tooling)
}
