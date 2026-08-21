use std::env;
use std::path::PathBuf;

fn main() {
    link_webrpc_sdk();
    tauri_build::build();
}

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

fn link_webrpc_sdk() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let sdk_dir = manifest_dir.join("..").join("sdk");

    let (header_name, static_lib_names) = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", _) => ("libwebrpc-Mac.h", vec!["libwebrpc-Mac.a"]),
        ("linux", "aarch64") => ("libwebrpc-Linux-arm64.h", vec!["libwebrpc-Linux-arm64.a"]),
        ("linux", _) => ("libwebrpc-Linux.h", vec!["libwebrpc-Linux.a"]),
        ("windows", _) => (
            "libwebrpc-Windows.h",
            vec!["libwebrpc-Windows.a", "libwebrpc-Windows.lib"],
        ),
        _ => return,
    };

    let header = sdk_dir.join(header_name);
    let static_lib_paths: Vec<PathBuf> = static_lib_names.iter().map(|name| sdk_dir.join(name)).collect();
    let static_lib = first_existing(&static_lib_paths).unwrap_or_else(|| {
        panic!(
            "webrpc 静态库未找到: {}",
            static_lib_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    });
    if !header.exists() {
        panic!("webrpc 头文件未找到: {}", header.display());
    }

    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-changed={}", static_lib.display());
    println!("cargo:rustc-link-arg={}", static_lib.display());

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=resolv");
    }

    if target_os == "linux" {
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=dl");
        println!("cargo:rustc-link-lib=dylib=m");
        println!("cargo:rustc-link-lib=dylib=resolv");
    }

    if target_os == "windows" {
        let libs = [
            "kernel32", "user32", "advapi32", "shell32", "ws2_32", "winmm", "ole32", "oleaut32",
            "iphlpapi", "userenv", "crypt32", "secur32", "bcrypt", "ntdll", "dbghelp", "psapi",
            "version", "netapi32",
        ];
        if target_env == "gnu" {
            // 静态链上 MinGW 运行库，避免再带 libgcc / libwinpthread dll。
            // kernel32 等系统库仍动态链接，那是 Windows 自带的。
            println!("cargo:rustc-link-arg=-static-libgcc");
            println!("cargo:rustc-link-arg=-Wl,-Bstatic");
            println!("cargo:rustc-link-arg=-lpthread");
            println!("cargo:rustc-link-arg=-lwinpthread");
            println!("cargo:rustc-link-arg=-lmingwex");
            println!("cargo:rustc-link-arg=-Wl,-Bdynamic");
            for lib in libs {
                println!("cargo:rustc-link-arg=-l{lib}");
            }
        } else {
            // Go c-archive 会直接调用 fprintf；MSVC 新 CRT 要把兼容库链上。
            println!("cargo:rustc-link-lib=legacy_stdio_definitions");
            // 强制拉入 Go 库入口，避免 MSVC/lld 丢掉未从 CRT 引用的构造函数。
            println!("cargo:rustc-link-arg=/INCLUDE:_rt0_amd64_windows_lib");
            println!("cargo:rustc-link-arg=/INCLUDE:_cgo_maybe_run_preinit");
            for lib in libs {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
    }
}
