//! Менеджер iOS SDK. Не скачивает и не подделывает Apple SDK — только
//! регистрирует и проверяет SDK, легально предоставленный пользователем.

use crate::config::Config;
use crate::diagnostics::Diagnostic;
use anyhow::{bail, Context, Result};
use plist::Value as PlistValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkEntry {
    pub name: String,
    pub path: PathBuf,
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub canonical_name: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SdkRegistry {
    pub active: Option<String>,
    pub sdks: BTreeMap<String, SdkEntry>,
}

impl SdkRegistry {
    pub fn load(config: &Config) -> Result<Self> {
        let path = config.sdk_registry_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("не удалось прочитать {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("битый реестр SDK: {}", path.display()))
    }
    pub fn save(&self, config: &Config) -> Result<()> {
        config.ensure_dirs()?;
        let path = config.sdk_registry_path();
        fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("не удалось записать {}", path.display()))?;
        Ok(())
    }
    pub fn resolve(&self, name: &str, _config: &Config) -> Result<SdkEntry> {
        if name.is_empty() {
            let active = self.active.as_deref().ok_or_else(|| {
                anyhow::anyhow!("активный SDK не назначен. Используйте `openipa sdk use <name>`")
            })?;
            return self
                .sdks
                .get(active)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("активный SDK '{active}' отсутствует в реестре"));
        }
        if let Some(entry) = self.sdks.get(name) {
            return Ok(entry.clone());
        }
        if Path::new(name).exists() {
            return Ok(SdkEntry {
                name: Path::new(name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("sdk")
                    .to_string(),
                path: PathBuf::from(name),
                version: None,
                display_name: None,
                canonical_name: None,
                added_at: String::new(),
            });
        }
        bail!("SDK '{}' не найден в реестре", name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkReport {
    pub path: PathBuf,
    pub valid: bool,
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub canonical_name: Option<String>,
    pub min_deployment_target: Option<String>,
    pub frameworks_found: Vec<String>,
    pub framework_binaries: Vec<(String, PathBuf)>,
    pub required_frameworks_missing: Vec<String>,
    pub libsystem: Option<PathBuf>,
    pub swift_stdlib_iphoneos: Option<PathBuf>,
    pub can_link: bool,
    pub can_compile_swift_ios: bool,
    pub checks: Vec<SdkCheck>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

const REQUIRED_FRAMEWORKS: &[&str] = &[
    "Foundation.framework",
    "UIKit.framework",
    "CoreGraphics.framework",
];

/// Проверяет структуру каталога SDK. Ничего не скачивает и не создаёт.
pub fn inspect_sdk(path: &Path) -> Result<SdkReport> {
    let mut checks = Vec::new();
    let mut diagnostics = Vec::new();
    let mut version = None;
    let mut display_name = None;
    let mut canonical_name = None;
    let mut min_deployment_target = None;

    let push = |checks: &mut Vec<SdkCheck>, name: &str, ok: bool, detail: String| {
        checks.push(SdkCheck {
            name: name.into(),
            ok,
            detail,
        });
    };

    let exists = path.is_dir();
    push(
        &mut checks,
        "directory",
        exists,
        format!("каталог SDK: {}", path.display()),
    );
    if !exists {
        diagnostics.push(
            Diagnostic::error("SDK_MISSING", "каталог SDK не существует")
                .with_detail(path.display().to_string())
                .with_hint(
                    "Укажите путь к легально полученному iPhoneOS.sdk: openipa sdk add /path/to/iPhoneOS.sdk",
                ),
        );
        return Ok(SdkReport {
            path: path.to_path_buf(),
            valid: false,
            version,
            display_name,
            canonical_name,
            min_deployment_target,
            frameworks_found: vec![],
            framework_binaries: vec![],
            required_frameworks_missing: REQUIRED_FRAMEWORKS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            libsystem: None,
            swift_stdlib_iphoneos: None,
            can_link: false,
            can_compile_swift_ios: false,
            checks,
            diagnostics,
        });
    }

    // SDKSettings.plist — обязательный маркер настоящего Apple SDK.
    let settings_candidates = [path.join("SDKSettings.plist"), path.join("Settings.plist")];
    let mut settings = None;
    for candidate in &settings_candidates {
        if candidate.exists() {
            match plist::Value::from_file(candidate) {
                Ok(v) => {
                    settings = Some(v);
                    break;
                }
                Err(e) => diagnostics.push(
                    Diagnostic::warning(
                        "SDK_SETTINGS_UNREADABLE",
                        "не удалось прочитать SDKSettings.plist",
                    )
                    .with_detail(e.to_string())
                    .with_file(candidate),
                ),
            }
        }
    }
    push(
        &mut checks,
        "sdksettings",
        settings.is_some(),
        if settings.is_some() {
            "SDKSettings.plist найден".into()
        } else {
            "SDKSettings.plist отсутствует — это не похоже на Apple SDK".into()
        },
    );
    if settings.is_none() {
        diagnostics.push(Diagnostic::error(
            "SDK_NOT_APPLE",
            "не найден SDKSettings.plist — каталог не является корректным iPhoneOS.sdk",
        ));
    } else if let Some(v) = &settings {
        if let Some(dict) = v.as_dictionary() {
            version = get_string(dict, "Version");
            display_name = get_string(dict, "DisplayName");
            canonical_name = get_string(dict, "CanonicalName");
            if let Some(PlistValue::Dictionary(props)) = dict.get("DefaultProperties") {
                min_deployment_target = get_string(props, "IPHONEOS_DEPLOYMENT_TARGET");
            }
            if let Some(PlistValue::Dictionary(props)) = dict.get("Properties") {
                if min_deployment_target.is_none() {
                    min_deployment_target = get_string(props, "IPHONEOS_DEPLOYMENT_TARGET");
                }
            }
        }
    }

    let frameworks_dir = path.join("System/Library/Frameworks");
    let frameworks = list_dir_names(&frameworks_dir);
    push(
        &mut checks,
        "frameworks_dir",
        frameworks_dir.is_dir(),
        format!("frameworks: {}", frameworks_dir.display()),
    );

    let mut found = Vec::new();
    let mut missing = Vec::new();
    for required in REQUIRED_FRAMEWORKS {
        if frameworks.iter().any(|f| f == required) {
            found.push(required.to_string());
        } else {
            missing.push(required.to_string());
        }
    }
    push(
        &mut checks,
        "required_frameworks",
        missing.is_empty(),
        format!(
            "найдено {}/{} обязательных framework",
            found.len(),
            REQUIRED_FRAMEWORKS.len()
        ),
    );
    for m in &missing {
        diagnostics.push(
            Diagnostic::error("SDK_FRAMEWORK_MISSING", format!("отсутствует {m}")).with_hint(
                "Используйте полный iPhoneOS.sdk из Xcode; урезанные SDK не поддерживаются.",
            ),
        );
    }

    let usr_lib = path.join("usr/lib");
    push(
        &mut checks,
        "usr_lib",
        usr_lib.is_dir(),
        format!("system libraries: {}", usr_lib.display()),
    );
    let usr_include = path.join("usr/include");
    push(
        &mut checks,
        "usr_include",
        usr_include.is_dir(),
        format!("headers: {}", usr_include.display()),
    );

    let has_swiftui = frameworks.iter().any(|f| f == "SwiftUI.framework");
    let has_uikit = frameworks.iter().any(|f| f == "UIKit.framework");
    push(
        &mut checks,
        "ui_frameworks",
        has_swiftui && has_uikit,
        if has_swiftui && has_uikit {
            "SwiftUI и UIKit присутствуют".into()
        } else {
            format!(
                "SwiftUI: {has_swiftui}, UIKit: {has_uikit} — для SwiftUI/UIKit нужен полный SDK"
            )
        },
    );

    // Бинарники framework: линкер требует реальный .tbd/.dylib/ binary,
    // одного пустого каталога .framework недостаточно.
    let mut framework_binaries = Vec::new();
    for fw in &frameworks {
        let fw_dir = path.join("System/Library/Frameworks").join(fw);
        let stem = fw.trim_end_matches(".framework");
        let candidates = [
            fw_dir.join(stem),
            fw_dir.join(format!("{stem}.tbd")),
            fw_dir.join("Versions/A").join(stem),
        ];
        if let Some(binary) = candidates.into_iter().find(|p| p.exists()) {
            framework_binaries.push((fw.clone(), binary));
        }
    }
    push(
        &mut checks,
        "framework_binaries",
        !framework_binaries.is_empty(),
        format!(
            "найдено {} framework binary/tbd из {}",
            framework_binaries.len(),
            frameworks.len()
        ),
    );

    let libsystem_candidates = [
        path.join("usr/lib/libSystem.tbd"),
        path.join("usr/lib/libSystem.dylib"),
        path.join("usr/lib/libSystem.B.tbd"),
        path.join("usr/lib/libSystem.B.dylib"),
    ];
    let libsystem = libsystem_candidates.into_iter().find(|p| p.exists());
    push(
        &mut checks,
        "libsystem",
        libsystem.is_some(),
        match &libsystem {
            Some(p) => format!("libSystem: {}", p.display()),
            None => "libSystem не найден — ld64.lld не сможет слинковать iOS-приложение".into(),
        },
    );
    if libsystem.is_none() {
        diagnostics.push(Diagnostic::error(
            "SDK_LIBSYSTEM_MISSING",
            "в SDK нет libSystem (.tbd/.dylib) — линковка невозможна",
        ));
    }

    let swift_stdlib = path.join("usr/lib/swift/iphoneos");
    let swift_stdlib = swift_stdlib.exists().then_some(swift_stdlib);
    push(
        &mut checks,
        "swift_stdlib_iphoneos",
        swift_stdlib.is_some(),
        match &swift_stdlib {
            Some(p) => format!("Swift stdlib для iOS: {}", p.display()),
            None => "Swift stdlib для iOS отсутствует (нужен toolchain от Xcode)".into(),
        },
    );

    let can_link = libsystem.is_some() && missing.is_empty();
    let can_compile_swift_ios = can_link && swift_stdlib.is_some() && has_swiftui && has_uikit;
    // Swift stdlib нужен только для Swift-сборки; для C/Objective-C он необязателен.
    let valid = checks
        .iter()
        .filter(|c| c.name != "swift_stdlib_iphoneos")
        .all(|c| c.ok);
    if valid {
        diagnostics.push(Diagnostic::note(
            "SDK_OK",
            "SDK прошёл структурную проверку и пригоден для линковки",
        ));
    }

    Ok(SdkReport {
        path: path.to_path_buf(),
        valid,
        version,
        display_name,
        canonical_name,
        min_deployment_target,
        frameworks_found: frameworks,
        framework_binaries,
        required_frameworks_missing: missing,
        libsystem,
        swift_stdlib_iphoneos: swift_stdlib,
        can_link,
        can_compile_swift_ios,
        checks,
        diagnostics,
    })
}

/// Сравнивает запрошенный deployment target с минимумом, объявленным в SDK.
pub fn check_deployment_target(report: &SdkReport, requested: &str) -> Result<()> {
    let parse = |v: &str| -> Option<(u32, u32)> {
        let mut parts = v.trim().split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        Some((major, minor))
    };
    if let (Some(requested), Some(minimum)) = (
        parse(requested),
        report.min_deployment_target.as_deref().and_then(parse),
    ) {
        if requested < minimum {
            anyhow::bail!(
                "deployment target {}.{} ниже минимума SDK {}.{} ({})",
                requested.0,
                requested.1,
                minimum.0,
                minimum.1,
                report.path.display()
            );
        }
    }
    Ok(())
}

fn get_string(dict: &plist::Dictionary, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(|v| v.as_string())
        .map(str::to_string)
}

fn list_dir_names(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}
