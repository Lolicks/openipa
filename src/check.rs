//! Pre-build Analyzer: все проверки, которые можно выполнить до запуска
//! Clang/Swift/ld64.lld. Не выдаёт неподтверждённых ошибок — только то,
//! что реально проверено по проекту, SDK и toolchain.

use crate::compiler::SwiftCapability;
use crate::diagnostics::Diagnostic;
use crate::project::{Target, XcodeProject};
use crate::sdk::{check_deployment_target, SdkReport};
use crate::toolchain::Toolchain;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct CheckOptions<'a> {
    pub project: &'a XcodeProject,
    pub target: &'a Target,
    pub configuration: &'a str,
    pub project_root: &'a Path,
    pub sdk_report: Option<&'a SdkReport>,
    pub toolchain: &'a Toolchain,
    pub swift_capability: Option<&'a SwiftCapability>,
}

fn locate(path: &Path, needle: &str) -> Option<(usize, usize, String, String)> {
    let text = fs::read_to_string(path).ok()?;
    for (index, line) in text.lines().enumerate() {
        if let Some(column) = line.find(needle) {
            return Some((index + 1, column + 1, line.to_string(), needle.to_string()));
        }
    }
    None
}

fn located(mut diagnostic: Diagnostic, path: &Path, needle: &str) -> Diagnostic {
    match locate(path, needle) {
        Some((line, column, source, label)) => {
            diagnostic = diagnostic.with_location(path, line, column, source, label)
        }
        None => diagnostic = diagnostic.with_file(path),
    }
    diagnostic
}

fn resolve(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn parse_version(value: &str) -> Option<(u32, u32)> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    Some((major, minor))
}

pub fn analyze(options: &CheckOptions<'_>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let pbxproj = options.project.path.join("project.pbxproj");
    let settings = options
        .target
        .configuration(options.configuration)
        .map(|c| &c.settings);

    // Структура Xcode-проекта.
    for warning in &options.project.parse_warnings {
        out.push(located(
            Diagnostic::error("E0401", warning)
                .with_hint("перечислите исходные файлы явно в PBXSourcesBuildPhase"),
            &pbxproj,
            "PBXFileSystemSynchronizedRootGroup",
        ));
    }

    // Исходники и ресурсы.
    for source in &options.target.sources {
        let full = resolve(options.project_root, source);
        if !full.exists() {
            out.push(located(
                Diagnostic::error(
                    "E0002",
                    format!("исходный файл не найден: {}", source.display()),
                )
                .with_hint("создайте файл или удалите ссылку из Xcode-проекта"),
                &pbxproj,
                source.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            ));
        }
    }
    for resource in &options.target.resources {
        let full = resolve(options.project_root, resource);
        if !full.exists() {
            out.push(located(
                Diagnostic::warning("W0002", format!("ресурс не найден: {}", resource.display()))
                    .with_hint("восстановите файл или удалите ссылку"),
                &pbxproj,
                resource.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            ));
        }
    }

    if let Some(settings) = settings {
        // Build settings.
        for (key, value) in &settings.unknown {
            out.push(located(
                Diagnostic::warning(
                    "W0001",
                    format!("неизвестная build setting {key} = {value}"),
                )
                .with_note("настройка будет проигнорирована"),
                &pbxproj,
                key,
            ));
        }
        for (key, value) in &settings.raw {
            if value.contains("$(") && !value.contains("$(inherited)") {
                out.push(located(
                    Diagnostic::warning(
                        "W0003",
                        format!("не раскрыта переменная в {key}: {value}"),
                    )
                    .with_hint("определите переменную в .xcconfig"),
                    &pbxproj,
                    key,
                ));
            }
        }
        if let Some(min) = &settings.iphoneos_deployment_target {
            if parse_version(min).is_none() {
                out.push(located(
                    Diagnostic::error(
                        "E0003",
                        format!("IPHONEOS_DEPLOYMENT_TARGET имеет неверный формат: {min}"),
                    )
                    .with_hint("укажите версию вида 16.0"),
                    &pbxproj,
                    "IPHONEOS_DEPLOYMENT_TARGET",
                ));
            }
        }
        if let Some(bundle_id) = &settings.product_bundle_identifier {
            if bundle_id.is_empty()
                || bundle_id
                    .chars()
                    .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
            {
                out.push(located(
                    Diagnostic::error(
                        "E0003",
                        format!("некорректный PRODUCT_BUNDLE_IDENTIFIER: {bundle_id}"),
                    )
                    .with_hint("используйте формат com.example.App"),
                    &pbxproj,
                    "PRODUCT_BUNDLE_IDENTIFIER",
                ));
            }
        }
        if let Some(archs) = settings.raw.get("ARCHS") {
            if !archs.contains("arm64") {
                out.push(located(
                    Diagnostic::error("E0003", "ARCHS не содержит arm64")
                        .with_hint("добавьте arm64 в ARCHS"),
                    &pbxproj,
                    "ARCHS",
                ));
            }
        }
    }

    // Toolchain.
    if !options.toolchain.can_compile_c() {
        out.push(
            Diagnostic::error("E0502", "clang не найден в окружении")
                .with_hint("установите Clang/LLVM с поддержкой AArch64"),
        );
    }
    if !options.toolchain.can_link_macho() {
        out.push(
            Diagnostic::error("E0501", "ld64.lld не найден в окружении")
                .with_hint("apt install lld / choco install llvm"),
        );
    }

    // SDK.
    match options.sdk_report {
        None => out.push(
            Diagnostic::error("E0101", "iPhoneOS.sdk не подключён")
                .with_hint("openipa sdk add /path/to/iPhoneOS.sdk"),
        ),
        Some(report) => {
            if !report.valid {
                out.push(
                    Diagnostic::error("E0101", "iPhoneOS.sdk не прошёл проверку")
                        .with_file(&report.path)
                        .with_hint("используйте полный SDK из Xcode"),
                );
            }
            if report.version.as_deref().and_then(parse_version).is_none() {
                out.push(
                    Diagnostic::error("E0103", "версия SDK не может быть прочитана")
                        .with_file(&report.path),
                );
            }
            if report.libsystem.is_none() {
                out.push(
                    Diagnostic::error("E0104", "в SDK отсутствует libSystem")
                        .with_file(&report.path)
                        .with_hint("подключите полный iPhoneOS.sdk"),
                );
            }
            if let Some(min) = settings.and_then(|s| s.iphoneos_deployment_target.as_deref()) {
                if let Err(e) = check_deployment_target(report, min) {
                    out.push(located(
                        Diagnostic::error("E0105", e.to_string()),
                        &pbxproj,
                        "IPHONEOS_DEPLOYMENT_TARGET",
                    ));
                }
            }

            // frameworks: из build phase и из OTHER_LDFLAGS.
            let mut frameworks: Vec<String> = options
                .target
                .frameworks
                .iter()
                .map(|f| {
                    Path::new(f)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(f)
                        .trim_end_matches(".framework")
                        .to_string()
                })
                .collect();
            if let Some(flags) = settings.map(|s| &s.other_ldflags) {
                let mut iter = flags.iter();
                while let Some(flag) = iter.next() {
                    if flag == "-framework" {
                        if let Some(name) = iter.next() {
                            frameworks.push(name.trim_end_matches(".framework").to_string());
                        }
                    }
                }
            }
            let framework_root = report.path.join("System/Library/Frameworks");
            for framework in frameworks {
                if framework.is_empty() {
                    continue;
                }
                let dir = framework_root.join(format!("{framework}.framework"));
                if !dir.exists() {
                    out.push(located(
                        Diagnostic::error(
                            "E0102",
                            format!("required framework not found: {framework}.framework"),
                        )
                        .with_hint("install a complete iOS SDK")
                        .with_note(format!("искалось: {}", dir.display())),
                        &pbxproj,
                        &framework,
                    ));
                }
            }

            // Swift UI framework presence.
            let swiftui_missing = !framework_root.join("SwiftUI.framework").exists();
            if swiftui_missing && !options.target.swift_sources().is_empty() {
                for source in options.target.swift_sources() {
                    let full = resolve(options.project_root, source);
                    if let Ok(text) = fs::read_to_string(&full) {
                        if text.contains("import SwiftUI") {
                            out.push(located(
                                Diagnostic::warning("W0008", "SwiftUI.framework отсутствует в SDK")
                                    .with_hint("используйте UIKit или полный Xcode SDK"),
                                &pbxproj,
                                "SwiftUI",
                            ));
                            break;
                        }
                    }
                }
            }
        }
    }

    // Swift toolchain.
    if !options.target.swift_sources().is_empty() {
        if let Some(capability) = options.swift_capability {
            for diagnostic in &capability.diagnostics {
                let code = if diagnostic.code == "SWIFT_TOOLCHAIN_MISSING" {
                    "E0202"
                } else if diagnostic.code == "SWIFT_IOS_CROSS_UNSUPPORTED" {
                    "E0201"
                } else {
                    continue;
                };
                out.push(Diagnostic::new_cloned(diagnostic, code));
            }
        } else if !options.toolchain.can_compile_swift() {
            out.push(Diagnostic::error(
                "E0202",
                "swiftc не найден, а проект содержит Swift-исходники",
            ));
        }
    }

    // Asset catalogs.
    for resource in &options.target.resources {
        let full = resolve(options.project_root, resource);
        if !full.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&full).max_depth(3) {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) != Some("Contents.json") {
                continue;
            }
            let text = match fs::read_to_string(path) {
                Ok(text) => text,
                Err(e) => {
                    out.push(
                        Diagnostic::error(
                            "E0301",
                            format!("не удалось прочитать {}: {e}", path.display()),
                        )
                        .with_file(path),
                    );
                    continue;
                }
            };
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(e) => {
                    let line = e.line();
                    let column = e.column();
                    let source = text
                        .lines()
                        .nth(line.saturating_sub(1))
                        .unwrap_or("")
                        .to_string();
                    out.push(
                        Diagnostic::error("E0301", format!("invalid Contents.json: {e}"))
                            .with_location(path, line, column, source, "invalid JSON")
                            .with_hint("исправьте JSON в Xcode"),
                    );
                    continue;
                }
            };
            if let Some(images) = json.get("images").and_then(|v| v.as_array()) {
                for image in images {
                    if let Some(name) = image.get("filename").and_then(|v| v.as_str()) {
                        let asset = path.parent().unwrap_or(path).join(name);
                        if !asset.exists() {
                            out.push(
                                Diagnostic::error(
                                    "E0303",
                                    format!("файл ассета не найден: {}", asset.display()),
                                )
                                .with_file(path)
                                .with_hint("добавьте файл или уберите запись из Contents.json"),
                            );
                        }
                    }
                }
            }
        }
        if full.file_name().and_then(|n| n.to_str()) == Some("Assets.xcassets") {
            out.push(
                Diagnostic::warning(
                    "W0302",
                    "Assets.car не генерируется: требуется actool из Xcode",
                )
                .with_file(&full)
                .with_hint("ресурсы будут скопированы как обычные файлы"),
            );
        }
    }

    out.sort_by_key(|d| {
        (
            std::cmp::Reverse(d.severity),
            d.code.clone(),
            d.file.clone(),
        )
    });
    out.dedup_by(|a, b| {
        a.code == b.code && a.message == b.message && a.file == b.file && a.line == b.line
    });
    out
}
