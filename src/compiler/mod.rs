//! Драйверы компиляторов.

pub mod clang;
pub mod swift;

use std::fmt;

pub use clang::ClangDriver;
pub use swift::{SwiftCapability, SwiftDriver};

/// Ошибка компилятора/линкера с сохранённым оригинальным выводом.
#[derive(Debug, Clone)]
pub struct CompilerError {
    pub tool: &'static str,
    pub message: String,
    pub stderr: String,
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.tool, self.message)
    }
}

impl std::error::Error for CompilerError {}
