// SPDX-License-Identifier: GPL-3.0-or-later
plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

fun releaseSecret(name: String): String? =
    (System.getenv(name) ?: project.findProperty(name) as String?)?.takeIf { it.isNotEmpty() }

val releaseKeystore: String? = releaseSecret("COMETCAST_KEYSTORE")
val releaseSigningConfigured: Boolean =
    releaseKeystore != null &&
        releaseSecret("COMETCAST_KEYSTORE_PASSWORD") != null &&
        releaseSecret("COMETCAST_KEY_ALIAS") != null

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

    // Release signing comes from the environment (or Gradle properties of the person building,
    // for example ~/.gradle/gradle.properties); nothing secret is in the repository:
    //   COMETCAST_KEYSTORE            path of the keystore file
    //   COMETCAST_KEYSTORE_PASSWORD   store password
    //   COMETCAST_KEY_ALIAS           key alias
    //   COMETCAST_KEY_PASSWORD        key password (defaults to the store password)
    // Without all of the first three the release build type uses the debug key and the guard
    // task `checkReleaseSigning` (end of this file) refuses unless it is allowed explicitly.
    signingConfigs {
        if (releaseKeystore != null) {
            create("release") {
                storeFile = file(releaseKeystore)
                storePassword = releaseSecret("COMETCAST_KEYSTORE_PASSWORD")
                keyAlias = releaseSecret("COMETCAST_KEY_ALIAS")
                keyPassword = releaseSecret("COMETCAST_KEY_PASSWORD") ?: releaseSecret("COMETCAST_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            signingConfig =
                if (releaseSigningConfigured) {
                    signingConfigs.getByName("release")
                } else {
                    signingConfigs.getByName("debug")
                }
        }
    }

    packaging {
        jniLibs {
            // The PDF and Office workers are executables shipped as lib*.so (see the worker
            // tasks below). They can only be started from a real file, so the libraries must be
            // extracted to nativeLibraryDir at install time (android:extractNativeLibs=true).
            // Costs install size (the libraries exist twice: in the APK and on disk).
            useLegacyPackaging = true
        }
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }
}

dependencies {
    // The system QR scanner, a part of Google Play services (no camera permission in the app):
    // GmsBarcodeScanning, native-apps.md section 8. Proprietary service, Google APIs terms; see
    // THIRD_PARTY_NOTICES.md. Pinned.
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
    // The Google Play install referrer (Apache-2.0): the code and server of the link that sent
    // the person to the store, native-apps.md 20.4. Pinned.
    implementation("com.android.installreferrer:installreferrer:2.2")
    // Plain JVM unit tests of the pure Kotlin helpers (`./gradlew testDebugUnitTest`).
    testImplementation("junit:junit:4.13.2")
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
// PDF and Office workers. cc-docs-worker and cc-office-worker are separate *programs* (resource
// limited child processes). Android cannot start a binary from the app's data directory (since
// Android 10 exec from there is blocked for apps that target API 29+), but it can start a file in
// the app's nativeLibraryDir. So each worker is built as an ordinary executable for Android with
// cargo-ndk, copied into jniLibs as lib<name>.so (the package manager only extracts files called
// lib*.so), and extracted at install time because useLegacyPackaging is on. Dart finds the folder
// through the platform channel (`nativeLibraryDir`) and gives the paths to the Rust core
// (`set_docs_worker`, `set_office_worker`). See docs/android.md, "PDF and Office on Android".
//
// Each worker is built in its own cargo invocation: building them together would unify their
// features and put the Typst engine into the PDF worker (same reason as in linux/CMakeLists.txt).
// ---------------------------------------------------------------------------------------------
val ndkTriples =
    mapOf(
        "arm64-v8a" to "aarch64-linux-android",
        "armeabi-v7a" to "armv7-linux-androideabi",
        "x86_64" to "x86_64-linux-android",
    )
val rustWorkersRoot = layout.buildDirectory.dir("rust-workers")

fun registerWorkers(
    buildType: String,
    release: Boolean,
) {
    val cap = buildType.replaceFirstChar { it.uppercase() }
    val outDir = rustWorkersRoot.get().dir(buildType).asFile
    val coreTask = tasks.named("cargoNdk$cap")
    val profileDir = if (release) "release" else "debug"
    val workerTasks =
        listOf("cc-docs" to "cc-docs-worker", "cc-office" to "cc-office-worker").map { (crate, bin) ->
            val so = "lib" + bin.replace('-', '_') + ".so"
            tasks.register<Exec>("cargoNdkWorker$cap${bin.replace("-", "")}") {
                group = "cometcast"
                description = "Builds $bin ($buildType) for $rustAbis as $so"
                workingDir = workspaceRoot
                dependsOn(coreTask) // the core task wipes its own output directory first
                inputs.files(fileTree(workspaceRoot.resolve("crates")) { include("**/*.rs", "**/Cargo.toml") })
                inputs.files(workspaceRoot.resolve("Cargo.toml"), workspaceRoot.resolve("Cargo.lock"))
                inputs.property("abis", rustAbis.joinToString(","))
                outputs.dir(outDir)
                doFirst {
                    val cargoBin = System.getProperty("user.home") + "/.cargo/bin"
                    val oldPath = System.getenv("PATH")
                    environment("PATH", if (oldPath.isNullOrEmpty()) cargoBin else cargoBin + File.pathSeparator + oldPath)
                    environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)
                    environment("CARGO_PROFILE_DEV_DEBUG", "0")
                }
                val args = mutableListOf("cargo", "ndk", "--platform", flutter.minSdkVersion.toString())
                rustAbis.forEach { args += listOf("-t", it) }
                args += listOf("build", "-p", crate, "--bin", bin, "--target-dir", cargoTargetDir.get().asFile.absolutePath)
                if (release) args += "--release"
                commandLine(args)
                doLast {
                    rustAbis.forEach { abi ->
                        val triple = ndkTriples.getValue(abi)
                        val from = cargoTargetDir.get().asFile.resolve("$triple/$profileDir/$bin")
                        val to = File(outDir, "$abi/$so")
                        to.parentFile.mkdirs()
                        from.copyTo(to, overwrite = true)
                    }
                }
            }
        }
    android.sourceSets.getByName(buildType).jniLibs.srcDir(outDir)
    // Workers are cleaned together with the first one of a build so an ABI that is no longer
    // built cannot stay behind.
    workerTasks.first().configure { doFirst { outDir.deleteRecursively() } }
    workerTasks.drop(1).forEach { t -> t.configure { mustRunAfter(workerTasks.first()) } }
    tasks
        .matching {
            it.name.startsWith("merge") &&
                it.name.contains(buildType, ignoreCase = true) &&
                (it.name.endsWith("JniLibFolders") || it.name.endsWith("NativeLibs"))
        }.configureEach { dependsOn(workerTasks) }
}

registerWorkers("debug", release = false)
registerWorkers("profile", release = true)
registerWorkers("release", release = true)

// ---------------------------------------------------------------------------------------------
// Release signing guard. With the keystore from the environment (COMETCAST_KEYSTORE,
// COMETCAST_KEYSTORE_PASSWORD, COMETCAST_KEY_ALIAS, optional COMETCAST_KEY_PASSWORD) the release
// build type is signed with it. Without it the build type falls back to the debug key, and then
// building a release fails unless cometcastAllowDebugSigning=true is set:
// `-PcometcastAllowDebugSigning=true` for Gradle, or
// `ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true` in the environment for
// `flutter build apk --release` (flutter does not forward -P options). A release APK built that
// way must not be distributed. See docs/android.md.
// ---------------------------------------------------------------------------------------------
val checkReleaseSigning =
    tasks.register("checkReleaseSigning") {
        group = "cometcast"
        description = "Fails release builds that would be signed with the debug key unless allowed."
        val allowed = project.findProperty("cometcastAllowDebugSigning") == "true"
        val configured = releaseSigningConfigured
        doLast {
            if (configured) return@doLast
            if (!allowed) {
                throw GradleException(
                    "The release build would be signed with the DEBUG key (no release keystore is " +
                        "configured: set COMETCAST_KEYSTORE, COMETCAST_KEYSTORE_PASSWORD and " +
                        "COMETCAST_KEY_ALIAS). Or pass -PcometcastAllowDebugSigning=true (or set " +
                        "ORG_GRADLE_PROJECT_cometcastAllowDebugSigning=true) for a local test " +
                        "build; never distribute the result. See docs/android.md.",
                )
            }
            logger.warn("WARNING: the release APK is signed with the DEBUG key. Do not distribute it.")
        }
    }
tasks.matching { it.name == "preReleaseBuild" }.configureEach { dependsOn(checkReleaseSigning) }
