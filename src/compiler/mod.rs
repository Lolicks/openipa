//! Драйверы компиляторов.

pub mod clang;
pub mod swift;

pub use clang::ClangDriver;
pub use swift::{SwiftCapability, SwiftDriver};
