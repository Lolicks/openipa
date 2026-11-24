//! Статическая проверка IPA. Не гарантирует запуск на устройстве — об этом
//! отдельно говорится в отчёте.

use crate::diagnostics::{Diagnostic, Severity};
use crate::macho::{parse_macho, MachOInfo};
use crate::packager::read_info_plist_from_ipa;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub id: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ipa: String,
    pub valid: bool,
    pub checks: Vec<ValidationCheck>,
    pub diagnostics: Vec<Diagnostic>,
    pub macho: Option<MachOInfo>,
    pub bundle_identifier: Option<String>,
    pub executable_name: Option<String>,
    pub min_os_version: Option<String>,
    pub disclaimer: String,
}

pub fn validate_ipa(ipa: &Path) -> Result<ValidationReport> {
    let mut checks = Vec::new();
    let mut diagnostics = Vec::new();
    let mut check = |id: &str, ok: bool, msg: String| {
        checks.push(ValidationCheck {
            id: id.into(),
            ok,
            message: msg.clone(),
        });
        if !ok {
            diagnostics.push(Diagnostic::error(id, msg));
        }
    };

    check(
        "IPA_EXISTS",
        ipa.is_file(),
        format!("IPA существует: {}", ipa.display()),
    );
    if !ipa.is_file() {
        return Ok(ValidationReport {
            ipa: ipa.display().to_string(),
            valid: false,
            checks,
            diagnostics,
            macho: None,
            bundle_identifier: None,
            executable_name: None,
            min_os_version: None,
            disclaimer: DISCLAIMER.into(),
        });
    }

    let file = File::open(ipa)?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            check(
                "IPA_ZIP",
                false,
                format!("IPA не является корректным ZIP: {e}"),
            );
            return Ok(ValidationReport {
                ipa: ipa.display().to_string(),
                valid: false,
                checks,
                diagnostics,
                macho: None,
                bundle_identifier: None,
                executable_name: None,
                min_os_version: None,
                disclaimer: DISCLAIMER.into(),
            });
        }
    };
    check("IPA_ZIP", true, "IPA — корректный ZIP-архив".into());

    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .collect();
    let has_payload = names.iter().any(|n| n.starts_with("Payload/"));
    check(
        "PAYLOAD_DIR",
        has_payload,
        "архив содержит каталог Payload/".into(),
    );

    let app_entries: Vec<&String> = names
        .iter()
        .filter(|n| n.starts_with("Payload/") && n.contains(".app/"))
        .collect();
    check(
        "APP_BUNDLE",
        !app_entries.is_empty(),
        format!("найден .app bundle ({} записей)", app_entries.len()),
    );

    let has_code_resources = names
        .iter()
        .any(|n| n.contains("_CodeSignature/") || n.ends_with("embedded.mobileprovision"));
    check(
        "UNSIGNED",
        !has_code_resources,
        if has_code_resources {
            "обнаружена подпись или provisioning profile — ожидался неподписанный IPA".into()
        } else {
            "подпись и provisioning profile отсутствуют (ожидается Sideloadly)".into()
        },
    );

    let plist = read_info_plist_from_ipa(ipa)?;
    let bundle_id = plist
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .map(str::to_string);
    let exe_name = plist
        .get("CFBundleExecutable")
        .and_then(|v| v.as_string())
        .map(str::to_string);
    let min_os = plist
        .get("MinimumOSVersion")
        .and_then(|v| v.as_string())
        .map(str::to_string);

    check(
        "INFOPLIST_IDENTIFIER",
        bundle_id.is_some(),
        format!("CFBundleIdentifier: {:?}", bundle_id),
    );
    check(
        "INFOPLIST_EXECUTABLE",
        exe_name.is_some(),
        format!("CFBundleExecutable: {:?}", exe_name),
    );
    check(
        "INFOPLIST_MIOS",
        min_os.is_some(),
        format!("MinimumOSVersion: {:?}", min_os),
    );
    let supported_ios = plist
        .get("CFBundleSupportedPlatforms")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().any(|v| v.as_string() == Some("iPhoneOS")))
        .unwrap_or(false);
    check(
        "INFOPLIST_PLATFORM",
        supported_ios,
        "CFBundleSupportedPlatforms содержит iPhoneOS".into(),
    );
    let arm64_only = plist
        .get("UIRequiredDeviceCapabilities")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().any(|v| v.as_string() == Some("arm64")))
        .unwrap_or(false);
    check(
        "INFOPLIST_ARM64",
        arm64_only,
        "UIRequiredDeviceCapabilities содержит arm64".into(),
    );
    let bundle_id_valid = bundle_id
        .as_deref()
        .map(|id| !id.is_empty() && id.split('.').all(|p| !p.is_empty()))
        .unwrap_or(false);
    check(
        "INFOPLIST_BUNDLE_ID_FORMAT",
        bundle_id_valid,
        format!("формат CFBundleIdentifier валиден: {bundle_id_valid}"),
    );

    let mut macho = None;
    if let Some(exe) = &exe_name {
        let pattern = format!(".app/{exe}");
        let entry_index = names.iter().position(|n| n.ends_with(&pattern));
        if let Some(idx) = entry_index {
            let mut entry = archive.by_index(idx)?;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            match parse_macho(&bytes) {
                Ok(info) => {
                    check(
                        "MACHO_EXECUTABLE",
                        info.filetype_name == "MH_EXECUTE",
                        format!("исполняемый файл — Mach-O {}", info.filetype_name),
                    );
                    check(
                        "MACHO_ARM64",
                        info.architecture == "arm64",
                        format!("архитектура: {}", info.architecture),
                    );
                    check(
                        "MACHO_LOAD_COMMANDS",
                        !info.load_commands.is_empty(),
                        format!("load commands: {}", info.ncmds),
                    );
                    diagnostics.push(Diagnostic::note(
                        "DYNAMIC_DEPENDENCIES",
                        format!("зависимости: {}", info.dylibs.join(", ")),
                    ));
                    macho = Some(info);
                }
                Err(e) => {
                    check("MACHO_EXECUTABLE", false, format!("не Mach-O: {e}"));
                }
            }
        } else {
            check(
                "MACHO_EXECUTABLE",
                false,
                format!("исполняемый файл {exe} не найден в IPA"),
            );
        }
    }

    let valid = !diagnostics.iter().any(|d| d.severity == Severity::Error);
    diagnostics.push(Diagnostic::warning("STATIC_ONLY", DISCLAIMER));

    Ok(ValidationReport {
        ipa: ipa.display().to_string(),
        valid,
        checks,
        diagnostics,
        macho,
        bundle_identifier: bundle_id,
        executable_name: exe_name,
        min_os_version: min_os,
        disclaimer: DISCLAIMER.into(),
    })
}

const DISCLAIMER: &str = "Статическая проверка не гарантирует успешную загрузку и запуск приложения на реальном iPhone. Требуется подпись через Sideloadly и проверка на устройстве.";
