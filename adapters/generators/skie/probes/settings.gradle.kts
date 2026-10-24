pluginManagement {
    repositories {
        val localRepo = providers.gradleProperty("madb.mavenRepo").orNull
        if (localRepo != null) maven { url = uri(localRepo); metadataSources { gradleMetadata(); mavenPom() } }
        else { gradlePluginPortal(); mavenCentral() }
    }
}
dependencyResolutionManagement {
    repositories {
        val localRepo = providers.gradleProperty("madb.mavenRepo").orNull
        if (localRepo != null) maven { url = uri(localRepo); metadataSources { gradleMetadata(); mavenPom() } }
        else mavenCentral()
    }
}
rootProject.name = "MadBSkie"
