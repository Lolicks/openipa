//! Драйвер Swift. Честно сообщает об объективном ограничении: открытый
//! Swift toolchain для Linux не содержит Swift stdlib/Frameworks для
//! `arm64-apple-ios` и не может собирать SwiftUI/UIKit без Apple SDK.

use crate::diagnostics::Diagnostic;
use crate::sdk::SdkReport;
use crate::toolchain::Toolchain;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwiftCapability {
    pub swiftc_available: bool,
    pub swiftc_path: Option<PathBuf>,
    pub swift_version: Option<String>,
    pub sdk_present: bool,
    pub sdk_frameworks_complete: bool,
    pub swift_stdlib_iphoneos: Option<PathBuf>,
    pub can_target_ios: bool,
    pub can_build_swiftui: bool,
    pub limitation: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

fn find_ios_stdlib(swiftc: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = swiftc.parent() {
        candidates.push(parent.join("../../lib/swift/iphoneos"));
        candidates.push(parent.join("../../../lib/swift/iphoneos"));
    }
    if let Some(root) = std::env::var_os("OPENIPA_SWIFT_TOOLCHAIN") {
        candidates.push(PathBuf::from(root).join("usr/lib/swift/iphoneos"));
    }
    candidates
        .into_iter()
        .map(|p| p.canonicalize().unwrap_or(p))
        .find(|p| p.is_dir())
}

pub struct SwiftDriver<'a> {
    pub toolchain: &'a Toolchain,
    pub sdk: Option<SdkReport>,
    pub verbose: bool,
}

impl<'a> SwiftDriver<'a> {
    pub fn new(toolchain: &'a Toolchain, sdk: Option<SdkReport>) -> Self {
        Self {
            toolchain,
            sdk,
            verbose: false,
        }
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Реальная проверка возможности кросс-компиляции в iOS.
    pub fn capability(&self) -> SwiftCapability {
        let swiftc_available = self.toolchain.swiftc.is_some();
        let sdk_present = self.sdk.as_ref().map(|s| s.valid).unwrap_or(false);
        let sdk_frameworks_complete = self
            .sdk
            .as_ref()
            .map(|s| s.required_frameworks_missing.is_empty())
            .unwrap_or(false);
        let swift_stdlib_iphoneos = self
            .toolchain
            .swiftc
            .as_ref()
            .and_then(|swiftc| find_ios_stdlib(swiftc));
        let mut diagnostics = Vec::new();
        let mut limitation = None;

        if !swiftc_available {
            diagnostics.push(
                Diagnostic::error("SWIFT_TOOLCHAIN_MISSING", "swiftc не найден в окружении")
                    .with_hint(
                        "Установите открытый Swift toolchain или задайте OPENIPA_SWIFT_TOOLCHAIN=/path/to/toolchain",
                    ),
            );
            limitation = Some("Swift compiler отсутствует.".into());
        }
        if !sdk_present {
            diagnostics.push(
                Diagnostic::error(
                    "IOS_SDK_MISSING",
                    "iPhoneOS.sdk не подключён или не прошёл проверку",
                )
                .with_hint("openipa sdk add /path/to/iPhoneOS.sdk"),
            );
            limitation = Some("Для iOS-сборки необходим легально полученный iPhoneOS.sdk.".into());
        }
        if swiftc_available && sdk_present {
            if let Some(stdlib) = &swift_stdlib_iphoneos {
                diagnostics.push(
                    Diagnostic::note(
                        "SWIFT_IOS_STDLIB_FOUND",
                        format!(
                            "найден Swift stdlib для iOS: {} — возможна настоящая компиляция",
                            stdlib.display()
                        ),
                    )
                    .with_file(stdlib),
                );
            } else {
                limitation = Some(
                    concat!(
                        "Открытый Swift toolchain для Linux не содержит Swift stdlib и module maps ",
                        "для target arm64-apple-ios и не может линковать SwiftUI/UIKit. Для настоящей ",
                        "Swift-сборки требуется toolchain, поставляемый с Xcode (только macOS)."
                    )
                    .into(),
                );
                diagnostics.push(
                    Diagnostic::error("SWIFT_IOS_CROSS_UNSUPPORTED", "в toolchain нет Swift stdlib для arm64-apple-ios")
                        .with_detail(limitation.clone().unwrap())
                        .with_hint("Укажите Xcode toolchain через OPENIPA_SWIFT_TOOLCHAIN или соберите C/Objective-C через OpenIPA."),
                );
            }
        }

        let sdk_can_link = self.sdk.as_ref().map(|s| s.can_link).unwrap_or(false);
        let can_target_ios = swiftc_available && swift_stdlib_iphoneos.is_some() && sdk_can_link;
        let can_build_swiftui = can_target_ios
            && self
                .sdk
                .as_ref()
                .map(|s| s.can_compile_swift_ios)
                .unwrap_or(false);
        if sdk_frameworks_complete {
            diagnostics.push(Diagnostic::note(
                "FRAMEWORKS_PRESENT",
                "в SDK найдены системные Frameworks (настоящие, не заглушки)",
            ));
        }

        SwiftCapability {
            swiftc_available,
            swiftc_path: self.toolchain.swiftc.clone(),
            swift_version: self.toolchain.swift_version.clone(),
            sdk_present,
            sdk_frameworks_complete,
            swift_stdlib_iphoneos,
            can_target_ios,
            can_build_swiftui,
            limitation,
            diagnostics,
        }
    }

    /// Пытается выполнить реальную компиляцию и возвращает честную ошибку
    /// компилятора. Никогда не подменяет успех.
    pub fn try_compile(
        &self,
        sources: &[PathBuf],
        output: &PathBuf,
    ) -> anyhow::Result<std::process::Output> {
        let swiftc = self
            .toolchain
            .swiftc
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("swiftc не найден"))?;
        let mut cmd = Command::new(swiftc);
        cmd.arg("-target").arg("arm64-apple-ios");
        if let Some(sdk) = &self.sdk {
            cmd.arg("-sdk").arg(&sdk.path);
        }
        for s in sources {
            cmd.arg(s);
        }
        cmd.arg("-o").arg(output);
        if self.verbose {
            eprintln!("[swiftc] {cmd:?}");
        }
        Ok(cmd.output()?)
    }

    /// Компилирует один Swift-файл в настоящий объектный файл через swiftc.
    pub fn compile_object(
        &self,
        source: &std::path::Path,
        output: &std::path::Path,
    ) -> anyhow::Result<std::process::Output> {
        let swiftc = self
            .toolchain
            .swiftc
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("swiftc не найден"))?;
        let mut cmd = Command::new(swiftc);
        cmd.arg("-frontend")
            .arg("-c")
            .arg("-target")
            .arg("arm64-apple-ios")
            .arg("-emit-object")
            .arg(source)
            .arg("-o")
            .arg(output);
        if let Some(sdk) = &self.sdk {
            cmd.arg("-sdk").arg(&sdk.path);
        }
        if let Some(stdlib) = self.capability().swift_stdlib_iphoneos {
            cmd.arg("-I").arg(stdlib);
        }
        if self.verbose {
            eprintln!("[swiftc] {cmd:?}");
        }
        Ok(cmd.output()?)
    }
}
