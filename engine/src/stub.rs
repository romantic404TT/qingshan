// 由 tools/gen_stub.py 从 NativeClient.java 解析生成，请勿手改；要改内容就改生成脚本。
//
// 这些是非输入路径的 native 方法占位实现：签名逐位对齐 Java 声明，返回
// {"ok":false,"error":"not_implemented"}，靠水杉 Java 自身的降级分支（fail()）失效。
// 真实现的 17 个在 lib.rs，合计 102 个符号一个不能少。

use jni::objects::JObject;
use jni::sys::{jboolean, jbyteArray, jint, jlong, jshortArray, jobject};
use jni::EnvUnowned;

// 五个宏按返回类型分支；方法名在表里写全（含 Java_app_msime_and… 前缀），
// 因为 macro_rules! 无法拼接标识符。
macro_rules! stub_bytes {
    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $f(mut env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jbyteArray {
                crate::wire::not_implemented(&mut env)
            }
        )*
    };
}

macro_rules! stub_void {
    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) {}
        )*
    };
}

macro_rules! stub_int {
    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jint {
                0
            }
        )*
    };
}

macro_rules! stub_long {
    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jlong {
                0
            }
        )*
    };
}

macro_rules! stub_boolean {
    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jboolean {
                jni::sys::JNI_FALSE
            }
        )*
    };
}

// bytes 返回，77 个
stub_bytes! {
    Java_app_msime_android_NativeClient_accountSettingsApplyRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_accountSettingsExportRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_aiRequestForQueryRaw(jlong _session, jbyteArray _query);
    Java_app_msime_android_NativeClient_aiSkinPlanRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_appThemeCatalogRaw();
    Java_app_msime_android_NativeClient_applyCloudResponseRaw(jlong _session, jbyteArray _query, jbyteArray _body);
    Java_app_msime_android_NativeClient_applyOnlineCandidatesRaw(jlong _session, jbyteArray _query, jbyteArray _candidates, jint _source);
    Java_app_msime_android_NativeClient_applyTranslationsRaw(jlong _session, jlong _generation, jbyteArray _translations);
    Java_app_msime_android_NativeClient_balancePairedPunctuationAfterAutoCloseRaw(jlong _session, jint _opening);
    Java_app_msime_android_NativeClient_candidateGlossesRaw(jbyteArray _request, jbyteArray _resources);
    Java_app_msime_android_NativeClient_chooseNineKeySpellingRaw(jlong _session, jlong _generation, jlong _index);
    Java_app_msime_android_NativeClient_clearOnlineCandidatesRaw(jlong _session, jint _source);
    Java_app_msime_android_NativeClient_cloudRequestUrlRaw(jbyteArray _query);
    Java_app_msime_android_NativeClient_commonPhrasesRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_communityResourceLibraryRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_communitySkinInstallRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_defaultPreferencesRaw();
    Java_app_msime_android_NativeClient_diagnosticBundleRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_dictionaryCollectionsRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_dictionaryHansEntriesRaw(jbyteArray _text, jbyteArray _resources);
    Java_app_msime_android_NativeClient_dictionaryImportEntriesRaw(jbyteArray _request, jbyteArray _resources);
    Java_app_msime_android_NativeClient_dictionaryManifestRaw(jbyteArray _resources);
    Java_app_msime_android_NativeClient_dictionaryRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_doubaoAudioFrameRaw(jint _sequence, jbyteArray _pcm, jint _length, jboolean _final_chunk);
    Java_app_msime_android_NativeClient_doubaoDecodeFrameRaw(jbyteArray _frame);
    Java_app_msime_android_NativeClient_doubaoStartFrameRaw(jboolean _itn, jboolean _punctuation, jboolean _ddc, jbyteArray _boosting_table_id);
    Java_app_msime_android_NativeClient_emojiCatalogRaw(jbyteArray _query, jbyteArray _resources);
    Java_app_msime_android_NativeClient_englishCompletionsRaw(jbyteArray _request, jbyteArray _resources);
    Java_app_msime_android_NativeClient_glideRaw(jlong _session, jbyteArray _request);
    Java_app_msime_android_NativeClient_hostCapabilitiesRaw(jbyteArray _platform);
    Java_app_msime_android_NativeClient_keySoundPackRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_keyboardSkinTrialRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_loadPreferencesRaw(jbyteArray _directory);
    Java_app_msime_android_NativeClient_localSpeechAcceptRaw(jlong _handle, jshortArray _pcm, jint _count);
    Java_app_msime_android_NativeClient_localSpeechFinishRaw(jlong _handle);
    Java_app_msime_android_NativeClient_localSpeechStartRaw(jlong _handle, jbyteArray _model, jbyteArray _language, jbyteArray _hotwords, jint _threads);
    Java_app_msime_android_NativeClient_mobileClipboardHistoryRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_mobileVoiceConfigurationRaw(jbyteArray _directory);
    Java_app_msime_android_NativeClient_noticeDismissRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_noticesRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_onlineQueryRaw(jlong _session);
    Java_app_msime_android_NativeClient_personalDictionaryRequestRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_personalDictionarySyncRaw(jbyteArray _options);
    Java_app_msime_android_NativeClient_pluginsRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_polishPromptRaw(jbyteArray _id, jbyteArray _custom1, jbyteArray _custom2, jbyteArray _custom3);
    Java_app_msime_android_NativeClient_punctuationWithContextRaw(jlong _session, jint _ascii, jint _preceding_code_point);
    Java_app_msime_android_NativeClient_resetCacheRaw(jlong _session);
    Java_app_msime_android_NativeClient_resolveAppThemeRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_resolveThemeRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_resourcePackAdoptRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_resourcePackInstallRaw(jbyteArray _request, jobject _listener);
    Java_app_msime_android_NativeClient_resourcePacksRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_restoreDefaultPreferencesRaw(jbyteArray _directory, jlong _expected_revision);
    Java_app_msime_android_NativeClient_savePreferencesRaw(jbyteArray _directory, jlong _expected_revision, jbyteArray _snapshot);
    Java_app_msime_android_NativeClient_setCharacterWidthRaw(jlong _session, jboolean _fullwidth);
    Java_app_msime_android_NativeClient_setChinesePunctuationRaw(jlong _session, jboolean _enabled);
    Java_app_msime_android_NativeClient_setNineKeyFilterRaw(jlong _session, jboolean _single_character, jbyteArray _strokes);
    Java_app_msime_android_NativeClient_setNineKeyModeRaw(jlong _session, jboolean _enabled);
    Java_app_msime_android_NativeClient_setPrivateSessionRaw(jlong _session, jboolean _enabled);
    Java_app_msime_android_NativeClient_shuangpinKeyHintsRaw(jbyteArray _profile);
    Java_app_msime_android_NativeClient_simplifiedToTraditionalRaw(jbyteArray _text);
    Java_app_msime_android_NativeClient_smartPunctuationArmRaw(jlong _session, jbyteArray _request);
    Java_app_msime_android_NativeClient_smartPunctuationDecideRaw(jlong _session, jbyteArray _request);
    Java_app_msime_android_NativeClient_snapshotActivateRaw(jlong _handle, jbyteArray _expected_version);
    Java_app_msime_android_NativeClient_snapshotDiscardRaw(jlong _handle);
    Java_app_msime_android_NativeClient_snapshotPrepareRaw(jbyteArray _request, jbyteArray _file);
    Java_app_msime_android_NativeClient_snapshotVersionRaw(jbyteArray _options);
    Java_app_msime_android_NativeClient_telemetryBeginRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_telemetryClearRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_telemetryEndRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_telemetryFlushRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_themeCatalogRaw();
    Java_app_msime_android_NativeClient_typingStatisticsRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_updatePreferencesRaw(jlong _session, jbyteArray _snapshot);
    Java_app_msime_android_NativeClient_vocabularyReviewRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_voiceHotwordCorrectRaw(jbyteArray _request);
    Java_app_msime_android_NativeClient_voiceHotwordsRaw(jbyteArray _request);
}

// void 返回，4 个
stub_void! {
    Java_app_msime_android_NativeClient_localSpeechCancelRaw(jlong _handle);
    Java_app_msime_android_NativeClient_localSpeechDestroyRaw(jlong _handle);
    Java_app_msime_android_NativeClient_localSpeechRuntimeRaw(jbyteArray _path);
    Java_app_msime_android_NativeClient_resourcePackCancelRaw(jbyteArray _pack);
}

// int 返回，2 个
stub_int! {
    Java_app_msime_android_NativeClient_localSpeechReleaseRaw(jlong _idle_millis);
    Java_app_msime_android_NativeClient_typingStatisticsEnabledRaw(jbyteArray _directory);
}

// long 返回，1 个
stub_long! {
    Java_app_msime_android_NativeClient_localSpeechCreateRaw();
}

// boolean 返回，1 个
stub_boolean! {
    Java_app_msime_android_NativeClient_localSpeechAvailableRaw();
}
