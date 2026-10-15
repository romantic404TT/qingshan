plugins {
    // 本项目 AGP 固定 8.13.2，与 host/gradle-app/gradle/wrapper 里 Gradle 8.14.3 匹配。
    // AGP 9.x 要求 Gradle 9.6 以上，本项目的 wrapper 是 8.14.3，升 AGP 会失败。
    id("com.android.application") version "8.13.2" apply false
}
