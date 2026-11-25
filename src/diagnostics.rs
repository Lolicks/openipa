//! Централизованная система диагностики в стиле Cargo/rustc.

pub mod catalog;
pub mod compiler;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Note = 0,
    Help = 1,
    Warning = 2,
    Error = 3,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Severity::Note => "note",
            Severity::Help => "help",
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "help", skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Оригинальный вывод компилятора — сохраняется всегда, показывается в --verbose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }
    pub fn note(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Note, code, message)
    }
    pub fn help(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Help, code, message)
    }

    fn new(severity: Severity, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            file: None,
            line: None,
            column: None,
            source_line: None,
            label: None,
            hint: None,
            notes: Vec::new(),
            original: None,
            detail: None,
        }
    }

    /// Копирует диагностику другого подсистемы с каноническим кодом каталога.
    pub fn new_cloned(original: &Diagnostic, code: impl Into<String>) -> Self {
        let mut d = original.clone();
        d.code = code.into();
        d.normalized()
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
    pub fn with_file(mut self, file: impl Into<PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
    pub fn with_original(mut self, original: impl Into<String>) -> Self {
        self.original = Some(original.into());
        self
    }
    pub fn with_location(
        mut self,
        file: impl Into<PathBuf>,
        line: usize,
        column: usize,
        source_line: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        self.file = Some(file.into());
        self.line = Some(line);
        self.column = Some(column);
        self.source_line = Some(source_line.into());
        self.label = Some(label.into());
        self
    }

    /// Дополнительные пояснения старого слоя `detail` + `notes`.
    pub fn all_notes(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(detail) = &self.detail {
            out.push(detail.clone());
        }
        out.extend(self.notes.iter().cloned());
        out
    }

    pub fn normalized(&self) -> Self {
        let mut out = self.clone();
        out.notes = self.all_notes();
        out.detail = None;
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundle {
    pub tool: String,
    pub version: String,
    pub generated_at: DateTime<Utc>,
    pub diagnostics: Vec<Diagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
}

impl DiagnosticBundle {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let normalized: Vec<Diagnostic> = diagnostics.iter().map(|d| d.normalized()).collect();
        let error_count = normalized
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        let warning_count = normalized
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count();
        Self {
            tool: "openipa".into(),
            version: crate::VERSION.into(),
            generated_at: Utc::now(),
            diagnostics: normalized,
            error_count,
            warning_count,
        }
    }
    pub fn to_json_pretty(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn severity_code(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "1;31",
        Severity::Warning => "1;33",
        Severity::Note => "1;34",
        Severity::Help => "1;36",
    }
}

/// Форматирует список диагностик в стиле rustc.
pub fn render(diagnostics: &[Diagnostic], color: bool) -> String {
    let mut out = String::new();
    for d in diagnostics {
        let label = paint(&d.severity.to_string(), severity_code(d.severity), color);
        let code = paint(&d.code, "1", color);
        out.push_str(&format!("{label}[{code}]: {}\n", d.message));

        if let (Some(file), Some(line)) = (&d.file, d.line) {
            let column = d.column.unwrap_or(1);
            out.push_str(&format!(
                "  {} {}:{line}:{column}\n",
                paint("-->", "1;36", color),
                file.display()
            ));
            let width = line.to_string().len();
            let gutter = " ".repeat(width);
            out.push_str(&format!("   {gutter} {}\n", paint("|", "1;34", color)));
            if let Some(source) = &d.source_line {
                out.push_str(&format!(
                    " {} {} {}\n",
                    paint(&line.to_string(), "1;34", color),
                    paint("|", "1;34", color),
                    source
                ));
                if let Some(highlight) = &d.label {
                    if !highlight.is_empty() && !source.is_empty() {
                        let start = source.find(highlight).unwrap_or(column.saturating_sub(1));
                        let start = start.min(source.len());
                        let carets = "^".repeat(highlight.len().max(1));
                        out.push_str(&format!(
                            "   {gutter} {}{}\n",
                            " ".repeat(start),
                            paint(&carets, severity_code(d.severity), color)
                        ));
                    }
                }
            }
            out.push_str(&format!("   {gutter} {}\n", paint("|", "1;34", color)));
        }
        if let Some(help) = &d.hint {
            out.push_str(&format!(
                "   {} help: {}\n",
                paint("=", "1;34", color),
                help
            ));
        }
        for note in d.all_notes() {
            out.push_str(&format!("   {} {}\n", paint("=", "1;34", color), note));
        }
        out.push('\n');
    }
    out
}

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// `--strict`: предупреждения повышаются до ошибок.
pub fn apply_strict(diagnostics: &mut [Diagnostic], strict: bool) {
    if strict {
        for d in diagnostics.iter_mut() {
            if d.severity == Severity::Warning {
                d.severity = Severity::Error;
                d.code = format!("W_AS_ERROR({})", d.code);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_error_with_snippet() {
        let d = Diagnostic::error("E0102", "required framework not found")
            .with_location("project.pbxproj", 42, 18, "SDKROOT = iphoneos;", "iphoneos")
            .with_hint("install a complete iOS SDK");
        let text = render(&[d], false);
        assert!(text.contains("error[E0102]: required framework not found"));
        assert!(text.contains("project.pbxproj:42:18"));
        assert!(text.contains("SDKROOT = iphoneos;"));
        assert!(text.contains("^^^^^^^^"));
        assert!(text.contains("= help: install a complete iOS SDK"));
    }

    #[test]
    fn json_contains_required_fields() {
        let d = Diagnostic::error("E0002", "missing source file")
            .with_location("main.swift", 3, 1, "let x = 1", "let")
            .with_hint("create the file")
            .with_note("referenced from project.pbxproj");
        let json = DiagnosticBundle::new(vec![d]).to_json_pretty().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let item = &value["diagnostics"][0];
        for key in [
            "code",
            "severity",
            "message",
            "file",
            "line",
            "column",
            "source_line",
            "label",
            "help",
            "notes",
        ] {
            assert!(item.get(key).is_some(), "нет поля {key}");
        }
    }

    #[test]
    fn strict_promotes_warnings() {
        let mut diags = vec![Diagnostic::warning("W0001", "unknown setting")];
        apply_strict(&mut diags, true);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].code, "W_AS_ERROR(W0001)");
    }
}
