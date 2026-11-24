//! Раскрытие переменных build settings и разбор .xcconfig.
//! Реализует базовую семантику Xcode: $(VAR), $(inherited), приоритет
//! xcconfig < project < target и условные ключи вида KEY[sdk=iphoneos*].

use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SettingsContext {
    pub vars: HashMap<String, String>,
}

impl SettingsContext {
    pub fn new(
        project_root: &Path,
        project_name: &str,
        target_name: &str,
        configuration: &str,
        sdk_name: &str,
        platform: &str,
    ) -> Self {
        let mut vars = HashMap::new();
        vars.insert(
            "SRCROOT".into(),
            project_root.to_string_lossy().into_owned(),
        );
        vars.insert(
            "PROJECT_DIR".into(),
            project_root.to_string_lossy().into_owned(),
        );
        vars.insert(
            "SOURCE_ROOT".into(),
            project_root.to_string_lossy().into_owned(),
        );
        vars.insert("PROJECT_NAME".into(), project_name.to_string());
        vars.insert("TARGET_NAME".into(), target_name.to_string());
        vars.insert("PRODUCT_NAME".into(), target_name.to_string());
        vars.insert("CONFIGURATION".into(), configuration.to_string());
        vars.insert("SDKROOT".into(), sdk_name.to_string());
        vars.insert("PLATFORM_NAME".into(), platform.to_string());
        Self { vars }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// Рекурсивно раскрывает $(VAR). Неизвестные переменные остаются как есть —
    /// это позволяет диагностировать их отдельно, а не терять молча.
    pub fn expand(&self, input: &str) -> String {
        self.expand_depth(input, 0)
    }

    fn expand_depth(&self, input: &str, depth: usize) -> String {
        if depth > 12 {
            return input.to_string();
        }
        let bytes = input.as_bytes();
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
                if let Some(end) = find_close(bytes, i + 2) {
                    let name = &input[i + 2..end];
                    if name == "inherited" {
                        out.push_str("$(inherited)");
                    } else if let Some(value) = self.vars.get(name) {
                        out.push_str(&self.expand_depth(value, depth + 1));
                    } else {
                        out.push_str(&input[i..=end]);
                    }
                    i = end + 1;
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }
}

fn find_close(bytes: &[u8], from: usize) -> Option<usize> {
    (from..bytes.len()).find(|&i| bytes[i] == b')')
}

#[derive(Debug, Clone, Default)]
pub struct Xcconfig {
    pub assignments: BTreeMap<String, String>,
    pub includes: Vec<PathBuf>,
}

/// Разбирает .xcconfig: комментарии `//`, `#include "file"` / `#include <file>`
/// и присваивания `KEY = value` (включая `$(inherited)`).
pub fn parse_xcconfig(path: &Path) -> Result<Xcconfig> {
    let mut out = Xcconfig::default();
    parse_xcconfig_inner(path, &mut out, &mut Vec::new())?;
    Ok(out)
}

fn parse_xcconfig_inner(path: &Path, out: &mut Xcconfig, stack: &mut Vec<PathBuf>) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if stack.contains(&canonical) {
        return Ok(());
    }
    stack.push(canonical.clone());
    let text = fs::read_to_string(path)
        .with_context(|| format!("не удалось прочитать xcconfig {}", path.display()))?;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let code = line.split("//").next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        if let Some(rest) = code.strip_prefix("#include") {
            let quoted = rest.trim();
            let name = quoted
                .trim_matches(|c| c == '"' || c == '<' || c == '>')
                .trim();
            if !name.is_empty() {
                let include = if Path::new(name).is_absolute() {
                    PathBuf::from(name)
                } else {
                    path.parent().unwrap_or(Path::new(".")).join(name)
                };
                out.includes.push(include.clone());
                if include.exists() {
                    parse_xcconfig_inner(&include, out, stack)?;
                }
            }
            continue;
        }
        if let Some((key, value)) = code.split_once('=') {
            let key = strip_condition(key.trim());
            out.assignments
                .insert(key.to_string(), value.trim().to_string());
        }
    }
    stack.pop();
    Ok(())
}

/// Убирает квалификатор вида `[sdk=iphoneos*]` из имени ключа.
pub fn strip_condition(key: &str) -> &str {
    key.split('[').next().unwrap_or(key).trim()
}

/// Объединяет слои настроек. `$(inherited)` в value заменяется на значение
/// из предыдущего (более низкого по приоритету) слоя.
pub fn merge_layers(layers: &[&BTreeMap<String, String>]) -> BTreeMap<String, String> {
    let mut result: BTreeMap<String, String> = BTreeMap::new();
    for layer in layers {
        for (key, value) in *layer {
            let expanded = if value.contains("$(inherited)") {
                let inherited = result.get(key).cloned().unwrap_or_default();
                value.replace("$(inherited)", &inherited)
            } else {
                value.clone()
            };
            result.insert(key.clone(), expanded);
        }
    }
    result
}

/// Применяет контекст ко всем значениям и раскрывает вложенные переменные,
/// используя уже раскрытые значения как переменные (как это делает Xcode).
pub fn resolve_values(
    raw: &BTreeMap<String, String>,
    ctx: &SettingsContext,
) -> BTreeMap<String, String> {
    let mut ctx = ctx.clone();
    for (k, v) in raw {
        if !matches!(
            k.as_str(),
            "SRCROOT"
                | "PROJECT_DIR"
                | "SOURCE_ROOT"
                | "PROJECT_NAME"
                | "TARGET_NAME"
                | "PRODUCT_NAME"
                | "CONFIGURATION"
                | "SDKROOT"
                | "PLATFORM_NAME"
        ) {
            ctx.set(k, v);
        }
    }
    // Несколько проходов: PRODUCT_NAME может ссылаться на TARGET_NAME и т.д.
    let mut resolved = raw.clone();
    for _ in 0..3 {
        for (k, v) in raw {
            resolved.insert(k.clone(), ctx.expand(v));
        }
        for (k, v) in &resolved {
            ctx.set(k, v);
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_vars() {
        let ctx = SettingsContext::new(
            Path::new("/tmp/MyApp"),
            "MyApp",
            "Demo",
            "Debug",
            "iphoneos",
            "iphoneos",
        );
        assert_eq!(ctx.expand("$(SRCROOT)/$(TARGET_NAME)"), "/tmp/MyApp/Demo");
        assert_eq!(ctx.expand("$(inherited) -O"), "$(inherited) -O");
        assert_eq!(ctx.expand("$(UNKNOWN_FLAG)"), "$(UNKNOWN_FLAG)");
    }

    #[test]
    fn parses_xcconfig_and_inherited() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Base.xcconfig");
        fs::write(
            &base,
            "PRODUCT_BUNDLE_IDENTIFIER = com.base.app\n// comment\n",
        )
        .unwrap();
        let child = dir.path().join("App.xcconfig");
        fs::write(
            &child,
            "#include \"Base.xcconfig\"\nPRODUCT_BUNDLE_IDENTIFIER = $(inherited).demo\nOTHER_SWIFT_FLAGS[sdk=iphoneos*] = -DIOS\n",
        )
        .unwrap();
        let cfg = parse_xcconfig(&child).unwrap();
        assert_eq!(
            cfg.assignments.get("PRODUCT_BUNDLE_IDENTIFIER").unwrap(),
            "$(inherited).demo"
        );
        assert!(cfg.assignments.contains_key("OTHER_SWIFT_FLAGS"));
        let layers = [&cfg.assignments];
        // $(inherited) в xcconfig ссылается на предыдущий уровень конфигурации.
        let merged = merge_layers(&layers);
        assert!(merged.contains_key("PRODUCT_BUNDLE_IDENTIFIER"));
    }

    #[test]
    fn merge_precedence() {
        let mut low = BTreeMap::new();
        low.insert("A".to_string(), "1".to_string());
        let mut high = BTreeMap::new();
        high.insert("A".to_string(), "$(inherited)2".to_string());
        let merged = merge_layers(&[&low, &high]);
        assert_eq!(merged.get("A").unwrap(), "12");
    }
}
