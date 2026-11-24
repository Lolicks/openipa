//! Извлечение семантической модели из разобранного pbxproj.

use super::pbxproj::Value;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildSettings {
    pub product_bundle_identifier: Option<String>,
    pub product_name: Option<String>,
    pub iphoneos_deployment_target: Option<String>,
    pub swift_version: Option<String>,
    pub swift_optimization_level: Option<String>,
    pub swift_active_compilation_conditions: Vec<String>,
    pub other_swift_flags: Vec<String>,
    pub other_c_flags: Vec<String>,
    pub header_search_paths: Vec<String>,
    pub framework_search_paths: Vec<String>,
    pub info_plist_file: Option<String>,
    pub asset_catalog_compiler_appicon_name: Option<String>,
    pub targeted_device_family: Option<String>,
    pub product_bundle_package_type: Option<String>,
    pub other_ldflags: Vec<String>,
    pub other_cxxflags: Vec<String>,
    pub clang_enable_modules: bool,
    pub clang_enable_objc_arc: bool,
    pub raw: BTreeMap<String, String>,
    pub unknown: Vec<(String, String)>,
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        Value::Data(d) => hex::encode(d),
        Value::Dict(_) => String::new(),
    }
}

pub fn map_from_value(v: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, val) in v.as_dict().unwrap_or(&[]) {
        out.insert(
            super::settings::strip_condition(k).to_string(),
            value_to_string(val),
        );
    }
    out
}

fn split_words(s: Option<&String>) -> Vec<String> {
    s.map(|v| v.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

impl BuildSettings {
    /// Создаёт настройки из карты с уже раскрытыми значениями.
    pub fn from_map(map: &BTreeMap<String, String>) -> Self {
        const KNOWN: &[&str] = &[
            "PRODUCT_BUNDLE_IDENTIFIER",
            "PRODUCT_NAME",
            "IPHONEOS_DEPLOYMENT_TARGET",
            "SWIFT_VERSION",
            "SWIFT_OPTIMIZATION_LEVEL",
            "SWIFT_ACTIVE_COMPILATION_CONDITIONS",
            "OTHER_SWIFT_FLAGS",
            "OTHER_CFLAGS",
            "OTHER_CPLUSPLUSFLAGS",
            "OTHER_LDFLAGS",
            "HEADER_SEARCH_PATHS",
            "FRAMEWORK_SEARCH_PATHS",
            "LIBRARY_SEARCH_PATHS",
            "INFOPLIST_FILE",
            "ASSETCATALOG_COMPILER_APPICON_NAME",
            "TARGETED_DEVICE_FAMILY",
            "PRODUCT_BUNDLE_PACKAGE_TYPE",
            "SDKROOT",
            "SUPPORTED_PLATFORMS",
            "GENERATE_INFOPLIST_FILE",
            "CURRENT_PROJECT_VERSION",
            "MARKETING_VERSION",
            "INFOPLIST_KEY_CFBundleDisplayName",
            "INFOPLIST_KEY_UILaunchStoryboardName",
            "INFOPLIST_KEY_UIMainStoryboardFile",
            "INFOPLIST_KEY_UIApplicationSceneManifest_Generation",
            "INFOPLIST_KEY_LSApplicationCategoryType",
            "CLANG_ENABLE_MODULES",
            "CLANG_ENABLE_OBJC_ARC",
            "CLANG_ANALYZER_NONNULL",
            "CLANG_CXX_LANGUAGE_STANDARD",
            "CODE_SIGNING_ALLOWED",
            "CODE_SIGN_STYLE",
            "DEVELOPMENT_TEAM",
            "ENABLE_BITCODE",
            "ENABLE_TESTABILITY",
            "EXCLUDED_ARCHS",
            "ARCHS",
            "INSTALL_PATH",
            "ONLY_ACTIVE_ARCH",
            "SKIP_INSTALL",
            "SWIFT_EMIT_LOC_STRINGS",
            "SWIFT_STRICT_CONCURRENCY",
            "SWIFT_SWIFT3_OBJC_INFERENCE",
            "VALID_ARCHS",
            "WRAPPER_EXTENSION",
            "LD_RUNPATH_SEARCH_PATHS",
            "GCC_PREPROCESSOR_DEFINITIONS",
            "ASSETCATALOG_COMPILER_GLOBAL_ACCENT_COLOR_NAME",
            "ASSETCATALOG_COMPILER_APPICON_NAME",
        ];
        let get = |k: &str| map.get(k);
        let unknown = map
            .iter()
            .filter(|(k, _)| !KNOWN.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self {
            product_bundle_identifier: get("PRODUCT_BUNDLE_IDENTIFIER").cloned(),
            product_name: get("PRODUCT_NAME").cloned(),
            iphoneos_deployment_target: get("IPHONEOS_DEPLOYMENT_TARGET").cloned(),
            swift_version: get("SWIFT_VERSION").cloned(),
            swift_optimization_level: get("SWIFT_OPTIMIZATION_LEVEL").cloned(),
            swift_active_compilation_conditions: split_words(get(
                "SWIFT_ACTIVE_COMPILATION_CONDITIONS",
            )),
            other_swift_flags: split_words(get("OTHER_SWIFT_FLAGS")),
            other_c_flags: split_words(get("OTHER_CFLAGS")),
            other_cxxflags: split_words(get("OTHER_CPLUSPLUSFLAGS")),
            other_ldflags: split_words(get("OTHER_LDFLAGS")),
            header_search_paths: split_words(get("HEADER_SEARCH_PATHS")),
            framework_search_paths: split_words(get("FRAMEWORK_SEARCH_PATHS")),
            info_plist_file: get("INFOPLIST_FILE").cloned(),
            asset_catalog_compiler_appicon_name: get("ASSETCATALOG_COMPILER_APPICON_NAME").cloned(),
            targeted_device_family: get("TARGETED_DEVICE_FAMILY").cloned(),
            product_bundle_package_type: get("PRODUCT_BUNDLE_PACKAGE_TYPE").cloned(),
            clang_enable_modules: get("CLANG_ENABLE_MODULES")
                .map(|v| v == "YES" || v == "1")
                .unwrap_or(false),
            clang_enable_objc_arc: get("CLANG_ENABLE_OBJC_ARC")
                .map(|v| v == "YES" || v == "1")
                .unwrap_or(false),
            raw: map.clone(),
            unknown,
        }
    }

    pub fn from_value(v: &Value) -> Self {
        Self::from_map(&map_from_value(v))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XcConfiguration {
    pub name: String,
    pub settings: BuildSettings,
    pub base_xcconfig: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub product_type: Option<String>,
    pub product_name: Option<String>,
    pub sources: Vec<PathBuf>,
    pub resources: Vec<PathBuf>,
    pub frameworks: Vec<String>,
    pub configurations: Vec<XcConfiguration>,
    pub dependencies: Vec<String>,
}

impl Target {
    pub fn is_application(&self) -> bool {
        self.product_type
            .as_deref()
            .map(|t| t.contains("application"))
            .unwrap_or(false)
    }
    pub fn configuration(&self, name: &str) -> Option<&XcConfiguration> {
        self.configurations.iter().find(|c| c.name == name)
    }
    pub fn swift_sources(&self) -> Vec<&PathBuf> {
        self.sources
            .iter()
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("swift"))
            .collect()
    }
    pub fn c_sources(&self) -> Vec<&PathBuf> {
        self.sources
            .iter()
            .filter(|p| {
                matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("c" | "cc" | "cpp" | "cxx" | "m" | "mm")
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XcodeProject {
    pub path: PathBuf,
    pub name: String,
    pub targets: Vec<Target>,
    pub object_version: Option<String>,
    pub unknown_settings: Vec<(String, String)>,
    pub parse_warnings: Vec<String>,
}

impl XcodeProject {
    pub fn application_targets(&self) -> Vec<&Target> {
        self.targets.iter().filter(|t| t.is_application()).collect()
    }
}

struct Objects<'a> {
    by_id: HashMap<&'a str, &'a Value>,
}

impl<'a> Objects<'a> {
    fn get(&self, id: &str) -> Option<&'a Value> {
        self.by_id.get(id).copied()
    }
}

pub fn parse_project(path: &Path, root: &Value) -> Result<XcodeProject> {
    let objects_value = root
        .get("objects")
        .ok_or_else(|| anyhow::anyhow!("pbxproj не содержит секцию objects"))?;
    let mut by_id = HashMap::new();
    for (id, val) in objects_value.as_dict().unwrap_or(&[]) {
        by_id.insert(id.as_str(), val);
    }
    let objects = Objects { by_id };

    let root_id = root
        .get_str("rootObject")
        .ok_or_else(|| anyhow::anyhow!("pbxproj не содержит rootObject"))?;
    let project_obj = objects
        .get(root_id)
        .ok_or_else(|| anyhow::anyhow!("rootObject {root_id} не найден"))?;

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Project")
        .trim_end_matches(".xcodeproj")
        .to_string();

    let target_ids: Vec<&str> = project_obj
        .get("targets")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let project_root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let project_cfg_list = project_obj.get_str("buildConfigurationList");

    let mut targets = Vec::new();
    let mut all_unknown = Vec::new();

    for tid in target_ids {
        let tobj = match objects.get(tid) {
            Some(v) => v,
            None => continue,
        };
        let tname = tobj.get_str("name").unwrap_or(tid).to_string();
        let product_type = tobj.get_str("productType").map(str::to_string);
        let product_name = tobj.get_str("productName").map(str::to_string);

        let mut sources = Vec::new();
        let mut resources = Vec::new();
        let mut frameworks = Vec::new();
        for phase_id in tobj
            .get("buildPhases")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()))
            .into_iter()
            .flatten()
        {
            let phase = match objects.get(phase_id) {
                Some(p) => p,
                None => continue,
            };
            let isa = phase.get_str("isa").unwrap_or("");
            if !matches!(
                isa,
                "PBXSourcesBuildPhase" | "PBXResourcesBuildPhase" | "PBXFrameworksBuildPhase"
            ) {
                continue;
            }
            for file_id in phase
                .get("files")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()))
                .into_iter()
                .flatten()
            {
                let build_file = match objects.get(file_id) {
                    Some(b) => b,
                    None => continue,
                };
                let file_ref_id = match build_file.get_str("fileRef") {
                    Some(id) => id,
                    None => continue,
                };
                let file_ref = match objects.get(file_ref_id) {
                    Some(f) => f,
                    None => continue,
                };
                let p = resolve_file_path(file_ref, &objects);
                match isa {
                    "PBXSourcesBuildPhase" => sources.push(p),
                    "PBXResourcesBuildPhase" => resources.push(p),
                    "PBXFrameworksBuildPhase" => frameworks.push(p.to_string_lossy().into_owned()),
                    _ => {}
                }
            }
        }

        let mut configurations = Vec::new();
        if let Some(cfg_list_id) = tobj.get_str("buildConfigurationList") {
            for cfg_id in config_ids(cfg_list_id, &objects) {
                if let Some(cfg) = objects.get(cfg_id) {
                    let cname = cfg.get_str("name").unwrap_or("Unnamed").to_string();
                    let mut layers: Vec<BTreeMap<String, String>> = Vec::new();
                    let mut base_xcconfig = None;

                    let mut project_cfg = None;
                    if let Some(list_id) = project_cfg_list {
                        for id in config_ids(list_id, &objects) {
                            if let Some(c) = objects.get(id) {
                                if c.get_str("name") == Some(cname.as_str()) {
                                    project_cfg = Some(c);
                                    break;
                                }
                            }
                        }
                    }

                    if let Some(pc) = project_cfg {
                        if let Some(path) = xcconfig_path(pc, &objects, &project_root) {
                            base_xcconfig = Some(path.clone());
                            layers.push(load_xcconfig(&path));
                        }
                        layers.push(map_from_value(
                            pc.get("buildSettings").unwrap_or(&Value::Dict(vec![])),
                        ));
                    }
                    if let Some(path) = xcconfig_path(cfg, &objects, &project_root) {
                        if base_xcconfig.is_none() {
                            base_xcconfig = Some(path.clone());
                        }
                        layers.push(load_xcconfig(&path));
                    }
                    layers.push(map_from_value(
                        cfg.get("buildSettings").unwrap_or(&Value::Dict(vec![])),
                    ));

                    let layer_refs: Vec<&BTreeMap<String, String>> = layers.iter().collect();
                    let merged = super::settings::merge_layers(&layer_refs);
                    let ctx = super::settings::SettingsContext::new(
                        &project_root,
                        &name,
                        &tname,
                        &cname,
                        "iphoneos",
                        "iphoneos",
                    );
                    let resolved = super::settings::resolve_values(&merged, &ctx);
                    let settings = BuildSettings::from_map(&resolved);
                    all_unknown.extend(settings.unknown.clone());
                    configurations.push(XcConfiguration {
                        name: cname,
                        settings,
                        base_xcconfig,
                    });
                }
            }
        }

        let dependencies = tobj
            .get("dependencies")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .map(|dep_id| {
                        objects
                            .get(dep_id)
                            .and_then(|d| d.get_str("targetProxy"))
                            .and_then(|pid| objects.get(pid))
                            .and_then(|p| p.get_str("remoteGlobalIDString"))
                            .and_then(|tid| objects.get(tid))
                            .and_then(|t| t.get_str("name"))
                            .unwrap_or(dep_id)
                            .to_string()
                    })
                    .collect()
            })
            .unwrap_or_default();

        targets.push(Target {
            id: tid.to_string(),
            name: tname,
            product_type,
            product_name,
            sources,
            resources,
            frameworks,
            configurations,
            dependencies,
        });
    }

    if targets.is_empty() {
        bail!("в проекте не найдено ни одного target");
    }

    let mut all_unknown_dedup = Vec::new();
    for item in all_unknown {
        if !all_unknown_dedup.contains(&item) {
            all_unknown_dedup.push(item);
        }
    }

    let mut parse_warnings = Vec::new();
    for (id, val) in &objects.by_id {
        if val.get_str("isa") == Some("PBXFileSystemSynchronizedRootGroup") {
            parse_warnings.push(format!(
                "объект {id} использует PBXFileSystemSynchronizedRootGroup (Xcode 16): файлы синхронизируются автоматически и пока не перечислены в PBXSourcesBuildPhase"
            ));
        }
    }

    Ok(XcodeProject {
        path: path.to_path_buf(),
        name,
        targets,
        object_version: root.get_str("objectVersion").map(str::to_string),
        unknown_settings: all_unknown_dedup,
        parse_warnings,
    })
}

fn config_ids<'a>(list_id: &'a str, objects: &Objects<'a>) -> Vec<&'a str> {
    objects
        .get(list_id)
        .and_then(|list| list.get("buildConfigurations"))
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

fn xcconfig_path(cfg: &Value, objects: &Objects<'_>, project_root: &Path) -> Option<PathBuf> {
    let ref_id = cfg.get_str("baseConfigurationReference")?;
    let file_ref = objects.get(ref_id)?;
    let relative = resolve_file_path(file_ref, objects);
    let candidate = if relative.is_absolute() {
        relative
    } else {
        project_root.join(relative)
    };
    candidate.exists().then_some(candidate)
}

fn load_xcconfig(path: &Path) -> BTreeMap<String, String> {
    super::settings::parse_xcconfig(path)
        .map(|cfg| cfg.assignments)
        .unwrap_or_default()
}

fn resolve_file_path(file_ref: &Value, objects: &Objects<'_>) -> PathBuf {
    // Локализованные ресурсы Xcode хранит в PBXVariantGroup:
    // path = Main.storyboard, children = ( Base, en ), реальный файл —
    // Base.lproj/Main.storyboard.
    if file_ref.get_str("isa") == Some("PBXVariantGroup") {
        let variant = file_ref
            .get("children")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .and_then(|id| objects.get(id))
            .and_then(|c| c.get_str("name"))
            .unwrap_or("Base");
        let leaf = file_ref.get_str("path").unwrap_or("");
        let mut p = PathBuf::from(format!("{variant}.lproj"));
        p.push(leaf);
        return p;
    }
    if let Some(id) = file_ref.get_str("remoteGlobalIDString") {
        if let Some(remote) = objects.get(id) {
            if let Some(p) = remote.get_str("path") {
                return PathBuf::from(p);
            }
        }
    }
    let path = file_ref.get_str("path").unwrap_or("");
    let name = file_ref.get_str("name");
    let source_tree = file_ref.get_str("sourceTree").unwrap_or("<group>");
    // Сначала собираем путь по цепочке group'ов, затем добавляем путь самого файла.
    let mut parts: Vec<String> = Vec::new();
    let mut current = Some(file_ref);
    let mut guard = 0;
    while let Some(v) = current {
        guard += 1;
        if guard > 64 {
            break;
        }
        if let Some(p) = v.get_str("path") {
            if !p.is_empty() && p != source_tree {
                parts.push(p.to_string());
            }
        }
        current = v.get_str("parent").and_then(|pid| objects.get(pid));
    }
    parts.reverse();
    if parts.is_empty() {
        if let Some(n) = name {
            parts.push(n.to_string());
        }
    }
    if parts.is_empty() && !path.is_empty() {
        parts.push(path.to_string());
    }
    let mut result = PathBuf::new();
    for p in parts {
        result.push(p);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::pbxproj::Parser;

    fn sample() -> XcodeProject {
        let src = r#"// !$*UTF8*$!
{
 archiveVersion = 1;
 objectVersion = 56;
 objects = {
  P1 /* Project */ = { isa = PBXProject; targets = ( T1 ); rootObject = P1; };
  T1 /* MyApp */ = {
   isa = PBXNativeTarget;
   name = MyApp;
   productName = MyApp;
   productType = "com.apple.product-type.application";
   buildPhases = ( S1, R1 );
   buildConfigurationList = C1;
   dependencies = ( );
  };
  S1 = { isa = PBXSourcesBuildPhase; files = ( BF1, BF2 ); };
  R1 = { isa = PBXResourcesBuildPhase; files = ( BF3 ); };
  BF1 = { isa = PBXBuildFile; fileRef = F1; };
  BF2 = { isa = PBXBuildFile; fileRef = F2; };
  BF3 = { isa = PBXBuildFile; fileRef = F3; };
  F1 = { isa = PBXFileReference; path = "App.swift"; sourceTree = "<group>"; };
  F2 = { isa = PBXFileReference; path = "main.c"; sourceTree = "<group>"; };
  F3 = { isa = PBXFileReference; path = "logo.png"; sourceTree = "<group>"; };
  C1 = { isa = XCConfigurationList; buildConfigurations = ( CF1, CF2 ); };
  CF1 = { isa = XCBuildConfiguration; name = Debug; buildSettings = {
    PRODUCT_BUNDLE_IDENTIFIER = com.example.MyApp;
    PRODUCT_NAME = MyApp;
    IPHONEOS_DEPLOYMENT_TARGET = 16.0;
    SWIFT_VERSION = 5.9;
    INFOPLIST_FILE = "MyApp/Info.plist";
  }; };
  CF2 = { isa = XCBuildConfiguration; name = Release; buildSettings = {
    PRODUCT_BUNDLE_IDENTIFIER = com.example.MyApp;
    SWIFT_OPTIMIZATION_LEVEL = "-O";
    SOME_WEIRD_SETTING = whatever;
  }; };
 };
 rootObject = P1;
}"#;
        let root = Parser::new(src).parse().unwrap();
        parse_project(&PathBuf::from("/tmp/MyApp.xcodeproj"), &root).unwrap()
    }

    #[test]
    fn extracts_target() {
        let p = sample();
        assert_eq!(p.targets.len(), 1);
        let t = &p.targets[0];
        assert!(t.is_application());
        assert_eq!(t.name, "MyApp");
        assert_eq!(t.sources.len(), 2);
        assert_eq!(t.resources.len(), 1);
        assert_eq!(t.configurations.len(), 2);
    }

    #[test]
    fn extracts_settings() {
        let p = sample();
        let t = &p.targets[0];
        let debug = t.configuration("Debug").unwrap();
        assert_eq!(
            debug.settings.product_bundle_identifier.as_deref(),
            Some("com.example.MyApp")
        );
        assert_eq!(
            debug.settings.iphoneos_deployment_target.as_deref(),
            Some("16.0")
        );
        assert_eq!(
            debug.settings.info_plist_file.as_deref(),
            Some("MyApp/Info.plist")
        );
        assert_eq!(
            t.configuration("Release").unwrap().settings.unknown.len(),
            1
        );
    }
}
