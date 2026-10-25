plugins { kotlin("multiplatform") version "2.4.20"; id("co.touchlab.skie") version "0.10.15" }
kotlin {
    macosArm64 {
        compilations.getByName("main").cinterops.create("custom") { compilerOpts("-F${projectDir}/native") }
        binaries.framework {
            baseName = "MadBSkieGaps"; isStatic = true
            linkerOpts("-F${projectDir}/native", "-framework", "Custom")
            freeCompilerArgs += "-Xoverride-konan-properties=minVersion.macos=14.0;airplaneMode=true"
        }
    }
    sourceSets.commonMain.dependencies { implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.11.0") }
}
skie { build { produceDistributableFramework(); freeSwiftCompilerArgs.addAll("-file-prefix-map", "${projectDir}=.", "-prefix-serialized-debugging-options", "-Xfrontend", "-disable-incremental-llvm-codegen") }; analytics { enabled.set(false) } }
