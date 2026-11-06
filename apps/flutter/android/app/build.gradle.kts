// SPDX-License-Identifier: GPL-3.0-or-later
plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "de.cometcast.cometcast"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "de.cometcast.cometcast"
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // TODO: Add your own signing config for the release build.
            // Signed with the DEBUG key for now. That is fine for local tests only, never for
            // distribution, so the build refuses unless it is allowed explicitly (see the
            // checkReleaseSigning task at the end of this file).
            signingConfig = signingConfigs.getByName("debug")
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

// ---------------------------------------------------------------------------------------------
// Rust core (libcc_ffi.so) for Android, built with cargo-ndk. See docs/android.md.
//
// Not cargokit: the crate lives in the repository's Cargo workspace (crates/cc-ffi), not in a
// Flutter plugin folder, and a short explicit Gradle task is easier to read and to debug (same
// reasoning as the CMake step in linux/CMakeLists.txt).
//
// One task per Flutter build type writes build/app/rust-jniLibs/<type>/<abi>/libcc_ffi.so, and
// that folder is registered as the jniLibs source set of that build type.
// ---------------------------------------------------------------------------------------------

// Flutter passes the requested ABIs as the Gradle property "target-platform"
// (for example "android-arm64" for `flutter build apk --target-platform android-arm64`).
// Without it Flutter builds android-arm, android-arm64 and android-x64.
// -PcometcastRustAbis=arm64-v8a,armeabi-v7a overrides the mapping (cargo-ndk ABI names).
val flutterToNdkAbi =
    mapOf(
        "android-arm" to "armeabi-v7a",
        "android-arm64" to "arm64-v8a",
        "android-x64" to "x86_64",
    )
val rustAbis: List<String> =
    (
        (project.findProperty("cometcastRustAbis") as String?)
            ?: (project.findProperty("target-platform") as String?)
                ?.split(",")
                ?.mapNotNull { flutterToNdkAbi[it.trim()] }
                ?.joinToString(",")
            ?: "arm64-v8a,armeabi-v7a,x86_64"
    ).split(",").map { it.trim() }.filter { it.isNotEmpty() }

// android/ -> apps/flutter -> apps -> repository root (the Cargo workspace).
val workspaceRoot: File = rootProject.projectDir.resolve("../../..").canonicalFile
val rustJniRoot = layout.buildDirectory.dir("rust-jniLibs")
val cargoTargetDir = layout.buildDirectory.dir("cargo-target-android")

fun registerCargoNdk(
    buildType: String,
    release: Boolean,
) {
    val taskName = "cargoNdk" + buildType.replaceFirstChar { it.uppercase() }
    val outDir = rustJniRoot.get().dir(buildType)
    val cargoTask =
        tasks.register<Exec>(taskName) {
            group = "cometcast"
            description = "Builds libcc_ffi.so ($buildType) for $rustAbis with cargo-ndk"
            workingDir = workspaceRoot
            // Inputs/outputs let Gradle skip the task; cargo does its own incremental build too.
            inputs.files(
                fileTree(workspaceRoot.resolve("crates")) {
                    include("**/*.rs", "**/Cargo.toml")
                },
            )
            inputs.files(workspaceRoot.resolve("Cargo.toml"), workspaceRoot.resolve("Cargo.lock"))
            inputs.property("abis", rustAbis.joinToString(","))
            outputs.dir(outDir)
            doFirst {
                // Stale ABIs from an earlier build must not end up in the APK.
                outDir.asFile.deleteRecursively()
                val cargoBin = System.getProperty("user.home") + "/.cargo/bin"
                val oldPath = System.getenv("PATH")
                environment("PATH", if (oldPath.isNullOrEmpty()) cargoBin else cargoBin + File.pathSeparator + oldPath)
                // cargo-ndk reads the NDK location from here: the NDK version AGP uses.
                environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)
                // Without debug info the debug .so is much smaller (backtraces keep function
                // names through the symbol table).
                environment("CARGO_PROFILE_DEV_DEBUG", "0")
            }
            val args = mutableListOf("cargo", "ndk", "--platform", flutter.minSdkVersion.toString())
            rustAbis.forEach { args += listOf("-t", it) }
            args += listOf("-o", outDir.asFile.absolutePath)
            args += listOf("build", "-p", "cc-ffi", "--target-dir", cargoTargetDir.get().asFile.absolutePath)
            if (release) args += "--release"
            commandLine(args)
        }
    // AGP rejects Provider instances in source sets and flattens file collections to plain
    // directories, so a task dependency cannot be attached to the source set itself (tried:
    // srcDir(provider), srcDir(files(dir).builtBy(task)); neither ran the task). Instead the
    // tasks that read the directory depend on it explicitly:
    android.sourceSets.getByName(buildType).jniLibs.srcDir(outDir.asFile)
    tasks
        .matching {
            it.name.startsWith("merge") &&
                it.name.contains(buildType, ignoreCase = true) &&
                (it.name.endsWith("JniLibFolders") || it.name.endsWith("NativeLibs"))
        }.configureEach { dependsOn(cargoTask) }
}

registerCargoNdk("debug", release = false)
registerCargoNdk("profile", release = true)
registerCargoNdk("release", release = true)

// ---------------------------------------------------------------------------------------------
// Release signing guard. The release build type is signed with the debug key (no keystore is
// configured yet). Building a release therefore fails unless the property
// cometcastAllowDebugSigning=true is set: `-PcometcastAllowDebugSigning=true` for Gradle, or
// `ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true` in the environment for
// `flutter build apk --release` (flutter does not forward -P options). A release APK built this
// way must not be distributed. See docs/android.md.
// ---------------------------------------------------------------------------------------------
val checkReleaseSigning =
    tasks.register("checkReleaseSigning") {
        group = "cometcast"
        description = "Fails release builds that would be signed with the debug key unless allowed."
        val allowed = project.findProperty("cometcastAllowDebugSigning") == "true"
        doLast {
            if (!allowed) {
                throw GradleException(
                    "The release build is signed with the DEBUG key (no release keystore is " +
                        "configured). Pass -PcometcastAllowDebugSigning=true (or set " +
                        "ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true) for a local test " +
                        "build; never distribute the result. See docs/android.md.",
                )
            }
            logger.warn("WARNING: the release APK is signed with the DEBUG key. Do not distribute it.")
        }
    }
tasks.matching { it.name == "preReleaseBuild" }.configureEach { dependsOn(checkReleaseSigning) }
