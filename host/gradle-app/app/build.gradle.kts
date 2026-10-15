plugins { id("com.android.application") }

// 源码留在原地：host/ 就是这个模块的真实家，gradle-app 只是构建工程，不往里复制也不生成。
val hostRoot = rootDir.parentFile

// 版本号取自 host/version.txt，和上游一样支持 -PmsimeVersion 覆盖。
// 当前是青杉自己的 0.1.0（→ versionName 0.1.0 / versionCode 1000），
// 与水杉 platforms/android/version.txt 的 0.3.1 无关；青杉按自己的序列起算。
val releaseVersion = (findProperty("msimeVersion") as String?)?.trim()?.takeIf { it.isNotEmpty() }
    ?: hostRoot.resolve("version.txt").readText().trim()
val releaseVersionParts = Regex("""^(\d{1,3})\.(\d{1,3})\.(\d{1,3})$""").matchEntire(releaseVersion)
    ?.groupValues?.drop(1)?.map { it.toInt() }
    ?: error("Android release version must be major.minor.patch, got '$releaseVersion'")

// 产品版本（edition）机制整体移除：上游用 shared/contracts/editions.json 生成 6 个 productFlavor
// （full/pinyin/wubi/japanese/vietnamese/tibetan），本融合项目只出一个包，固定按 full 构建，
// 因此不再读版本表、不再有 flavor，也不再挂 editions/<id>/res 覆盖目录。
android {
    // namespace 必须是 app.msime.android，不能跟着 applicationId 改：
    // 394 个 .java 全部声明 package app.msime.android，且 AppEdition 反射的类名硬编码为
    // app.msime.android.BuildConfig。applicationId 才是装机标识，改成青杉自己的包名，
    // 使本包能与水杉原版共存而不冲突。
    namespace = "app.msime.android"
    compileSdk = 36
    // 与交叉编译用的 NDK 同一个版本，AGP 用它的 llvm-strip 处理打包进去的 .so。
    ndkVersion = "28.2.13676358"

    defaultConfig {
        // 装机标识用青杉自己的包名，与水杉原版（app.msime.android）区分，可共存不冲突。
        applicationId = "app.qingshan.ime"
        minSdk = 28
        targetSdk = 35
        // Grows with every release, as the platform requires for an update to install: 0.1.0 is 1000, 1.2.3 is 1002003.
        versionCode = releaseVersionParts[0] * 1_000_000 + releaseVersionParts[1] * 1_000 + releaseVersionParts[2]
        versionName = releaseVersion
        // 两条 ABI 与 PRD 一致：arm64-v8a 真机、x86_64 模拟器/ChromeOS。
        // 出包后用 engine/tools/verify_apk.py 核对 lib/<abi>/libmsime_android.so、
        // 每个 .so 的 102 个 JNI 导出与 assets 词库大小。
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }

        // 这 4 个字段的名字和类型都不能动：AppEdition.java 经反射按硬编码类名
        // "app.msime.android.BuildConfig" 读 EDITION / EDITION_INPUT_SCHEMES /
        // EDITION_DEFAULT_SCHEME（String）和 EDITION_TEMPORARY_JAPANESE（getBoolean）。
        // 缺任何一个 → AppEdition.declared() 抛 IllegalStateException，键盘起不来。
        // 且 AppEdition.of() 的非空校验在「id == full」短路之前，所以后两项不能给空串。
        // 取值抄上游 shared/contracts/editions.json 的 full 条目（10 个方案、默认全拼、
        // temporary_japanese = true）。
        buildConfigField("String", "EDITION", "\"full\"")
        buildConfigField("String", "EDITION_INPUT_SCHEMES",
            "\"quanpin,shuangpin,wubi,japanese,korean,cantonese,zhuyin,vietnamese,tibetan,stroke\"")
        buildConfigField("String", "EDITION_DEFAULT_SCHEME", "\"quanpin\"")
        buildConfigField("boolean", "EDITION_TEMPORARY_JAPANESE", "true")
    }

    sourceSets.getByName("main") {
        manifest.srcFile(hostRoot.resolve("AndroidManifest.xml"))
        java.setSrcDirs(listOf(hostRoot.resolve("java")))
        res.setSrcDirs(listOf(hostRoot.resolve("res")))
        // 上游这里指向 build-apk.sh 暂存的 target/android/host-assets；本项目直接用
        // host/assets：desktop-dictionary.lock.json + dictionary/dict.tsv（青简词库）。
        assets.setSrcDirs(listOf(hostRoot.resolve("assets")))
        // 上游指向 build-native.sh 产出的 target/android/jniLibs；本项目用 cargo-ndk 的
        // 输出目录 engine/jniLibs/<abi>/libmsime_android.so。
        jniLibs.setSrcDirs(listOf(hostRoot.resolve("../engine/jniLibs")))
    }

    buildTypes {
        getByName("release") {
            isMinifyEnabled = false
            // 上游由 build-apk.sh 事后 zipalign + apksigner 签名（密钥走 platforms/android/scripts/signing.sh）。
            // 本项目 3.6 先只出 debug APK，签名链留到发布阶段再补。
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        viewBinding = false
        buildConfig = true
    }

    packaging {
        jniLibs.useLegacyPackaging = false
    }

    // lint 在这里只做 API 级别的门禁：宿主调用了高于 minSdk 的 API 时 NewApi 让它失败。
    // javac 按 compileSdk 编译看不出来，D8 也只补齐其中一部分调用；0.2.0 就这样带上了
    // Path.of（API 34），Android 9 到 12 的设备准备词库时全部 NoSuchMethodError。
    lint {
        checkOnly += "NewApi"
        abortOnError = true
    }
}

// 13 个依赖一个都不裁：Java 树真实用到 androidx.appcompat/core/activity/constraintlayout/
// recyclerview/viewpager2/material、ML Kit 手写、markwon、credentials + googleid。
dependencies {
    implementation("androidx.appcompat:appcompat:1.8.0")
    // Held at 1.13.1: androidx.core 1.19 declares a minimum of AGP 9.1, and AGP cannot move here without the Gradle wrapper, which belongs to the Tauri bundle. The other four in this group carry no such floor.
    implementation("androidx.core:core:1.13.1")
    implementation("androidx.activity:activity:1.10.1")
    implementation("androidx.constraintlayout:constraintlayout:2.2.2")
    implementation("androidx.recyclerview:recyclerview:1.4.0")
    implementation("androidx.viewpager2:viewpager2:1.1.0")
    implementation("com.google.android.material:material:1.14.0")
    implementation("com.google.mlkit:digital-ink-recognition:19.0.0")
    // Notice bodies on the 设置 tab are simple Markdown. Core only: no HTML plugin, so raw HTML in a notice is never interpreted, and no image loader.
    implementation("io.noties.markwon:core:4.6.2")
    // Google 登录。Credential Manager 是 Google 现在的官方入口，旧的 GoogleSignInClient 已弃用；googleid 提供「通过 Google 登录」按钮用的 GetSignInWithGoogleOption（不是底部弹出一键登录用的 GetGoogleIdOption，见 GoogleSignInFlow），play-services-auth 那件是它在设备上的实现。googleid 1.2.1 依赖 credentials 1.6.0，androidx.credentials 又是一个版本一致的原子组，所以原来写的 1.3.0 实际都解析成 1.6.0；这里直接写解析出来的版本，免得读的人按 1.3.0 去查行为。
    implementation("androidx.credentials:credentials:1.6.0")
    implementation("androidx.credentials:credentials-play-services-auth:1.6.0")
    implementation("com.google.android.libraries.identity.googleid:googleid:1.2.1")
}
