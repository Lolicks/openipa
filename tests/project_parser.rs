//! Тест парсера настоящего project.pbxproj: targets, settings, файлы.

use openipa::project::{load_project, pbxproj};
use std::fs;
use std::path::{Path, PathBuf};

const PBXPROJ: &str = r#"// !$*UTF8*$!
{
	archiveVersion = 1;
	classes = {
	};
	objectVersion = 56;
	objects = {

/* Begin PBXBuildFile section */
		AA0000000000000000000001 /* App.swift in Sources */ = {isa = PBXBuildFile; fileRef = BB0000000000000000000001 /* App.swift */; };
		AA0000000000000000000002 /* Logo.png in Resources */ = {isa = PBXBuildFile; fileRef = BB0000000000000000000002 /* Logo.png */; };
/* End PBXBuildFile section */

/* Begin PBXFileReference section */
		BB0000000000000000000001 /* App.swift */ = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = "App.swift"; sourceTree = "<group>"; };
		BB0000000000000000000002 /* Logo.png */ = {isa = PBXFileReference; lastKnownFileType = image.png; path = "Assets.xcassets/Logo.png"; sourceTree = "<group>"; };
/* End PBXFileReference section */

/* Begin PBXNativeTarget section */
		CC0000000000000000000001 /* MyApp */ = {
			isa = PBXNativeTarget;
			buildConfigurationList = DD0000000000000000000001 /* Build configuration list */;
			buildPhases = (
				EE0000000000000000000001 /* Sources */,
				EE0000000000000000000002 /* Resources */,
			);
			buildRules = (
			);
			dependencies = (
			);
			name = MyApp;
			productName = MyApp;
			productType = "com.apple.product-type.application";
		};
/* End PBXNativeTarget section */

/* Begin PBXProject section */
		FF0000000000000000000001 /* Project object */ = {
			isa = PBXProject;
			targets = (
				CC0000000000000000000001 /* MyApp */,
			);
		};
/* End PBXProject section */

/* Begin PBXSourcesBuildPhase section */
		EE0000000000000000000001 /* Sources */ = {
			isa = PBXSourcesBuildPhase;
			files = (
				AA0000000000000000000001 /* App.swift in Sources */,
			);
		};
		EE0000000000000000000002 /* Resources */ = {
			isa = PBXResourcesBuildPhase;
			files = (
				AA0000000000000000000002 /* Logo.png in Resources */,
			);
		};
/* End PBXSourcesBuildPhase section */

/* Begin XCBuildConfiguration section */
		GG0000000000000000000001 /* Debug */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon;
				IPHONEOS_DEPLOYMENT_TARGET = 17.0;
				INFOPLIST_FILE = "MyApp/Info.plist";
				PRODUCT_BUNDLE_IDENTIFIER = com.acme.MyApp;
				PRODUCT_NAME = MyApp;
				SWIFT_ACTIVE_COMPILATION_CONDITIONS = DEBUG;
				SWIFT_VERSION = 5.10;
				TARGETED_DEVICE_FAMILY = "1,2";
				MY_CUSTOM_SETTING = somevalue;
			};
			name = Debug;
		};
/* End XCBuildConfiguration section */

/* Begin XCConfigurationList section */
		DD0000000000000000000001 /* Build configuration list */ = {
			isa = XCConfigurationList;
			buildConfigurations = (
				GG0000000000000000000001 /* Debug */,
			);
		};
/* End XCConfigurationList section */
	};
	rootObject = FF0000000000000000000001 /* Project object */;
}
"#;

#[test]
fn parses_full_project() {
    let root = pbxproj::Parser::new(PBXPROJ).parse().unwrap();
    let project =
        openipa::project::model::parse_project(&PathBuf::from("/tmp/MyApp.xcodeproj"), &root)
            .unwrap();
    assert_eq!(project.name, "MyApp");
    assert_eq!(project.targets.len(), 1);
    let target = &project.targets[0];
    assert!(target.is_application());
    assert_eq!(target.name, "MyApp");
    assert_eq!(target.sources.len(), 1);
    assert_eq!(target.sources[0], PathBuf::from("App.swift"));
    assert_eq!(target.resources.len(), 1);
    assert!(target.resources[0].to_string_lossy().contains("Logo.png"));
    let cfg = target.configuration("Debug").unwrap();
    assert_eq!(cfg.settings.swift_version.as_deref(), Some("5.10"));
    assert_eq!(
        cfg.settings.product_bundle_identifier.as_deref(),
        Some("com.acme.MyApp")
    );
    assert_eq!(
        cfg.settings.iphoneos_deployment_target.as_deref(),
        Some("17.0")
    );
    assert_eq!(
        cfg.settings.swift_active_compilation_conditions,
        vec!["DEBUG"]
    );
    assert_eq!(
        cfg.settings.asset_catalog_compiler_appicon_name.as_deref(),
        Some("AppIcon")
    );
    assert!(cfg
        .settings
        .unknown
        .iter()
        .any(|(k, _)| k == "MY_CUSTOM_SETTING"));
}

#[test]
fn loads_project_from_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let xcodeproj = tmp.path().join("MyApp.xcodeproj");
    fs::create_dir_all(&xcodeproj).unwrap();
    fs::write(xcodeproj.join("project.pbxproj"), PBXPROJ).unwrap();
    let project = load_project(&xcodeproj).unwrap();
    assert_eq!(project.targets.len(), 1);
}

#[test]
fn xcconfig_and_variable_expansion() {
    use openipa::project::settings::{merge_layers, resolve_values, SettingsContext};
    use std::collections::BTreeMap;

    let ctx = SettingsContext::new(
        Path::new("/tmp/Demo"),
        "Demo",
        "MyApp",
        "Release",
        "iphoneos",
        "iphoneos",
    );
    assert_eq!(
        ctx.expand("$(SRCROOT)/$(TARGET_NAME)/Info.plist"),
        "/tmp/Demo/MyApp/Info.plist"
    );

    let mut base = BTreeMap::new();
    base.insert(
        "PRODUCT_BUNDLE_IDENTIFIER".to_string(),
        "com.base.app".to_string(),
    );
    let mut target = BTreeMap::new();
    target.insert(
        "PRODUCT_BUNDLE_IDENTIFIER".to_string(),
        "$(inherited).demo".to_string(),
    );
    target.insert("PRODUCT_NAME".to_string(), "$(TARGET_NAME)".to_string());
    let merged = merge_layers(&[&base, &target]);
    let resolved = resolve_values(&merged, &ctx);
    assert_eq!(
        resolved.get("PRODUCT_BUNDLE_IDENTIFIER").unwrap(),
        "com.base.app.demo"
    );
    assert_eq!(resolved.get("PRODUCT_NAME").unwrap(), "MyApp");
}

#[test]
fn project_loads_xcconfig_and_unknown_setting() {
    let tmp = tempfile::tempdir().unwrap();
    let xcodeproj = tmp.path().join("MyApp.xcodeproj");
    fs::create_dir_all(&xcodeproj).unwrap();
    fs::write(
        tmp.path().join("Config.xcconfig"),
        "PRODUCT_BUNDLE_IDENTIFIER = com.from.xcconfig\nCUSTOM_XC_SETTING = yes\n",
    )
    .unwrap();
    let mut pbx = PBXPROJ.to_string();
    pbx = pbx.replace(
        "PRODUCT_BUNDLE_IDENTIFIER = com.acme.MyApp;",
        "PRODUCT_BUNDLE_IDENTIFIER = \"$(inherited)\";",
    );
    pbx = pbx.replace(
        "\t\tGG0000000000000000000001 /* Debug */ = {\n\t\t\tisa = XCBuildConfiguration;",
        "\t\tGG0000000000000000000001 /* Debug */ = {\n\t\t\tisa = XCBuildConfiguration;\n\t\t\tbaseConfigurationReference = XC0000000000000000000001 /* Config.xcconfig */;",
    );
    pbx = pbx.replace(
        "/* End PBXFileReference section */",
        "        XC0000000000000000000001 /* Config.xcconfig */ = {isa = PBXFileReference; path = Config.xcconfig; sourceTree = \"<group>\"; };\n/* End PBXFileReference section */",
    );
    fs::write(xcodeproj.join("project.pbxproj"), pbx).unwrap();
    let project = load_project(&xcodeproj).unwrap();
    let cfg = project.targets[0].configuration("Debug").unwrap();
    assert!(cfg.base_xcconfig.is_some());
    assert_eq!(
        cfg.settings.product_bundle_identifier.as_deref(),
        Some("com.from.xcconfig")
    );
    assert!(cfg
        .settings
        .unknown
        .iter()
        .any(|(k, _)| k == "CUSTOM_XC_SETTING"));
}
