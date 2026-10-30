use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let sdk = env::var_os("PVZP_SDK").map(PathBuf::from);
    let portable_root = sdk
        .clone()
        .or_else(|| env::var_os("PVZP_ROOT").map(PathBuf::from))
        .unwrap_or_else(|| {
            manifest_dir
                .ancestors()
                .nth(4)
                .expect("pvzp-rs must live under RustVsZombies/crates/backends/pvz_portable")
                .parent()
                .expect("RustVsZombies must have a workspace parent")
                .join("PvZ-Portable")
        });
    assert!(
        portable_root.join("src/LawnApp.h").is_file(),
        "PvZ-Portable source tree not found at {}",
        portable_root.display()
    );

    println!("cargo:rerun-if-env-changed=PVZP_ROOT");
    println!("cargo:rerun-if-env-changed=PVZP_SDK");
    println!("cargo:rerun-if-env-changed=RSVZ_PORTABLE_IMPORT_DIR");
    println!("cargo:rerun-if-env-changed=CLANG_PATH");
    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed={}", portable_root.join("src").display());
    for input in ["build.txt", "sdl-include.txt"] {
        let path = portable_root.join(input);
        // Source-tree builds have no SDK metadata; missing paths would rebuild on every invocation.
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    println!("cargo:rerun-if-env-changed=CXX");

    let target = env::var("TARGET").expect("TARGET");
    let android = target.contains("android");
    let sdk_flags = sdk_definitions(&portable_root, &target);
    let mut builder = bindgen::Builder::default()
        .header(manifest_dir.join("cpp/bridge.h").to_string_lossy())
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++20")
        .clang_arg(format!("-I{}", cmake_path(&portable_root.join("src"))))
        .clang_arg(format!("-I{}", cmake_path(&portable_root.join("src/SexyAppFramework"))))
        .allowlist_type("pvzp_rs_.*")
        .allowlist_type("GameObject|Plant|Zombie|Projectile|GridItem|Coin|SeedPacket|Reanimation")
        .allowlist_function("pvzp_rs_.*")
        .opaque_type("std::.*")
        .opaque_type("Sexy::.*")
        .derive_debug(false)
        .layout_tests(true)
        .generate_comments(false);
    for definition in &sdk_flags {
        builder = builder.clang_arg(format!("-D{definition}"));
    }
    if android {
        builder = builder.clang_args(android_flags());
        let import_dir = portable_root.join("lib");
        println!("cargo:rustc-link-search=native={}", import_dir.display());
        println!("cargo:rustc-link-lib=dylib=main");
    }

    if target.ends_with("windows-gnu") {
        builder = builder
            .clang_arg("--target=x86_64-w64-windows-gnu")
            .clang_arg(format!("-isystem{}", cmake_path(&clang_resource_include())));
        for include in mingw_cpp_include_paths() {
            builder = builder.clang_arg(format!("-isystem{}", cmake_path(&include)));
        }
    } else if target.ends_with("windows-msvc") {
        builder = builder.clang_arg("--target=x86_64-pc-windows-msvc");
    }

    if let Some(import_dir) = sdk
        .filter(|_| target.contains("windows"))
        .as_ref()
        .map(|sdk| sdk.join("lib").into_os_string())
        .or_else(|| env::var_os("RSVZ_PORTABLE_IMPORT_DIR"))
        .filter(|_| target.contains("windows"))
    {
        println!("cargo:rustc-link-search=native={}", PathBuf::from(import_dir).display());
        println!("cargo:rustc-link-lib=dylib=pvz-portable");
    }

    compile_bridge(&manifest_dir, &portable_root, &target, &sdk_flags);

    let bindings = builder
        .generate()
        .expect("failed to generate PvZ-Portable raw bindings");
    generate_raw_layout_checks(&bindings.to_string());
    bindings
        .write_to_file(PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("bindings.rs"))
        .expect("failed to write PvZ-Portable raw bindings");
}

fn generate_raw_layout_checks(bindings: &str) {
    let types = [
        "PointerInput",
        "GameObject",
        "Plant",
        "Zombie",
        "Projectile",
        "Coin",
        "GridItem",
        "SeedPacket",
        "Reanimation",
        "LawnMower",
    ];
    let mut source = String::from(
        "use crate::raw::*;\nuse std::mem::{size_of, align_of, offset_of};\npub const RAW_LAYOUT: &[pvzp_rs_layout_entry] = &[\n",
    );
    let mut entry = |name: &str, expression: String| {
        let mut hash = 14695981039346656037_u64;
        for byte in name.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(1099511628211);
        }
        source.push_str(&format!(
            "pvzp_rs_layout_entry {{ name: {hash}, value: ({expression}) as u64 }},\n"
        ));
    };
    entry("pointer.size", "size_of::<*const ()>()".to_owned());
    for name in types {
        entry(&format!("{name}.size"), format!("size_of::<{name}>()"));
        entry(&format!("{name}.align"), format!("align_of::<{name}>()"));
        let start = format!("pub struct {name} {{");
        let body = bindings
            .split_once(&start)
            .unwrap_or_else(|| panic!("missing raw {name}"))
            .1
            .split_once("\n}")
            .expect("struct end")
            .0;
        for line in body.lines() {
            let Some(field) = line.trim().strip_prefix("pub ") else {
                continue;
            };
            let Some((field, ty)) = field.split_once(": ") else {
                continue;
            };
            let ty = ty.trim_end_matches(',');
            if field == "_base" {
                entry(&format!("{name}.base.{ty}"), format!("offset_of!({name}, {field})"));
            } else {
                assert!(
                    name == "PointerInput" || field.starts_with('m'),
                    "unexpected raw field {name}.{field}"
                );
                entry(&format!("{name}.{field}"), format!("offset_of!({name}, {field})"));
                entry(&format!("{name}.{field}.size"), format!("size_of::<{ty}>()"));
            }
        }
    }
    entry("Rect.size", "size_of::<pvzp_rs_rect>()".to_owned());
    entry("Rect.align", "align_of::<pvzp_rs_rect>()".to_owned());
    source.push_str("];\n");
    std::fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("raw_layout_checks.rs"),
        source,
    )
    .expect("write raw layout checks");
}

fn compile_bridge(manifest_dir: &Path, portable_root: &Path, target: &str, sdk_flags: &[String]) {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let gnu = target.ends_with("windows-gnu");
    let unix = !target.contains("windows");
    let release = env::var("PROFILE").as_deref() == Ok("release");
    let llvm = Path::new(r"C:\Program Files\LLVM\bin");
    let object = out.join("pvzp_bridge.obj");
    let mut compiler = Command::new(if unix {
        env::var_os("CXX")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("clang++"))
    } else {
        llvm.join(if gnu { "clang++.exe" } else { "clang-cl.exe" })
    });
    let mut includes = vec![
        portable_root.join("src"),
        portable_root.join("src/SexyAppFramework"),
        portable_root.join("src/SexyAppFramework/sound/SDL-Mixer-X/include"),
        if unix {
            PathBuf::from("/usr/include/SDL2")
        } else if gnu {
            PathBuf::from("C:/msys64/ucrt64/include/SDL2")
        } else {
            manifest_dir
                .ancestors()
                .nth(4)
                .expect("workspace")
                .join("target/rsvz/vcpkg/msvc/x64-windows/include/SDL2")
        },
    ];
    if let Ok(sdl_includes) = std::fs::read_to_string(portable_root.join("sdl-include.txt")) {
        includes.pop();
        includes.extend(
            sdl_includes
                .split(';')
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        );
    }
    if !unix && !gnu && sdk_flags.iter().any(|flag| flag == "_DEBUG") {
        println!("cargo:rustc-link-lib=dylib=ucrtd");
    }
    if unix {
        compiler.args(["-std=c++20", "-c", "-fPIC", "-ffunction-sections", "-fdata-sections"]);
        if target.contains("android") {
            compiler.args(android_flags());
        }
        compiler.arg(if release { "-O2" } else { "-O0" });
        compiler.arg("-o").arg(&object);
    } else if gnu {
        compiler.args([
            "--target=x86_64-w64-windows-gnu",
            "--sysroot=C:/msys64/ucrt64",
            "-std=c++20",
            "-c",
            "-ffunction-sections",
            "-fdata-sections",
        ]);
        // All native includes use the matching UCRT/libstdc++ toolchain.
        compiler.arg("-Wno-invalid-constexpr");
        compiler.arg(if release { "-O2" } else { "-O0" });
        compiler.arg("-o").arg(&object);
    } else {
        compiler.args(["/std:c++20", "/utf-8", "/EHsc", "/c", "/Gy", "/Gw"]);
        compiler.arg(if sdk_flags.iter().any(|flag| flag == "_DEBUG") {
            "/MDd"
        } else {
            "/MD"
        });
        compiler.arg(if release { "/O2" } else { "/Od" });
        compiler.arg(format!("/Fo{}", object.display()));
    }
    compiler.arg("-Wno-invalid-offsetof");
    println!("cargo:rerun-if-env-changed=PVZP_CROSS_FFI_LTO");
    if release && (!unix || env::var_os("PVZP_CROSS_FFI_LTO").is_some()) {
        compiler.arg("-flto=thin");
    }
    for include in includes {
        compiler.arg(format!("-I{}", include.display()));
    }
    for definition in sdk_flags {
        compiler.arg(format!("-D{definition}"));
    }
    compiler.arg(manifest_dir.join("cpp/bridge.cpp"));
    assert!(
        compiler.status().expect("start Clang bridge compiler").success(),
        "Portable bridge compilation failed"
    );
    let status = if unix {
        Command::new(env::var_os("AR").unwrap_or_else(|| "ar".into()))
            .arg("crs")
            .arg(out.join("libpvzp_bridge.a"))
            .arg(&object)
            .status()
    } else if gnu {
        Command::new(llvm.join("llvm-ar.exe"))
            .arg("crs")
            .arg(out.join("libpvzp_bridge.a"))
            .arg(&object)
            .status()
    } else {
        Command::new(llvm.join("llvm-lib.exe"))
            .arg(format!("/OUT:{}", out.join("pvzp_bridge.lib").display()))
            .arg(&object)
            .status()
    };
    assert!(status.expect("archive bridge").success());
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=pvzp_bridge");
    if unix {
        println!(
            "cargo:rustc-link-lib={}",
            if target.contains("android") {
                "c++_shared"
            } else if target.contains("apple") {
                "c++"
            } else {
                "stdc++"
            }
        );
    }
    if gnu {
        println!("cargo:rustc-link-search=native=C:/msys64/ucrt64/lib");
        println!("cargo:rustc-link-lib=stdc++");
    }
}

fn android_flags() -> Vec<String> {
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_HOME");
    let ndk = PathBuf::from(env::var_os("ANDROID_NDK_HOME").expect("ANDROID_NDK_HOME"));
    let host = if cfg!(windows) {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    };
    let sysroot = ndk.join("toolchains/llvm/prebuilt").join(host).join("sysroot");
    vec![
        format!("--target={}29", env::var("TARGET").expect("TARGET")),
        format!("--sysroot={}", cmake_path(&sysroot)),
        format!("-isystem{}", cmake_path(&sysroot.join("usr/include/c++/v1"))),
    ]
}

fn sdk_definitions(root: &Path, target: &str) -> Vec<String> {
    let Ok(config) = std::fs::read_to_string(root.join("build.txt")) else {
        return Vec::new();
    };
    let mut definitions = Vec::new();
    if config.lines().any(|line| line == "pvz_debug=1") {
        definitions.push("PVZ_DEBUG".to_owned());
    }
    if config.lines().any(|line| line == "low_memory=1") {
        definitions.push("LOW_MEMORY".to_owned());
    }
    if target.ends_with("windows-msvc") && config.lines().any(|line| line == "profile=Debug") {
        definitions.push("_DEBUG".to_owned());
    }
    definitions
}

fn mingw_cpp_include_paths() -> Vec<PathBuf> {
    let compiler = env::var_os("CXX").map_or_else(|| PathBuf::from(r"C:\msys64\ucrt64\bin\g++.exe"), PathBuf::from);
    let output = Command::new(compiler)
        .args(["-E", "-x", "c++", "-", "-v"])
        .stdin(Stdio::null())
        .output()
        .expect("failed to query MinGW C++ include paths");
    assert!(output.status.success(), "failed to query MinGW C++ include paths");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut in_search = false;
    stderr
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed == "#include <...> search starts here:" {
                in_search = true;
                return None;
            }
            if trimmed == "End of search list." {
                in_search = false;
                return None;
            }
            in_search.then(|| PathBuf::from(trimmed.trim_end_matches(" (framework directory)")))
        })
        .filter(|path| {
            let path = path.to_string_lossy().replace('\\', "/");
            path.contains("/include/c++/") || !path.contains("/lib/gcc/")
        })
        .collect()
}

fn clang_resource_include() -> PathBuf {
    let configured = env::var_os("CLANG_PATH").map(PathBuf::from);
    let candidates = configured.into_iter().chain([
        PathBuf::from("clang"),
        PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe"),
    ]);
    for candidate in candidates {
        let compiler = if candidate.is_dir() {
            candidate.join("clang.exe")
        } else {
            candidate
        };
        let Ok(output) = Command::new(&compiler).arg("--print-resource-dir").output() else {
            continue;
        };
        if output.status.success() {
            let include = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()).join("include");
            if include.is_dir() {
                return include;
            }
        }
    }
    panic!("failed to locate Clang resource headers; set CLANG_PATH to clang.exe");
}

fn cmake_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
