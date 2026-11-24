//! Драйвер Clang для C, C++, Objective-C и Objective-C++.
//! Генерирует настоящие ARM64 Mach-O объектные файлы через
//! `clang -target arm64-apple-ios`.

use crate::diagnostics::Diagnostic;
use crate::macho::parse_macho_file;
use crate::project::BuildSettings;
use crate::toolchain::Toolchain;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    C,
    Cxx,
    ObjC,
    ObjCxx,
}

impl Lang {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("c") => Some(Lang::C),
            Some("cc" | "cpp" | "cxx" | "c++") => Some(Lang::Cxx),
            Some("m") => Some(Lang::ObjC),
            Some("mm") => Some(Lang::ObjCxx),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Lang::C => "C",
            Lang::Cxx => "C++",
            Lang::ObjC => "Objective-C",
            Lang::ObjCxx => "Objective-C++",
        }
    }
}

pub struct ClangDriver<'a> {
    pub toolchain: &'a Toolchain,
    pub target_triple: String,
    pub sdk: Option<PathBuf>,
    pub min_os: String,
    pub verbose: bool,
}

pub struct CompileResult {
    pub output: PathBuf,
    pub command: String,
    pub stderr: String,
    pub object_info: crate::macho::MachOInfo,
}

impl<'a> ClangDriver<'a> {
    pub fn new(toolchain: &'a Toolchain, sdk: Option<PathBuf>) -> Self {
        Self {
            toolchain,
            target_triple: "arm64-apple-ios".into(),
            sdk,
            min_os: "16.0".into(),
            verbose: false,
        }
    }

    pub fn with_min_os(mut self, min_os: impl Into<String>) -> Self {
        self.min_os = min_os.into();
        self
    }

    pub fn with_target(mut self, triple: impl Into<String>) -> Self {
        self.target_triple = triple.into();
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Проверяет, что clang способен породить код для arm64-apple-ios.
    pub fn check(&self) -> Result<Vec<Diagnostic>> {
        let mut out = Vec::new();
        let clang = self
            .toolchain
            .clang
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("clang не найден в окружении"))?;
        if !self.toolchain.supports_arm64_apple_ios(clang) {
            bail!("clang не поддерживает target aarch64/arm64");
        }
        if self.sdk.is_none() {
            out.push(
                Diagnostic::warning(
                    "CLANG_NO_SDK",
                    "SDK не подключён — компиляция возможна только для исходников без системных заголовков",
                )
                .with_hint("Подключите iPhoneOS.sdk: openipa sdk add /path/to/iPhoneOS.sdk"),
            );
        }
        Ok(out)
    }

    pub fn compile(
        &self,
        source: &Path,
        output: &Path,
        settings: &BuildSettings,
    ) -> Result<CompileResult> {
        let lang = Lang::from_path(source)
            .ok_or_else(|| anyhow::anyhow!("неизвестный язык исходника: {}", source.display()))?;
        let clang = self
            .toolchain
            .clang
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("clang не найден"))?;

        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("не удалось создать {}", parent.display()))?;
        }

        let mut cmd = Command::new(clang);
        cmd.arg("-target")
            .arg(&self.target_triple)
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(output)
            .arg("-fexceptions")
            .arg("-fno-common");

        match lang {
            Lang::C => {
                cmd.arg("-std=c11");
            }
            Lang::Cxx => {
                let binary = self.toolchain.clangpp.as_ref().unwrap_or(clang);
                // clang++ выбирается отдельно ниже, чтобы не потерять флаги.
                if binary != clang {
                    cmd = Command::new(binary);
                    cmd.arg("-target")
                        .arg(&self.target_triple)
                        .arg("-c")
                        .arg(source)
                        .arg("-o")
                        .arg(output)
                        .arg("-fexceptions")
                        .arg("-fno-common");
                }
                cmd.arg("-std=c++17");
            }
            Lang::ObjC => {
                cmd.arg("-x").arg("objective-c");
                if settings.clang_enable_objc_arc {
                    cmd.arg("-fobjc-arc");
                }
            }
            Lang::ObjCxx => {
                let binary = self.toolchain.clangpp.as_ref().unwrap_or(clang);
                if binary != clang {
                    cmd = Command::new(binary);
                    cmd.arg("-target")
                        .arg(&self.target_triple)
                        .arg("-c")
                        .arg(source)
                        .arg("-o")
                        .arg(output)
                        .arg("-fexceptions")
                        .arg("-fno-common");
                }
                cmd.arg("-x").arg("objective-c++").arg("-std=c++17");
                if settings.clang_enable_objc_arc {
                    cmd.arg("-fobjc-arc");
                }
            }
        }

        if matches!(lang, Lang::ObjC | Lang::ObjCxx) {
            cmd.arg(format!("-fobjc-runtime=ios-{}", self.min_os));
        }
        if settings.clang_enable_modules {
            cmd.arg("-fmodules");
        }
        if let Some(sdk) = &self.sdk {
            cmd.arg("-isysroot").arg(sdk);
            cmd.arg("-F").arg(sdk.join("System/Library/Frameworks"));
        } else {
            cmd.arg("-nostdinc");
        }
        for p in &settings.framework_search_paths {
            cmd.arg("-F").arg(p);
        }
        for p in &settings.header_search_paths {
            cmd.arg("-I").arg(p);
        }
        for flag in &settings.other_c_flags {
            cmd.arg(flag);
        }

        let display = format_command(&cmd);
        let out = cmd
            .output()
            .with_context(|| format!("не удалось запустить clang для {}", source.display()))?;
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if self.verbose {
            eprintln!("[clang] {display}");
            if !stderr.is_empty() {
                eprintln!("{stderr}");
            }
        }
        if !out.status.success() {
            bail!(
                "{} компиляция {} завершилась с кодом {}:\n{}",
                lang.name(),
                source.display(),
                out.status.code().unwrap_or(-1),
                stderr.trim_end()
            );
        }
        if !output.exists() {
            bail!("clang не создал объектный файл {}", output.display());
        }
        let object_info = parse_macho_file(output)?;
        if object_info.architecture != "arm64" {
            bail!(
                "объектный файл {} имеет архитектуру {}, ожидалась arm64",
                output.display(),
                object_info.architecture
            );
        }
        Ok(CompileResult {
            output: output.to_path_buf(),
            command: display,
            stderr,
            object_info,
        })
    }
}

fn format_command(cmd: &Command) -> String {
    let mut parts = vec![cmd.get_program().to_string_lossy().into_owned()];
    for a in cmd.get_args() {
        parts.push(a.to_string_lossy().into_owned());
    }
    parts.join(" ")
}
