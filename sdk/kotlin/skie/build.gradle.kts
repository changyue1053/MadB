plugins {
    kotlin("multiplatform") version "2.4.20"
    id("co.touchlab.skie") version "0.10.15"
}
val profileDigest = providers.gradleProperty("madbProfileDigest")
val interfaceDigest = providers.gradleProperty("madbInterfaceDigest")
val generateIdentity = tasks.register("generateIdentity") {
    val output = layout.buildDirectory.file("generated/identity/BuildIdentity.kt")
    inputs.property("profile", profileDigest)
    inputs.property("interface", interfaceDigest)
    outputs.file(output)
    doLast {
        val profile = profileDigest.get()
        val api = interfaceDigest.get()
        require(profile.matches(Regex("[a-f0-9]{64}"))) { "madbProfileDigest must be a resolved profile SHA256" }
        require(api.matches(Regex("[a-f0-9]{64}"))) { "madbInterfaceDigest must be an interface SHA256" }
        output.get().asFile.apply { parentFile.mkdirs(); writeText("package madb.skie\ninternal object BuildIdentity { const val PROFILE = \"sha256:$profile\"; const val INTERFACE = \"sha256:$api\" }\n") }
    }
}
kotlin {
    macosArm64 { binaries.framework { baseName = "MadBSkie"; isStatic = true; binaryOption("bundleId", "dev.madb.skie"); freeCompilerArgs += "-Xdebug-prefix-map=${projectDir}=."; freeCompilerArgs += "-Xoverride-konan-properties=minVersion.macos=14.0;airplaneMode=true" } }
    sourceSets.commonMain.dependencies { implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.11.0") }
    sourceSets.getByName("macosArm64Main").kotlin.srcDir(layout.buildDirectory.dir("generated/identity"))
    compilerOptions { languageVersion.set(org.jetbrains.kotlin.gradle.dsl.KotlinVersion.KOTLIN_2_2); apiVersion.set(org.jetbrains.kotlin.gradle.dsl.KotlinVersion.KOTLIN_2_2) }
}
tasks.matching { it.name.startsWith("compileKotlin") }.configureEach { dependsOn(generateIdentity) }
skie { build { produceDistributableFramework(); freeSwiftCompilerArgs.addAll("-file-prefix-map", "${projectDir}=.", "-prefix-serialized-debugging-options", "-Xfrontend", "-disable-incremental-llvm-codegen") }; analytics { enabled.set(false) } }
dependencyLocking { lockAllConfigurations() }
