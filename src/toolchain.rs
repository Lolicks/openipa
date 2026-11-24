//! Обнаружение компиляторов и утилит в окружении.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

fn which(name: &str) -> Option<PathBuf> {
    which::which(name).ok()
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
        let clang = which("clang")
            .or_else(|| which("clang-21"))
            .or_else(|| which("clang-22"));
        let clangpp = which("clang++")
            .or_else(|| which("clang++-21"))
            .or_else(|| which("clang++-22"));
        let ld64_lld = which("ld64.lld")
            .or_else(|| which("ld64.lld-21"))
            .or_else(|| which("ld64.lld-22"));
        // Пользователь может указать toolchain, включая Xcode-расположенный,
        // где есть Swift stdlib для iOS. Без этого open-source swiftc не годится.
        let swiftc = if let Some(root) = std::env::var_os("OPENIPA_SWIFT_TOOLCHAIN") {
            let candidate = PathBuf::from(root).join("usr/bin/swiftc");
            candidate
                .exists()
                .then_some(candidate)
                .or_else(|| which("swiftc"))
        } else {
            which("swiftc")
        };
        Ok(Self {
            clang_version: clang.as_ref().and_then(|p| version(p, &["--version"])),
            swift_version: swiftc.as_ref().and_then(|p| version(p, &["--version"])),
            clang,
            clangpp,
            swiftc,
            ld64_lld,
            libtool: which("libtool"),
            ar: which("llvm-ar").or_else(|| which("ar")),
            actool: which("actool"),
            ibtool: which("ibtool"),
            plutil: which("plutil"),
            zip: which("zip"),
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
