use std::env;
use std::path::Path;
use std::path::PathBuf;

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let lib_dir = manifest_dir.join("lib");
    let windows_icon = manifest_dir.join("assets").join("file2file_icon.ico");

    let (header_name, static_lib_candidates) = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", _) => ("libwebrpc-Mac.h", vec!["libwebrpc-Mac.a"]),
        ("linux", "aarch64") => ("libwebrpc-Linux-arm64.h", vec!["libwebrpc-Linux-arm64.a"]),
        ("linux", _) => ("libwebrpc-Linux.h", vec!["libwebrpc-Linux.a"]),
        ("windows", _) => ("libwebrpc-Windows.h", vec!["libwebrpc-Windows.a", "libwebrpc-Windows.lib"]),
        _ => return,
    };

    let header = lib_dir.join(header_name);
    let static_lib_paths: Vec<PathBuf> = static_lib_candidates
        .iter()
        .map(|name| lib_dir.join(name))
        .collect();
    let static_lib = first_existing(&static_lib_paths).unwrap_or_else(|| {
        let candidates = static_lib_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        panic!("webrpc static lib not found for target {target_os}/{target_arch}: [{candidates}]");
    });

    if !header.exists() {
        panic!(
            "webrpc header not found for target {target_os}/{target_arch}: {}",
            header.display()
        );
    }

    println!("cargo:rerun-if-changed={}", header.display());
    for path in &static_lib_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed={}", static_lib.display());
    println!("cargo:rustc-link-arg={}", static_lib.display());

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=Security");
    }

    // Windows exe icon in Explorer: embed ICO via winres (GNU target uses windres + ar).
    if target_os == "windows" && target_env == "gnu" && windows_icon.exists() {
        println!("cargo:rerun-if-changed={}", windows_icon.display());
        let mingw_bin = env::var("MINGW_BIN").unwrap_or_else(|_| {
            if Path::new("/opt/homebrew/opt/mingw-w64/bin").exists() {
                "/opt/homebrew/opt/mingw-w64/bin".to_string()
            } else if Path::new("/usr/local/opt/mingw-w64/bin").exists() {
                "/usr/local/opt/mingw-w64/bin".to_string()
            } else {
                String::new()
            }
        });
        if mingw_bin.is_empty() {
            println!("cargo:warning=skip Windows icon embed: MINGW_BIN not set and default mingw-w64 path not found");
        } else {
            let windres = format!("{target_arch}-w64-mingw32-windres");
            let ar = format!("{target_arch}-w64-mingw32-ar");
            let windres_path = PathBuf::from(&mingw_bin).join(&windres);
            let ar_path = PathBuf::from(&mingw_bin).join(&ar);
            if !windres_path.exists() || !ar_path.exists() {
                println!(
                    "cargo:warning=skip Windows icon embed: missing {} or {}",
                    windres_path.display(),
                    ar_path.display()
                );
            } else {
                let mut res = winres::WindowsResource::new();
                res.set_toolkit_path(mingw_bin.as_str());
                res.set_windres_path(windres.as_str());
                res.set_ar_path(ar.as_str());
                res.set_icon(windows_icon.to_string_lossy().as_ref());
                if let Err(err) = res.compile() {
                    println!("cargo:warning=failed to embed Windows icon: {err}");
                } else {
                    // MinGW 链接器默认会丢弃“未被引用符号”的 .a 成员，导致 .rsrc 进不了最终 exe。
                    // 强制 whole-archive 把资源对象链进主程序，资源管理器才能显示 exe 图标。
                    let out_dir = env::var("OUT_DIR").unwrap_or_default();
                    let libresource = Path::new(&out_dir).join("libresource.a");
                    if libresource.exists() {
                        println!("cargo:rustc-link-arg=-Wl,--whole-archive");
                        println!("cargo:rustc-link-arg={}", libresource.display());
                        println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");
                    }
                }
            }
        }
    }

    // Go/cgo 生成的 `libwebrpc-Windows.a` 会引用大量 `kernel32` / `msvcrt` 等符号。
    // 在 `x86_64-pc-windows-gnu` 下，该 .a 往往出现在链接命令后部；若系统库只出现在更前位置，
    // GNU ld 不会回扫，导致 `__imp_CreateEventA`、`fwrite` 等 undefined reference。
    // 在链接命令末尾再拉一遍常用 Windows API 与 MinGW C 运行时即可闭合依赖。
    if target_os == "windows" && target_env == "gnu" {
        println!("cargo:rustc-link-arg=-Wl,-Bdynamic");
        for lib in [
            "kernel32",
            "user32",
            "advapi32",
            "shell32",
            "ws2_32",
            "winmm",
            "ole32",
            "oleaut32",
            "iphlpapi",
            "userenv",
            "crypt32",
            "secur32",
            "bcrypt",
            "ntdll",
            "dbghelp",
            "psapi",
            "version",
            "netapi32",
        ] {
            println!("cargo:rustc-link-arg=-l{lib}");
        }
        println!("cargo:rustc-link-arg=-lmsvcrt");
        println!("cargo:rustc-link-arg=-lmingwex");
    }
}
