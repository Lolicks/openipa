//! Парсер сообщений Clang, Swift и ld64.lld в общий формат OpenIPA.
//! Местоположение извлекается только тогда, когда компилятор его указал.

use super::{Diagnostic, Severity};

fn parse_line(line: &str) -> Option<Diagnostic> {
    const MARKERS: [(&str, Severity); 4] = [
        (": error: ", Severity::Error),
        (": warning: ", Severity::Warning),
        (": note: ", Severity::Note),
        (": help: ", Severity::Help),
    ];
    for (marker, severity) in MARKERS {
        let Some(pos) = line.find(marker) else {
            continue;
        };
        let head = &line[..pos];
        let message = line[pos + marker.len()..].trim();
        let mut parts = head.rsplit(':');
        let maybe_column = parts.next();
        let maybe_line = parts.next();
        let (file, line_no, column) = match (
            maybe_line.and_then(|s| s.parse::<usize>().ok()),
            maybe_column,
        ) {
            (Some(line_no), Some(column)) if column.parse::<usize>().is_ok() => {
                let rest: Vec<&str> = parts.collect();
                let file = rest.into_iter().rev().collect::<Vec<_>>().join(":");
                (file, Some(line_no), column.parse::<usize>().ok())
            }
            (Some(line_no), Some(_)) => {
                // head заканчивается на path:line (без колонки).
                let rest: Vec<&str> = parts.clone().collect();
                let file = rest.into_iter().rev().collect::<Vec<_>>().join(":");
                (file, Some(line_no), None)
            }
            _ => (String::new(), None, None),
        };
        let mut d = Diagnostic::note("I0001", message);
        d.severity = severity;
        if severity == Severity::Error {
            d.code = "E0601".into();
        }
        d.file = if file.is_empty() {
            None
        } else {
            Some(file.into())
        };
        d.line = line_no;
        d.column = column;
        d.original = Some(line.to_string());
        return Some(d);
    }
    None
}

/// Разбирает вывод компилятора. Строки без распознаваемого формата не теряются:
/// они сохраняются как диагностика уровня note без выдуманного местоположения.
pub fn parse(output: &str) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(d) = parse_line(line) {
            out.push(d);
            continue;
        }
        let trimmed = line.trim_start();
        let is_context = trimmed.starts_with('|')
            || trimmed.starts_with('^')
            || trimmed.starts_with('~')
            || (trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
                && trimmed.contains('|'))
            || trimmed.ends_with("generated.")
            || trimmed.ends_with("generated");
        if let Some(last) = out.last_mut() {
            let note = trimmed.to_string();
            if is_context {
                last.notes.push(note);
                continue;
            }
            let mut d = Diagnostic::note("I0001", trimmed).with_original(line);
            if trimmed.to_lowercase().contains("warning") {
                d.severity = Severity::Warning;
            }
            out.push(d);
        } else {
            let mut d = Diagnostic::note("I0001", trimmed).with_original(line);
            if trimmed.to_lowercase().contains("error") {
                d.severity = Severity::Error;
                d.code = "E0601".into();
            } else if trimmed.to_lowercase().contains("warning") {
                d.severity = Severity::Warning;
            }
            out.push(d);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clang_style() {
        let diags = parse(
            "src/main.c:12:5: error: use of undeclared identifier 'x'\n2 warnings generated.\n",
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0]
            .notes
            .iter()
            .any(|n| n.contains("2 warnings generated")));
        assert_eq!(diags[0].code, "E0601");
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(
            diags[0].file.as_deref(),
            Some(std::path::Path::new("src/main.c"))
        );
        assert_eq!(diags[0].line, Some(12));
        assert_eq!(diags[0].column, Some(5));
        assert_eq!(diags[0].message, "use of undeclared identifier 'x'");
        assert!(diags[0].original.is_some());
    }

    #[test]
    fn parses_windows_path() {
        let diags = parse(r"C:\proj\main.m:7:3: warning: unused variable");
        assert_eq!(
            diags[0].file.as_deref(),
            Some(std::path::Path::new(r"C:\proj\main.m"))
        );
        assert_eq!(diags[0].line, Some(7));
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn does_not_invent_location() {
        let diags = parse("ld64.lld: error: library not found for -lSystem");
        assert_eq!(diags[0].file, None);
        assert_eq!(diags[0].line, None);
        assert_eq!(diags[0].severity, Severity::Error);
    }
}
