//! Обработка ресурсов. Разбирает xcassets (imagesets, colorsets, app icons),
//! копирует изображения и шрифты в .app. Настоящий скомпилированный Assets.car
//! создаётся только проприетарным actool из Xcode — OpenIPA не подделывает его,
//! а честно сообщает об ограничении и кладёт ресурсы в виде обычных файлов.

use crate::diagnostics::Diagnostic;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetItem {
    pub name: String,
    pub kind: AssetKind,
    pub source: PathBuf,
    pub output_name: String,
    pub idiom: Option<String>,
    pub scale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Image,
    Color,
    AppIcon,
    Font,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCatalog {
    pub path: PathBuf,
    pub items: Vec<AssetItem>,
    pub app_icons: Vec<String>,
    pub launch_images: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

fn read_contents(dir: &Path) -> Option<JsonValue> {
    let path = dir.join("Contents.json");
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn suffix_for(idiom: Option<&str>, scale: Option<&str>) -> String {
    let mut out = String::new();
    // Xcode использует ~iphone/~ipad только когда idiom явно задан.
    match idiom.unwrap_or("") {
        "iphone" => out.push_str("~iphone"),
        "ipad" => out.push_str("~ipad"),
        _ => {}
    }
    match scale.unwrap_or("") {
        "2x" => out.push_str("@2x"),
        "3x" => out.push_str("@3x"),
        _ => {}
    }
    out
}

fn convert_svg(source: &Path, target: &Path, diagnostics: &mut Vec<Diagnostic>) -> Result<PathBuf> {
    if !source.exists() {
        return Ok(source.to_path_buf());
    }
    if let Ok(status) = Command::new("convert").arg(source).arg(target).status() {
        if status.success() && target.exists() {
            diagnostics.push(
                Diagnostic::note(
                    "SVG_CONVERTED",
                    format!("SVG сконвертирован: {}", source.display()),
                )
                .with_file(source),
            );
            return Ok(target.to_path_buf());
        }
    }
    diagnostics.push(
        Diagnostic::warning(
            "SVG_CONVERSION_FAILED",
            format!(
                "не удалось сконвертировать SVG {} в PNG (нужен ImageMagick с delegate для SVG)",
                source.display()
            ),
        )
        .with_file(source),
    );
    Ok(source.to_path_buf())
}

pub fn compile_catalog(catalog: &Path) -> Result<AssetCatalog> {
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();
    let mut app_icons = Vec::new();
    let mut launch_images = Vec::new();
    if !catalog.is_dir() {
        return Ok(AssetCatalog {
            path: catalog.to_path_buf(),
            items,
            app_icons,
            launch_images,
            diagnostics,
        });
    }

    for entry in WalkDir::new(catalog)
        .min_depth(1)
        .max_depth(2)
        .sort_by_file_name()
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_str().unwrap_or("asset").to_string();
        let contents = read_contents(entry.path());
        let images: Vec<JsonValue> = contents
            .as_ref()
            .and_then(|v| v.get("images"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if name.ends_with(".imageset") || name.ends_with(".launchimageset") {
            let stem = name
                .trim_end_matches(".imageset")
                .trim_end_matches(".launchimageset");
            if images.is_empty() {
                let files: Vec<PathBuf> = fs::read_dir(entry.path())
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        matches!(
                            p.extension().and_then(|e| e.to_str()),
                            Some("png" | "jpg" | "jpeg" | "svg")
                        )
                    })
                    .collect();
                for f in files {
                    items.push(AssetItem {
                        name: stem.to_string(),
                        kind: AssetKind::Image,
                        source: f,
                        output_name: format!("{stem}.png"),
                        idiom: None,
                        scale: None,
                    });
                }
            }
            for image in &images {
                let filename = image.get("filename").and_then(|v| v.as_str());
                let idiom = image.get("idiom").and_then(|v| v.as_str());
                let scale = image.get("scale").and_then(|v| v.as_str());
                let Some(filename) = filename else { continue };
                let src = entry.path().join(filename);
                if !src.exists() {
                    diagnostics.push(
                        Diagnostic::warning(
                            "ASSET_FILE_MISSING",
                            format!("файл ассета {filename} не найден в {name}"),
                        )
                        .with_file(&src),
                    );
                    continue;
                }
                let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
                let base = Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(stem)
                    .trim_end_matches("@2x")
                    .trim_end_matches("@3x");
                let mut out_name = PathBuf::from(format!("{base}{}", suffix_for(idiom, scale)));
                out_name.set_extension(ext);
                let source = if ext == "svg" {
                    let png = catalog.join(format!("{base}.png"));
                    convert_svg(&src, &png, &mut diagnostics)?
                } else {
                    src.clone()
                };
                let output_name = out_name.to_string_lossy().into_owned();
                if name.ends_with(".launchimageset") {
                    launch_images.push(output_name.clone());
                }
                items.push(AssetItem {
                    name: stem.to_string(),
                    kind: AssetKind::Image,
                    source,
                    output_name,
                    idiom: idiom.map(str::to_string),
                    scale: scale.map(str::to_string),
                });
            }
        } else if name.ends_with(".colorset") {
            let colors = images
                .iter()
                .filter_map(|v| v.get("color"))
                .filter_map(|c| c.get("components"))
                .cloned()
                .collect::<Vec<_>>();
            let out = serde_json::json!({
                "name": name.trim_end_matches(".colorset"),
                "colors": colors,
                "note": "OpenIPA: цвета не скомпилированы в Assets.car (требуется actool)"
            });
            let target = catalog.join(format!(
                "{}.colorset.json",
                name.trim_end_matches(".colorset")
            ));
            fs::write(&target, serde_json::to_string_pretty(&out)?)
                .with_context(|| format!("запись {}", target.display()))?;
            items.push(AssetItem {
                name: name.trim_end_matches(".colorset").to_string(),
                kind: AssetKind::Color,
                source: target.clone(),
                output_name: target
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("color.json")
                    .to_string(),
                idiom: None,
                scale: None,
            });
        } else if name.ends_with(".appiconset") {
            for image in &images {
                let Some(filename) = image.get("filename").and_then(|v| v.as_str()) else {
                    continue;
                };
                let src = entry.path().join(filename);
                if !src.exists() {
                    continue;
                }
                let idiom = image.get("idiom").and_then(|v| v.as_str());
                let scale = image.get("scale").and_then(|v| v.as_str());
                let base = Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("AppIcon")
                    .trim_end_matches("@2x")
                    .trim_end_matches("@3x");
                let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
                let output_name = format!("{base}{}.{ext}", suffix_for(idiom, scale));
                app_icons.push(output_name.clone());
                items.push(AssetItem {
                    name: "AppIcon".into(),
                    kind: AssetKind::AppIcon,
                    source: src,
                    output_name,
                    idiom: idiom.map(str::to_string),
                    scale: scale.map(str::to_string),
                });
            }
        }
    }

    if !app_icons.is_empty() {
        diagnostics.push(Diagnostic::note(
            "APP_ICONS_FOUND",
            format!(
                "найдено {} иконок приложения (CFBundleIcons будет добавлен в Info.plist)",
                app_icons.len()
            ),
        ));
    }
    diagnostics.push(Diagnostic::warning(
        "ASSETCAR_REQUIRES_ACTOOL",
        "OpenIPA не генерирует бинарный Assets.car: это требует проприетарного actool из Xcode. Ресурсы скопированы как обычные файлы в .app.",
    ));

    Ok(AssetCatalog {
        path: catalog.to_path_buf(),
        items,
        app_icons,
        launch_images,
        diagnostics,
    })
}

/// Копирует перечисленные ресурсы (файлы и каталоги) в корень .app.
pub fn copy_resources(resources: &[PathBuf], project_root: &Path, app_dir: &Path) -> Result<usize> {
    let mut count = 0;
    for res in resources {
        let src = if res.is_absolute() {
            res.clone()
        } else {
            project_root.join(res)
        };
        if !src.exists() {
            continue;
        }
        if src.is_dir() {
            for entry in WalkDir::new(&src) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let rel = entry.path().strip_prefix(&src)?;
                    let dst = app_dir.join(rel);
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(entry.path(), &dst)
                        .with_context(|| format!("копирование {}", entry.path().display()))?;
                    count += 1;
                }
            }
        } else if src.is_file() {
            let name = src.file_name().context("нет имени файла")?;
            fs::copy(&src, app_dir.join(name))
                .with_context(|| format!("копирование {}", src.display()))?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_imageset() {
        let dir = tempfile::tempdir().unwrap();
        let set = dir.path().join("Logo.imageset");
        fs::create_dir_all(&set).unwrap();
        fs::write(set.join("logo.png"), b"png").unwrap();
        fs::write(set.join("Contents.json"), b"{}").unwrap();
        let catalog = compile_catalog(dir.path()).unwrap();
        assert!(catalog
            .items
            .iter()
            .any(|i| i.name == "Logo" && i.kind == AssetKind::Image));
        assert!(catalog
            .diagnostics
            .iter()
            .any(|d| d.code == "ASSETCAR_REQUIRES_ACTOOL"));
    }

    #[test]
    fn parses_imageset_with_scales() {
        let dir = tempfile::tempdir().unwrap();
        let set = dir.path().join("Logo.imageset");
        fs::create_dir_all(&set).unwrap();
        fs::write(set.join("logo.png"), b"png").unwrap();
        fs::write(set.join("logo@2x.png"), b"png2").unwrap();
        fs::write(
            set.join("Contents.json"),
            r#"{"images":[{"idiom":"universal","scale":"1x","filename":"logo.png"},{"idiom":"universal","scale":"2x","filename":"logo@2x.png"}]}"#,
        )
        .unwrap();
        let catalog = compile_catalog(dir.path()).unwrap();
        assert!(catalog.items.iter().any(|i| i.output_name == "logo.png"));
        assert!(catalog.items.iter().any(|i| i.output_name == "logo@2x.png"));
    }

    #[test]
    fn parses_app_icon_and_collects_names() {
        let dir = tempfile::tempdir().unwrap();
        let set = dir.path().join("AppIcon.appiconset");
        fs::create_dir_all(&set).unwrap();
        fs::write(set.join("icon.png"), b"png").unwrap();
        fs::write(
            set.join("Contents.json"),
            r#"{"images":[{"idiom":"iphone","scale":"2x","size":"60x60","filename":"icon.png"}]}"#,
        )
        .unwrap();
        let catalog = compile_catalog(dir.path()).unwrap();
        assert_eq!(catalog.app_icons, vec!["icon~iphone@2x.png"]);
    }
}
