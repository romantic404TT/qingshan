// 本项目 Gradle 工程，独立于水杉的 Tauri 合包工程。只依赖 AndroidX 与 Material。
// 版本与水杉上游锁定值一致，以便同步上游时 diff 最小。
pluginManagement {
    repositories { google(); mavenCentral() }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "qingshan-android-host"
include(":app")
