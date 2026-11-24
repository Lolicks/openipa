//! Интеграция с ld64.lld — настоящим Mach-O линкером из LLVM.
//! Без SDK линкер способен создать структурно корректный MH_EXECUTE, но
//! такой бинарник не будет загружен iOS (нет libSystem и dyld) — это
//! явно диагностируется и не выдаётся за рабочее приложение.

use crate::diagnostics::Diagnostic;
use crate::macho::{parse_macho_file, MachOInfo};
use crate::toolchain::Toolchain;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Linker<'a> {
    pub toolchain: &'a Toolchain,
    pub sdk: Option<PathBuf>,
    pub min_os: String,
    pub sdk_version: String,
    pub frameworks: Vec<String>,
    pub framework_search_paths: Vec<PathBuf>,
    pub other_ldflags: Vec<String>,
    pub swift_runtime: bool,
    pub verbose: bool,
}

pub struct LinkResult {
    pub output: PathBuf,
    pub command: String,
    pub stderr: String,
    pub info: MachOInfo,
    pub loadable_on_ios: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Linker<'a> {
    pub fn new(
        toolchain: &'a Toolchain,
        sdk: Option<PathBuf>,
        min_os: String,
        sdk_version: String,
    ) -> Self {
        Self {
            toolchain,
            sdk,
            min_os,
            sdk_version,
            frameworks: Vec::new(),
            framework_search_paths: Vec::new(),
            other_ldflags: Vec::new(),
            swift_runtime: false,
            verbose: false,
        }
    }

    pub fn with_frameworks(mut self, frameworks: Vec<String>) -> Self {
        self.frameworks = frameworks;
        self
    }

    pub fn with_framework_search_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.framework_search_paths = paths;
        self
    }

    pub fn with_other_ldflags(mut self, flags: Vec<String>) -> Self {
        self.other_ldflags = flags;
        self
    }

    pub fn with_swift_runtime(mut self, enabled: bool) -> Self {
        self.swift_runtime = enabled;
        self
    }

    pub fn link(&self, objects: &[PathBuf], output: &Path) -> Result<LinkResult> {
        if objects.is_empty() {
            bail!("нет объектных файлов для линковки");
        }
        let ld = self.toolchain.ld64_lld.as_ref().ok_or_else(|| {
            anyhow::anyhow!("ld64.lld не найден. Установите пакет lld (ld64.lld-22).")
        })?;

        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut cmd = Command::new(ld);
        cmd.arg("-arch")
            .arg("arm64")
            .arg("-platform_version")
            .arg("ios")
            .arg(&self.min_os)
            .arg(&self.sdk_version)
            .arg("-o")
            .arg(output);
        for obj in objects {
            cmd.arg(obj);
        }
        let mut diagnostics = Vec::new();
        for path in &self.framework_search_paths {
            cmd.arg("-F").arg(path);
        }
        if let Some(sdk) = &self.sdk {
            cmd.arg("-syslibroot").arg(sdk);
            cmd.arg("-L").arg(sdk.join("usr/lib"));
            cmd.arg("-lSystem");
            if self.swift_runtime {
                cmd.arg("-L").arg(sdk.join("usr/lib/swift/iphoneos"));
                cmd.arg("-lswiftCore");
            }
            cmd.arg("-F").arg(sdk.join("System/Library/Frameworks"));
            for framework in &self.frameworks {
                let name = framework
                    .trim_end_matches(".framework")
                    .trim_end_matches(".tbd")
                    .trim_end_matches(".dylib");
                if name.is_empty() {
                    continue;
                }
                cmd.arg("-framework").arg(name);
            }
            for flag in &self.other_ldflags {
                cmd.arg(flag);
            }
        } else {
            cmd.arg("-e").arg("_main");
            diagnostics.push(Diagnostic::warning(
                "LINK_NO_SDK",
                "линковка без SDK: бинарник не зависит от libSystem и не может быть загружен iOS",
            ));
        }
        // Символ main может отсутствовать в ObjC-приложении с UIApplicationMain,
        // поэтому -e задаётся только когда известна точка входа.
        let display = format!("{cmd:?}");
        let out = cmd
            .output()
            .with_context(|| format!("не удалось запустить {}", ld.display()))?;
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if self.verbose {
            eprintln!("[ld64.lld] {display}\n{stderr}");
        }
        if !out.status.success() {
            bail!(
                "линковка завершилась с кодом {}:\n{}",
                out.status.code().unwrap_or(-1),
                stderr.trim_end()
            );
        }
        let info = parse_macho_file(output)?;
        if info.architecture != "arm64" || info.filetype_name != "MH_EXECUTE" {
            bail!(
                "линкер создал не ARM64 MH_EXECUTE: {} / {}",
                info.architecture,
                info.filetype_name
            );
        }
        let has_dyld = info
            .load_commands
            .iter()
            .any(|c| c.name.contains("DYLINKER"));
        let loadable = self.sdk.is_some() && has_dyld && !info.dylibs.is_empty();
        if self.sdk.is_some() && !loadable {
            diagnostics.push(Diagnostic::warning(
                "LINK_NO_DYLD_DEPS",
                "в исполняемом файле нет зависимостей от системных dylib — проверьте SDK",
            ));
        }
        Ok(LinkResult {
            output: output.to_path_buf(),
            command: display,
            stderr,
            info,
            loadable_on_ios: loadable,
            diagnostics,
        })
    }
}
