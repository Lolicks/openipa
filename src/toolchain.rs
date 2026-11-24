//! Обнаружение компиляторов и утилит в окружении.
//! Поддерживаются Linux, macOS и Windows (LLVM/Clang или Xcode toolchain).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toolchain {
    pub clang: Option<PathBuf>,
    pub clang_version: Option<String>,
    pub clangpp: Option<PathBuf>,
    pub swiftc: Option<PathBuf>,
    pub swift_version: Option<String>,
    pub ld64_lld: Option<PathBuf>,
    pub libtool: Option<PathBuf>,
    pub ar: Option<PathBuf>,
    pub actool: Option<PathBuf>,
    pub ibtool: Option<PathBuf>,
    pub plutil: Option<PathBuf>,
    pub zip: Option<PathBuf>,
    pub host_arch: String,
    pub host_os: String,
}

/// Converts a path to a command-line argument with forward slashes.
/// LLVM tools accept forward slashes on every platform; backslashes in
/// Windows paths are otherwise interpreted as escape sequences by some drivers.
pub fn cli_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

fn env_tool(env: &str) -> Option<PathBuf> {
    std::env::var_os(env)
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

fn find_tool(env: &str, names: &[&str], extra: &[PathBuf]) -> Option<PathBuf> {
    if let Some(path) = env_tool(env) {
        return Some(path);
    }
    for name in names {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
    }
    for dir in extra {
        for name in names {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn llvm_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(windows) {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            let root = PathBuf::from(program_files);
            dirs.push(root.join("LLVM/bin"));
            for version in ["22", "21", "20", "19", "18", "17"] {
                dirs.push(root.join(format!("LLVM{version}/bin")));
            }
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Programs/LLVM/bin"));
        }
    } else {
        dirs.push(PathBuf::from("/usr/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    dirs.retain(|d| d.is_dir());
    dirs
}

fn version(tool: &PathBuf, args: &[&str]) -> Option<String> {
    let output = Command::new(tool).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    text.lines()
        .chain(err.lines())
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

impl Toolchain {
    pub fn detect() -> Result<Self> {
        let llvm_dirs = llvm_bin_dirs();
        let clang = find_tool(
            "OPENIPA_CLANG",
            &["clang", "clang.exe", "clang-22", "clang-21"],
            &llvm_dirs,
        );
        let clangpp = find_tool(
            "OPENIPA_CLANGXX",
            &["clang++", "clang++.exe", "clang++-22", "clang++-21"],
            &llvm_dirs,
        );
        let ld64_lld = find_tool(
            "OPENIPA_LD64_LLD",
            &["ld64.lld", "ld64.lld.exe", "ld64.lld-22", "ld64.lld-21"],
            &llvm_dirs,
        );
        // Пользователь может указать toolchain, включая Xcode-расположенный,
        // где есть Swift stdlib для iOS. Без этого open-source swiftc не годится.
        let swiftc = if let Some(root) = std::env::var_os("OPENIPA_SWIFT_TOOLCHAIN") {
            let candidate = PathBuf::from(root).join("usr/bin/swiftc");
            if cfg!(windows) {
                candidate
                    .exists()
                    .then_some(candidate)
                    .or_else(|| find_tool("OPENIPA_SWIFTC", &["swiftc", "swiftc.exe"], &[]))
            } else {
                candidate
                    .exists()
                    .then_some(candidate)
                    .or_else(|| find_tool("OPENIPA_SWIFTC", &["swiftc"], &[]))
            }
        } else if cfg!(windows) {
            find_tool("OPENIPA_SWIFTC", &["swiftc", "swiftc.exe"], &[])
        } else {
            find_tool("OPENIPA_SWIFTC", &["swiftc"], &[])
        };
        Ok(Self {
            clang_version: clang.as_ref().and_then(|p| version(p, &["--version"])),
            swift_version: swiftc.as_ref().and_then(|p| version(p, &["--version"])),
            clang,
            clangpp,
            swiftc,
            ld64_lld,
            libtool: find_tool("OPENIPA_LIBTOOL", &["libtool", "libtool.exe"], &[]),
            ar: find_tool(
                "OPENIPA_AR",
                &["llvm-ar", "llvm-ar.exe", "ar", "ar.exe"],
                &llvm_dirs,
            ),
            actool: find_tool("OPENIPA_ACTOOL", &["actool", "actool.exe"], &[]),
            ibtool: find_tool("OPENIPA_IBTOOL", &["ibtool", "ibtool.exe"], &[]),
            plutil: find_tool("OPENIPA_PLUTIL", &["plutil", "plutil.exe"], &[]),
            zip: find_tool("OPENIPA_ZIP", &["zip", "zip.exe"], &[]),
            host_arch: std::env::consts::ARCH.to_string(),
            host_os: std::env::consts::OS.to_string(),
        })
    }

    pub fn can_compile_c(&self) -> bool {
        self.clang.is_some()
    }
    pub fn can_compile_swift(&self) -> bool {
        self.swiftc.is_some()
    }
    pub fn can_link_macho(&self) -> bool {
        self.ld64_lld.is_some()
    }

    /// clang умеет генерировать код для arm64-apple-ios (без SDK — только объекты).
    pub fn supports_arm64_apple_ios(&self, clang: &PathBuf) -> bool {
        let Ok(out) = Command::new(clang).args(["-print-targets"]).output() else {
            return false;
        };
        let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
        text.contains("aarch64") || text.contains("arm64")
    }
}
