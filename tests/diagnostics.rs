//! Интеграционные тесты Diagnostic Engine на намеренно испорченных проектах.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn broken_project(tmp: &Path) -> PathBuf {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/CApp");
    let root = tmp.join("Broken");
    fs::create_dir_all(&root).unwrap();
    copy_dir(&base.join("CApp.xcodeproj"), &root.join("Broken.xcodeproj"));
    copy_dir(&base.join("CApp"), &root.join("Broken"));
    let pbx = root.join("Broken.xcodeproj/project.pbxproj");
    let text = fs::read_to_string(&pbx).unwrap();
    fs::write(&pbx, text.replace("main.c", "deleted_main.c")).unwrap();
    root.join("Broken.xcodeproj")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_openipa"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn check_reports_missing_source_before_compilation() {
    let tmp = tempfile::tempdir().unwrap();
    let project = broken_project(tmp.path());
    let out = run(&["check", project.to_str().unwrap(), "--json", "--no-color"]);
    assert!(
        out.status.success() || !out.status.success(),
        "check должен выполниться"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let codes: Vec<&str> = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert!(codes.contains(&"E0002"), "нет E0002 в {codes:?}");
    let item = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "E0002")
        .unwrap();
    assert_eq!(item["severity"], "error");
    assert!(item.get("line").is_some());
    assert!(item.get("source_line").is_some());
    assert!(item.get("help").is_some());

    // Сборка должна остановиться до появления объектных файлов.
    let build = run(&["build", project.to_str().unwrap(), "--no-color"]);
    assert!(!build.status.success());
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(stderr.contains("E0002"), "нет E0002 в выводе сборки");
    assert!(!tmp.path().join("Broken/build/objects").exists());
    assert!(!tmp.path().join("Broken/build/Broken.ipa").exists());
}

#[test]
fn strict_promotes_unknown_setting_to_error() {
    let tmp = tempfile::tempdir().unwrap();
    let project = broken_project(tmp.path());
    let pbx = project.join("project.pbxproj");
    let text = fs::read_to_string(&pbx).unwrap();
    fs::write(
        &pbx,
        text.replace(
            "PRODUCT_NAME = \"$(TARGET_NAME)\";",
            "PRODUCT_NAME = \"$(TARGET_NAME)\";\n\t\t\tMYSTERY_SETTING = yes;",
        ),
    )
    .unwrap();

    let normal = run(&["check", project.to_str().unwrap(), "--json", "--no-color"]);
    let normal_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&normal.stdout)).unwrap();
    assert!(normal_json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "W0001"));

    let strict = run(&["check", project.to_str().unwrap(), "--strict", "--no-color"]);
    assert!(!strict.status.success());
    let stderr = String::from_utf8_lossy(&strict.stderr);
    assert!(stderr.contains("error[W_AS_ERROR(W0001)]"), "{stderr}");
}

#[test]
fn explain_catalog_code() {
    let out = run(&["explain", "E0102", "--no-color"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[E0102]"));
    assert!(stdout.contains("required framework not found"));
    assert!(stdout.contains("iPhoneOS.sdk"));

    let unknown = run(&["explain", "E9999", "--no-color"]);
    assert!(!unknown.status.success());
}

#[test]
fn check_renders_cargo_style_snippet() {
    let tmp = tempfile::tempdir().unwrap();
    let project = broken_project(tmp.path());
    let out = run(&["check", project.to_str().unwrap(), "--no-color"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("error[E0002]"), "{stderr}");
    assert!(stderr.contains("-->"), "{stderr}");
    assert!(stderr.contains("deleted_main.c"), "{stderr}");
    assert!(stderr.contains("^^^^"), "{stderr}");
}
