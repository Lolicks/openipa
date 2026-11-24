//! Сборка .app bundle и упаковка неподписанного IPA (ZIP).

use crate::diagnostics::Diagnostic;
use crate::project::info_plist::{self, PlistDict};
use anyhow::{bail, Context, Result};
use plist::Value as PlistValue;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageReport {
    pub ipa: PathBuf,
    pub app_dir: PathBuf,
    pub executable: PathBuf,
    pub files: Vec<String>,
    pub unsigned: bool,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct BundleOptions<'a> {
    pub app_name: &'a str,
    pub executable: &'a Path,
    pub info_plist: &'a PlistDict,
    pub resources: &'a [PathBuf],
    pub project_root: &'a Path,
    pub frameworks: &'a [PathBuf],
    pub plugins: &'a [PathBuf],
}

pub fn make_app_bundle(app_dir: &Path, options: &BundleOptions<'_>) -> Result<usize> {
    fs::create_dir_all(app_dir)
        .with_context(|| format!("не удалось создать {}", app_dir.display()))?;

    let exe_dst = app_dir.join(options.app_name);
    fs::copy(options.executable, &exe_dst)
        .with_context(|| format!("копирование исполняемого файла в {}", exe_dst.display()))?;
    set_executable(&exe_dst)?;

    let plist_path = app_dir.join("Info.plist");
    let xml = info_plist::to_xml_string(options.info_plist);
    fs::write(&plist_path, xml).with_context(|| format!("запись {}", plist_path.display()))?;

    let mut count = 2usize;
    count += crate::assets::copy_resources(options.resources, options.project_root, app_dir)?;

    if !options.frameworks.is_empty() {
        let fw_dir = app_dir.join("Frameworks");
        fs::create_dir_all(&fw_dir)?;
        for fw in options.frameworks {
            if !fw.exists() {
                continue;
            }
            let name = fw.file_name().context("нет имени framework")?;
            let dst = fw_dir.join(name);
            if fw.is_dir() {
                copy_dir(fw, &dst)?;
            } else {
                fs::copy(fw, &dst)?;
            }
            count += 1;
        }
    }
    if !options.plugins.is_empty() {
        let plug_dir = app_dir.join("PlugIns");
        fs::create_dir_all(&plug_dir)?;
        for p in options.plugins {
            if !p.exists() {
                continue;
            }
            let name = p.file_name().context("нет имени plugin")?;
            let dst = plug_dir.join(name);
            if p.is_dir() {
                copy_dir(p, &dst)?;
            } else {
                fs::copy(p, &dst)?;
            }
            count += 1;
        }
    }
    Ok(count)
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Создаёт IPA: Payload/<App>.app/... + опциональные iTunesMetadata.
pub fn package_ipa(app_dir: &Path, ipa_path: &Path, app_name: &str) -> Result<PackageReport> {
    if let Some(parent) = ipa_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(ipa_path)
        .with_context(|| format!("не удалось создать {}", ipa_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let mut files = Vec::new();
    let payload_prefix = format!("Payload/{app_name}.app/");
    for entry in walkdir::WalkDir::new(app_dir).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(app_dir)?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let archive_name = format!("{payload_prefix}{rel_str}");
        zip.start_file(&archive_name, options)
            .with_context(|| format!("добавление {archive_name} в IPA"))?;
        let bytes = fs::read(entry.path())?;
        zip.write_all(&bytes)?;
        files.push(archive_name);
    }

    // Явно neither подпись, ни provisioning profile не добавляются.
    zip.finish()?;

    Ok(PackageReport {
        ipa: ipa_path.to_path_buf(),
        app_dir: app_dir.to_path_buf(),
        executable: app_dir.join(app_name),
        files,
        unsigned: true,
        diagnostics: vec![Diagnostic::note(
            "IPA_UNSIGNED",
            "IPA неподписан: подпишите через Sideloadly перед установкой",
        )],
    })
}

pub fn read_info_plist_from_ipa(ipa: &Path) -> Result<PlistDict> {
    let file = File::open(ipa).with_context(|| format!("не удалось открыть {}", ipa.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("{} не является ZIP/IPA", ipa.display()))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.ends_with("Info.plist") && name.contains(".app/") {
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            let value = plist::Value::from_reader_xml(data.as_slice())
                .or_else(|_| plist::Value::from_reader(std::io::Cursor::new(data.as_slice())))?;
            let dict = value
                .into_dictionary()
                .ok_or_else(|| anyhow::anyhow!("Info.plist не словарь"))?;
            return Ok(dict.into_iter().collect());
        }
    }
    bail!("в IPA не найден Info.plist")
}

pub fn plist_string(dict: &PlistDict, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(|v| v.as_string())
        .map(str::to_string)
}

/// Добавляет ключи, которые должны присутствовать в IPA для установки и
/// корректной работы UIKit/SwiftUI приложения (Sideloadly не модифицирует Info.plist).
pub fn augment_info_plist(
    dict: &mut PlistDict,
    executable: &str,
    min_os: &str,
    sdk_version: Option<&str>,
    app_icons: &[String],
    launch_image: Option<&str>,
) {
    fn add(dict: &mut PlistDict, key: &str, value: PlistValue) {
        dict.entry(key.to_string()).or_insert(value);
    }
    add(
        dict,
        "CFBundleExecutable",
        PlistValue::String(executable.into()),
    );
    add(
        dict,
        "CFBundlePackageType",
        PlistValue::String("APPL".into()),
    );
    add(
        dict,
        "CFBundleSupportedPlatforms",
        PlistValue::Array(vec![PlistValue::String("iPhoneOS".into())]),
    );
    add(dict, "LSRequiresIPhoneOS", PlistValue::Boolean(true));
    add(
        dict,
        "UIRequiredDeviceCapabilities",
        PlistValue::Array(vec![PlistValue::String("arm64".into())]),
    );
    add(dict, "MinimumOSVersion", PlistValue::String(min_os.into()));
    if let Some(version) = sdk_version {
        add(
            dict,
            "DTSDKName",
            PlistValue::String(format!("iphoneos{version}")),
        );
    }
    if !app_icons.is_empty() {
        let files = PlistValue::Array(
            app_icons
                .iter()
                .map(|f| PlistValue::String(f.clone()))
                .collect(),
        );
        let mut primary = PlistDict::new();
        primary.insert("CFBundleIconFiles".into(), files);
        let mut icons = PlistDict::new();
        icons.insert(
            "CFBundlePrimaryIcon".into(),
            PlistValue::Dictionary(primary.into_iter().collect()),
        );
        dict.insert(
            "CFBundleIcons".into(),
            PlistValue::Dictionary(icons.into_iter().collect()),
        );
    }
    if let Some(name) = launch_image {
        add(
            dict,
            "UILaunchStoryboardName",
            PlistValue::String(name.into()),
        );
    }
}

pub fn default_info_plist(
    name: &str,
    bundle_id: &str,
    min_os: &str,
    executable: &str,
) -> PlistDict {
    let mut d = PlistDict::new();
    d.insert("CFBundleName".into(), PlistValue::String(name.into()));
    d.insert(
        "CFBundleDisplayName".into(),
        PlistValue::String(name.into()),
    );
    d.insert(
        "CFBundleIdentifier".into(),
        PlistValue::String(bundle_id.into()),
    );
    d.insert(
        "CFBundleExecutable".into(),
        PlistValue::String(executable.into()),
    );
    d.insert(
        "CFBundlePackageType".into(),
        PlistValue::String("APPL".into()),
    );
    d.insert(
        "CFBundleShortVersionString".into(),
        PlistValue::String("1.0".into()),
    );
    d.insert("CFBundleVersion".into(), PlistValue::String("1".into()));
    d.insert("MinimumOSVersion".into(), PlistValue::String(min_os.into()));
    d.insert(
        "UIDeviceFamily".into(),
        PlistValue::Array(vec![
            PlistValue::Integer(plist::Integer::from(1)),
            PlistValue::Integer(plist::Integer::from(2)),
        ]),
    );
    augment_info_plist(&mut d, executable, min_os, None, &[], None);
    d
}
