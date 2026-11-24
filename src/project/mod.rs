//! Парсер Xcode-проектов и Info.plist.

pub mod info_plist;
pub mod model;
pub mod pbxproj;
pub mod settings;

pub use model::{BuildSettings, Target, XcodeProject};

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Принимает путь к .xcodeproj или каталогу проекта с единственным .xcodeproj.
pub fn locate_pbxproj(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        if path.file_name().and_then(|n| n.to_str()) == Some("project.pbxproj") {
            return Ok(path.to_path_buf());
        }
        bail!(
            "ожидался project.pbxproj или каталог .xcodeproj: {}",
            path.display()
        );
    }
    if !path.is_dir() {
        bail!("путь не существует: {}", path.display());
    }
    if path.extension().and_then(|e| e.to_str()) == Some("xcodeproj") {
        let p = path.join("project.pbxproj");
        if !p.exists() {
            bail!("в {} нет project.pbxproj", path.display());
        }
        return Ok(p);
    }
    let candidates: Vec<PathBuf> = fs::read_dir(path)
        .with_context(|| format!("не удалось прочитать {}", path.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("xcodeproj"))
        .collect();
    match candidates.len() {
        0 => bail!("в каталоге {} нет .xcodeproj", path.display()),
        1 => {
            let p = candidates[0].join("project.pbxproj");
            if !p.exists() {
                bail!("в {} нет project.pbxproj", candidates[0].display());
            }
            Ok(p)
        }
        n => bail!("найдено {n} .xcodeproj — укажите конкретный путь"),
    }
}

pub fn load_project(path: &Path) -> Result<XcodeProject> {
    let pbxproj_path = locate_pbxproj(path)?;
    let src = fs::read_to_string(&pbxproj_path)
        .with_context(|| format!("не удалось прочитать {}", pbxproj_path.display()))?;
    let root = pbxproj::Parser::new(&src)
        .parse()
        .with_context(|| format!("ошибка разбора {}", pbxproj_path.display()))?;
    let xcodeproj_dir = pbxproj_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    model::parse_project(&xcodeproj_dir, &root)
}
