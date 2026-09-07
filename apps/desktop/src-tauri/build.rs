use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_clang_runtime();
    }
    tauri_build::build()
}

/// Links clang's compiler-rt builtins on macOS, which is what a clang-driven link would have done.
/// `@available` lowers to `___isPlatformVersionAtLeast`, absent from libSystem, and Rust links with `-nodefaultlibs`, so ggml-metal leaves it undefined.
fn link_clang_runtime() {
    let Some(dir) = runtime_dir() else {
        // Better a link error naming the real symbol than a build script failing on a differently laid-out toolchain.
        println!("cargo:warning=libclang_rt.osx.a not found; @available may not link");
        return;
    };
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}

/// The directory holding `libclang_rt.osx.a`, asked of the toolchain first and then searched.
/// An Xcode release candidate can report a runtime directory that holds no static archives, which is how this fails on a CI image and nowhere else.
fn runtime_dir() -> Option<PathBuf> {
    const ARCHIVE: &str = "libclang_rt.osx.a";

    if let Some(dir) = printed_runtime_dir() {
        if dir.join(ARCHIVE).is_file() {
            return Some(dir);
        }
    }
    let roots = [
        developer_dir().map(|dir| dir.join("Toolchains")),
        Some(PathBuf::from(
            "/Library/Developer/CommandLineTools/usr/lib/clang",
        )),
    ];
    roots
        .into_iter()
        .flatten()
        .find_map(|root| find_archive(&root, ARCHIVE, 6))
}

fn printed_runtime_dir() -> Option<PathBuf> {
    let output = Command::new("clang")
        .arg("-print-runtime-dir")
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    let dir = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

fn developer_dir() -> Option<PathBuf> {
    let output = Command::new("xcode-select").arg("-p").output().ok()?;
    output.status.success().then_some(())?;
    let dir = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

/// Depth-bounded search for `name`, returning the directory that holds it.
fn find_archive(root: &Path, name: &str, depth: u32) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().is_some_and(|file| file == name) {
            return path.parent().map(Path::to_path_buf);
        }
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.iter()
        .find_map(|dir| find_archive(dir, name, depth - 1))
}
