//! Интеграционный тест полного конвейера без проприетарного SDK.
//! ВАЖНО: здесь проверяется структурная корректность (настоящий ARM64 Mach-O,
//! корректный ZIP/Info.plist), а не загружаемость на iOS. Это тестовый
//! фикстурный бинарник, не рабочее приложение.

use openipa::macho::parse_macho_file;
use openipa::packager::{default_info_plist, make_app_bundle, package_ipa, BundleOptions};
use openipa::validator::validate_ipa;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn find_ld64() -> Option<PathBuf> {
    for name in ["ld64.lld-22", "ld64.lld-21", "ld64.lld"] {
        if let Ok(p) = which::which(name) {
            return Some(p);
        }
    }
    None
}

#[test]
fn native_arm64_macho_to_ipa_pipeline() {
    let Some(ld64) = find_ld64() else {
        eprintln!("ld64.lld недоступен — интеграционный тест пропущен");
        return;
    };
    let Some(clang) = which::which("clang-22")
        .or_else(|_| which::which("clang-21"))
        .or_else(|_| which::which("clang"))
        .ok()
    else {
        eprintln!("clang недоступен — интеграционный тест пропущен");
        return;
    };

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let src = root.join("main.c");
    fs::write(&src, "int main(void) { return 0; }\n").unwrap();

    let obj = root.join("main.o");
    let out = Command::new(&clang)
        .args(["-target", "arm64-apple-ios", "-c"])
        .arg(&src)
        .arg("-o")
        .arg(&obj)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "clang: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let exe = root.join("Demo");
    let out = Command::new(&ld64)
        .args([
            "-arch",
            "arm64",
            "-platform_version",
            "ios",
            "16.0",
            "16.0",
            "-e",
            "_main",
            "-o",
        ])
        .arg(&exe)
        .arg(&obj)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "ld64.lld: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let info = parse_macho_file(&exe).unwrap();
    assert_eq!(info.architecture, "arm64");
    assert_eq!(info.filetype_name, "MH_EXECUTE");
    assert!(!info.load_commands.is_empty());

    let app = root.join("Demo.app");
    let plist = default_info_plist("Demo", "com.example.Demo", "16.0", "Demo");
    let options = BundleOptions {
        app_name: "Demo",
        executable: &exe,
        info_plist: &plist,
        resources: &[],
        project_root: root,
        frameworks: &[],
        plugins: &[],
    };
    let files = make_app_bundle(&app, &options).unwrap();
    assert!(files >= 2);

    let ipa = root.join("Demo.ipa");
    let report = package_ipa(&app, &ipa, "Demo").unwrap();
    assert!(report.unsigned);
    assert!(report
        .files
        .iter()
        .any(|f| f.starts_with("Payload/Demo.app/")));

    let validation = validate_ipa(&ipa).unwrap();
    for check in &validation.checks {
        assert!(check.ok, "check {} failed: {}", check.id, check.message);
    }
    assert!(validation.valid);
    assert_eq!(validation.executable_name.as_deref(), Some("Demo"));
    assert_eq!(
        validation.bundle_identifier.as_deref(),
        Some("com.example.Demo")
    );
}
