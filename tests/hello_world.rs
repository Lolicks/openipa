//! Сквозной тест: HelloWorldApp → настоящий ARM64 Mach-O UIKit-бинарник → IPA.
//! Использует тестовую SDK-фикстуру с link-time TBD-стабами (не Apple SDK).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tbd(path: &Path, install: &str, symbols: &[&str]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = format!(
        "--- !tapi-tbd-v3\narchs: [ arm64 ]\nplatform: ios\ninstall-name: {install}\nexports:\n  - archs: [ arm64 ]\n    symbols: [\n"
    );
    for s in symbols {
        text.push_str(&format!("      {s},\n"));
    }
    text.push_str("    ]\n...\n");
    fs::write(path, text).unwrap();
}

fn make_sdk(root: &Path) -> PathBuf {
    let sdk = root.join("iPhoneOS.sdk");
    let frameworks = sdk.join("System/Library/Frameworks");
    tbd(
        &frameworks.join("UIKit.framework/UIKit.tbd"),
        "/System/Library/Frameworks/UIKit.framework/UIKit",
        &[
            "_UIApplicationMain",
            "_UIGraphicsGetCurrentContext",
            "_OBJC_CLASS_$_UIButton",
            "_OBJC_CLASS_$_UIColor",
            "_OBJC_CLASS_$_UIFont",
            "_OBJC_CLASS_$_UIImage",
            "_OBJC_CLASS_$_UIImageView",
            "_OBJC_CLASS_$_UILabel",
            "_OBJC_CLASS_$_UIBlurEffect",
            "_OBJC_CLASS_$_UINavigationController",
            "_OBJC_CLASS_$_UIVisualEffectView",
            "_OBJC_CLASS_$_UIView",
            "_OBJC_METACLASS_$_UIView",
            "_OBJC_CLASS_$_UIResponder",
            "_OBJC_METACLASS_$_UIResponder",
            "_OBJC_CLASS_$_UIScreen",
            "_OBJC_CLASS_$_UIViewController",
            "_OBJC_METACLASS_$_UIViewController",
            "_OBJC_CLASS_$_UIWindow",
        ],
    );
    tbd(
        &frameworks.join("Foundation.framework/Foundation.tbd"),
        "/System/Library/Frameworks/Foundation.framework/Foundation",
        &[
            "_OBJC_CLASS_$_NSObject",
            "_OBJC_METACLASS_$_NSObject",
            "_OBJC_CLASS_$_NSBundle",
            "_OBJC_CLASS_$_NSString",
            "_OBJC_CLASS_$_NSTimer",
        ],
    );
    tbd(
        &frameworks.join("CoreFoundation.framework/CoreFoundation.tbd"),
        "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation",
        &["___CFConstantStringClassReference"],
    );
    tbd(
        &frameworks.join("CoreGraphics.framework/CoreGraphics.tbd"),
        "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics",
        &[
            "_CGContextAddLineToPoint",
            "_CGContextClosePath",
            "_CGContextFillEllipseInRect",
            "_CGContextFillPath",
            "_CGContextFillRect",
            "_CGContextMoveToPoint",
            "_CGContextSetAlpha",
            "_CGContextSetFillColorWithColor",
            "_CGContextSetRGBFillColor",
        ],
    );
    tbd(
        &frameworks.join("SwiftUI.framework/SwiftUI.tbd"),
        "/System/Library/Frameworks/SwiftUI.framework/SwiftUI",
        &[],
    );
    tbd(
        &sdk.join("usr/lib/libSystem.tbd"),
        "/usr/lib/libSystem.B.dylib",
        &["___stack_chk_fail", "___stack_chk_guard", "_memcpy"],
    );
    tbd(
        &sdk.join("usr/lib/libobjc.tbd"),
        "/usr/lib/libobjc.A.dylib",
        &[
            "_objc_msgSend",
            "_objc_msgSendSuper2",
            "_objc_alloc",
            "_objc_alloc_init",
            "__objc_empty_cache",
            "_objc_storeStrong",
            "_objc_retainAutoreleasedReturnValue",
        ],
    );
    fs::create_dir_all(sdk.join("usr/include")).unwrap();
    fs::create_dir_all(sdk.join("usr/lib/swift/iphoneos")).unwrap();
    fs::write(
        sdk.join("SDKSettings.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CanonicalName</key><string>iphoneos</string>
<key>Version</key><string>17.0</string>
<key>DefaultProperties</key><dict><key>IPHONEOS_DEPLOYMENT_TARGET</key><string>12.0</string></dict>
</dict></plist>"#,
    )
    .unwrap();
    sdk
}

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

#[test]
fn hello_world_uikit_app_builds_and_validates() {
    if which::which("ld64.lld-22").is_err()
        && which::which("ld64.lld-21").is_err()
        && which::which("ld64.lld").is_err()
    {
        eprintln!("ld64.lld недоступен — тест пропущен");
        return;
    }
    let bin = env!("CARGO_BIN_EXE_openipa");
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let sdk = make_sdk(tmp.path());

    let source_project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/HelloWorldApp/HelloWorldApp.xcodeproj");
    let project = tmp.path().join("HelloWorldApp.xcodeproj");
    copy_dir(&source_project, &project);
    let source_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/HelloWorldApp/HelloWorldApp");
    copy_dir(&source_dir, &tmp.path().join("HelloWorldApp"));

    let add = Command::new(bin)
        .env("HOME", &home)
        .args([
            "sdk",
            "add",
            sdk.to_str().unwrap(),
            "--name",
            "test-ios",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "sdk add: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let build = Command::new(bin)
        .env("HOME", &home)
        .current_dir(tmp.path())
        .args([
            "build",
            "HelloWorldApp.xcodeproj",
            "--configuration",
            "release",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let ipa = tmp.path().join("build/HelloWorldApp.ipa");
    assert!(ipa.exists(), "IPA не создан");

    let validate = Command::new(bin)
        .args(["validate", ipa.to_str().unwrap(), "--no-color"])
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "validate: {}",
        String::from_utf8_lossy(&validate.stderr)
    );

    let exe = tmp.path().join("build/HelloWorldApp");
    let info = openipa::macho::parse_macho_file(&exe).unwrap();
    assert_eq!(info.architecture, "arm64");
    assert_eq!(info.filetype_name, "MH_EXECUTE");
    assert!(info.dylibs.iter().any(|d| d.contains("UIKit")));
    assert!(info.dylibs.iter().any(|d| d.contains("libobjc")));
}

#[test]
fn uikit_test_app_builds_with_resources() {
    if which::which("ld64.lld-22").is_err()
        && which::which("ld64.lld-21").is_err()
        && which::which("ld64.lld").is_err()
    {
        eprintln!("ld64.lld недоступен — тест пропущен");
        return;
    }
    let bin = env!("CARGO_BIN_EXE_openipa");
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let sdk = make_sdk(tmp.path());

    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/UIKitTestApp");
    copy_dir(
        &base.join("UIKitTestApp.xcodeproj"),
        &tmp.path().join("UIKitTestApp.xcodeproj"),
    );
    copy_dir(&base.join("UIKitTestApp"), &tmp.path().join("UIKitTestApp"));

    let add = Command::new(bin)
        .env("HOME", &home)
        .args([
            "sdk",
            "add",
            sdk.to_str().unwrap(),
            "--name",
            "test-ios",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "sdk add: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let build = Command::new(bin)
        .env("HOME", &home)
        .current_dir(tmp.path())
        .args([
            "build",
            "UIKitTestApp.xcodeproj",
            "--configuration",
            "release",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let ipa = tmp.path().join("build/UIKitTestApp.ipa");
    assert!(ipa.exists());
    let file = fs::File::open(&ipa).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .collect();
    assert!(
        names.iter().any(|n| n.ends_with("banner.png")),
        "нет banner.png"
    );
    assert!(
        names.iter().any(|n| n.ends_with("message.txt")),
        "нет message.txt"
    );

    let info = openipa::macho::parse_macho_file(&tmp.path().join("build/UIKitTestApp")).unwrap();
    assert_eq!(info.architecture, "arm64");
    assert!(info.dylibs.iter().any(|d| d.contains("UIKit")));
    assert!(info.dylibs.iter().any(|d| d.contains("Foundation")));
}

#[test]
fn flappy_bird_app_builds_and_validates() {
    if which::which("ld64.lld-22").is_err()
        && which::which("ld64.lld-21").is_err()
        && which::which("ld64.lld").is_err()
    {
        eprintln!("ld64.lld недоступен — тест пропущен");
        return;
    }
    let bin = env!("CARGO_BIN_EXE_openipa");
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let sdk = make_sdk(tmp.path());

    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/FlappyBirdApp");
    copy_dir(
        &base.join("FlappyBirdApp.xcodeproj"),
        &tmp.path().join("FlappyBirdApp.xcodeproj"),
    );
    copy_dir(
        &base.join("FlappyBirdApp"),
        &tmp.path().join("FlappyBirdApp"),
    );

    let add = Command::new(bin)
        .env("HOME", &home)
        .args([
            "sdk",
            "add",
            sdk.to_str().unwrap(),
            "--name",
            "test-ios",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "sdk add: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let build = Command::new(bin)
        .env("HOME", &home)
        .current_dir(tmp.path())
        .args([
            "build",
            "FlappyBirdApp.xcodeproj",
            "--configuration",
            "release",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let ipa = tmp.path().join("build/FlappyBirdApp.ipa");
    assert!(ipa.exists());

    let validation = Command::new(bin)
        .args(["validate", ipa.to_str().unwrap(), "--no-color"])
        .output()
        .unwrap();
    assert!(validation.status.success());

    let info = openipa::macho::parse_macho_file(&tmp.path().join("build/FlappyBirdApp")).unwrap();
    assert_eq!(info.architecture, "arm64");
    assert!(info.dylibs.iter().any(|d| d.contains("CoreGraphics")));
}

#[test]
fn flappy_bird_binary_has_3d_and_glass_features() {
    if which::which("ld64.lld-22").is_err()
        && which::which("ld64.lld-21").is_err()
        && which::which("ld64.lld").is_err()
    {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_openipa");
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let sdk = make_sdk(tmp.path());
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/FlappyBirdApp");
    copy_dir(
        &base.join("FlappyBirdApp.xcodeproj"),
        &tmp.path().join("FlappyBirdApp.xcodeproj"),
    );
    copy_dir(
        &base.join("FlappyBirdApp"),
        &tmp.path().join("FlappyBirdApp"),
    );

    let add = Command::new(bin)
        .env("HOME", &home)
        .args([
            "sdk",
            "add",
            sdk.to_str().unwrap(),
            "--name",
            "test-ios",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(add.status.success());
    let build = Command::new(bin)
        .env("HOME", &home)
        .current_dir(tmp.path())
        .args([
            "build",
            "FlappyBirdApp.xcodeproj",
            "--configuration",
            "release",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let exe = fs::read(tmp.path().join("build/FlappyBirdApp")).unwrap();
    let strings = String::from_utf8_lossy(&exe);
    // Objective-C class metadata содержит имена используемых классов.
    assert!(
        strings.contains("UIVisualEffectView"),
        "нет liquid-glass HUD"
    );
    assert!(strings.contains("UIBlurEffect"), "нет blur-эффекта");
    let info = openipa::macho::parse_macho_file(&tmp.path().join("build/FlappyBirdApp")).unwrap();
    assert!(
        info.dylibs.iter().any(|d| d.contains("CoreGraphics")),
        "нет 3D/CG рендера"
    );

    let source = fs::read_to_string(base.join("FlappyBirdApp/main.m")).unwrap();
    assert!(source.contains("drawCubeInContext:"), "нет 3D-проекции");
    assert!(
        source.contains("CGContextFillPath"),
        "нет заливки 3D-граней"
    );
    assert!(source.contains("coinX"), "нет монет");
    assert!(source.contains("shield"), "нет щита");
    assert!(source.contains("particleLife"), "нет частиц");
}
