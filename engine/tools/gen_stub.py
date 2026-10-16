#!/usr/bin/env python3
"""从 NativeClient.java 生成 engine/src/stub.rs。

stub.rs 里是「非输入路径」的 native 方法占位实现，签名逐位对齐 Java 声明。
真实现的 19 个（REAL 集合）由 src/lib.rs 手写，不在这张表里，避免重复符号。

用法（在 engine/ 目录）：
    python -X utf8 tools/gen_stub.py
"""
import re
import pathlib

JAVA = pathlib.Path(__file__).resolve().parents[2] / "host" / "java" / "app" / "msime" / "android" / "core" / "NativeClient.java"
OUT = pathlib.Path(__file__).resolve().parents[1] / "src" / "stub.rs"

PREFIX = "Java_app_msime_android_NativeClient_"

# lib.rs 里手写实现的方法；其余全部生成占位。
REAL = {
    "prepareHostRaw",
    "createRaw",
    "destroyRaw",
    "focusRaw",
    "characterRaw",
    "commandRaw",
    "selectRaw",
    "viewRaw",
    "setEnglishModeRaw",
    # 候选长按四件：Core 无对应能力，回「ok:true + 原样 View + handled:false」止血，
    # 不再是 not_implemented 桩。理由见 lib.rs 的 no_op。
    "pinCandidateRaw",
    "fixCandidatePositionRaw",
    "clearCandidatePositionRaw",
    "selectEdgeRaw",
    # 第五件有真对应能力：Engine::forget（qingjian-core engine/learning/mod.rs:152）。
    "removeCandidateRaw",
    # 展开候选面板与它唯一的写入口，必须成对：只回面板应答而点候选仍回桩，等于
    # 「面板能开了、一点就死会话」。见 lib.rs 的 allCandidatesRaw。
    "allCandidatesRaw",
    "selectAnyCandidateRaw",
    # 升级安装包后重解包的开关：只比随包锁里那几个文件的长度，不符就回
    # dictionary_outdated，宿主 Bootstrap.refreshExistingConfiguration 才会重新解包。
    # 逻辑在 resources_lock.rs（宿主可测），这里只是信封。
    "refreshHostRaw",
    # 偏好读写方案丙第一步：load 回「修订号 0 + 空偏好」，update 应用 learning 并回完整 View。
    # 两条必须成对——只做 load 消不掉那句提示，因为宿主 applyPreferencesSnapshot 自己还要
    # 过一次 updatePreferences。逻辑在 preferences.rs，理由见那里的注释与 PIN 同名小节。
    # save / restoreDefault / default 仍是桩（第二步与丁），default 在宿主里零调用点。
    "loadPreferencesRaw",
    "updatePreferencesRaw",
}

# Java 类型 -> JNI Rust 类型
TYPE_MAP = {
    "byte[]": "jbyteArray",
    "short[]": "jshortArray",
    "long": "jlong",
    "int": "jint",
    "boolean": "jboolean",
}
RET_MAP = {"byte[]": "bytes", "void": "void", "int": "int", "long": "long", "boolean": "boolean"}


def parse(text):
    """返回 [(ret_type, java_name, [(rust_type, param_name), ...]), ...]"""
    decls = []
    for ret, name, args in re.findall(
        r"private static native\s+([A-Za-z0-9_\[\]]+)\s+([A-Za-z0-9_]+)\s*\(([^;]*)\)", text, re.S
    ):
        args = re.sub(r"\s+", " ", args).strip()
        params = []
        if args:
            for i, part in enumerate(a.strip() for a in args.split(",")):
                m = re.match(r"([A-Za-z0-9_\[\]]+)(?:\s+([A-Za-z0-9_]+))?$", part)
                if not m:
                    raise SystemExit("参数解析失败: %r" % part)
                ty, pname = m.group(1), m.group(2)
                rt = TYPE_MAP.get(ty)
                if rt is None:
                    # 非原生类型 = Java 对象（ResourcePackProgress 回调）
                    rt = "jobject"
                snake = re.sub(r"([A-Z])", lambda m: "_" + m.group(1).lower(), pname); params.append((rt, ("_%s" % snake) if pname else "_arg%d" % i))
        decls.append((ret, name, params))
    return decls


def main():
    decls = parse(JAVA.read_text(encoding="utf-8"))
    total = len(decls)
    names = [d[1] for d in decls]
    assert len(set(names)) == total, "方法名有重复"
    missing = REAL - set(names)
    assert not missing, "REAL 集合里的名字不在 Java 里: %s" % missing

    stubs = [d for d in decls if d[1] not in REAL]

    lines = [
        "// 由 tools/gen_stub.py 从 NativeClient.java 解析生成，请勿手改；要改内容就改生成脚本。",
        "//",
        "// 这些是非输入路径的 native 方法占位实现：签名逐位对齐 Java 声明，返回",
        "// {\"ok\":false,\"error\":\"not_implemented\"}，靠水杉 Java 自身的降级分支（fail()）失效。",
        "// 真实现的 %d 个在 lib.rs，合计 %d 个符号一个不能少。" % (len(REAL), total),
        "",
        "use jni::objects::JObject;",
        "use jni::sys::{jboolean, jbyteArray, jint, jlong, jshortArray, jobject};",
        "use jni::EnvUnowned;",
        "",
        "// 五个宏按返回类型分支；方法名在表里写全（含 " + PREFIX[:18] + "… 前缀），",
        "// 因为 macro_rules! 无法拼接标识符。",
        "macro_rules! stub_bytes {",
        "    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {",
        "        $(",
        "            #[unsafe(no_mangle)]",
        "            pub extern \"system\" fn $f(mut env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jbyteArray {",
        "                crate::wire::not_implemented(&mut env)",
        "            }",
        "        )*",
        "    };",
        "}",
        "",
        "macro_rules! stub_void {",
        "    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {",
        "        $(",
        "            #[unsafe(no_mangle)]",
        "            pub extern \"system\" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) {}",
        "        )*",
        "    };",
        "}",
        "",
        "macro_rules! stub_int {",
        "    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {",
        "        $(",
        "            #[unsafe(no_mangle)]",
        "            pub extern \"system\" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jint {",
        "                0",
        "            }",
        "        )*",
        "    };",
        "}",
        "",
        "macro_rules! stub_long {",
        "    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {",
        "        $(",
        "            #[unsafe(no_mangle)]",
        "            pub extern \"system\" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jlong {",
        "                0",
        "            }",
        "        )*",
        "    };",
        "}",
        "",
        "macro_rules! stub_boolean {",
        "    ($($f:ident ( $($t:tt $n:ident),* $(,)? );)*) => {",
        "        $(",
        "            #[unsafe(no_mangle)]",
        "            pub extern \"system\" fn $f(_env: EnvUnowned<'_>, _class: JObject<'_>, $($n: $t),*) -> jboolean {",
        "                jni::sys::JNI_FALSE",
        "            }",
        "        )*",
        "    };",
        "}",
        "",
    ]

    by_ret = {}
    for ret, name, params in stubs:
        by_ret.setdefault(RET_MAP[ret], []).append((name, params))

    emitted = 0
    for bucket in ("bytes", "void", "int", "long", "boolean"):
        items = sorted(by_ret.get(bucket, []))
        if not items:
            continue
        lines.append("// %s 返回，%d 个" % (bucket, len(items)))
        lines.append("stub_%s! {" % bucket)
        for name, params in items:
            inner = ", ".join("%s %s" % (t, n) for t, n in params)
            lines.append("    %s(%s);" % (PREFIX + name, inner))
            emitted += 1
        lines.append("}")
        lines.append("")

    with open(OUT, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines))
    print("Java 声明总数        = %d" % total)
    print("REAL（lib.rs 手写）  = %d" % len(REAL))
    print("生成 stub            = %d (emit=%d)" % (len(stubs), emitted))
    print("合计导出             = %d" % (len(REAL) + emitted))
    for bucket in ("bytes", "void", "int", "long", "boolean"):
        print("  %-8s %d" % (bucket, len(by_ret.get(bucket, []))))
    print("写入 %s (%d 行)" % (OUT, len(lines)))


if __name__ == "__main__":
    main()
