use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_clang_runtime();
    }
    tauri_build::build()
}

/// Link clang's compiler-rt builtins on macOS.
///
/// Objective-C `@available` lowers to a call to `___isPlatformVersionAtLeast`,
/// which lives in compiler-rt rather than libSystem. Rust links with
/// `-nodefaultlibs`, so a C or Objective-C dependency that uses `@available`
/// (ggml-metal, through transcribe-cpp-sys) leaves that symbol undefined at the
/// final link. Naming it here is what a clang-driven link would have done.
fn link_clang_runtime() {
    let Some(dir) = runtime_dir() else {
        // Better a link error naming the real symbol than a build script that
        // fails on a machine whose toolchain is laid out differently.
        println!("cargo:warning=clang runtime dir not found; @available may not link");
        return;
    };
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}

/// Where clang keeps its darwin builtins, asked of the toolchain rather than
/// assembled from an Xcode path that moves every release.
fn runtime_dir() -> Option<String> {
    let output = Command::new("clang")
        .arg("-print-runtime-dir")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let dir = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!dir.is_empty() && std::path::Path::new(&dir).is_dir()).then_some(dir)
}
