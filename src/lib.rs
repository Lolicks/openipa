//! OpenIPA — независимая система сборки iOS-приложений под Linux.
//!
//! Библиотека предоставляет реальные компоненты: парсер Xcode-проектов,
//! менеджер SDK, драйверы компиляторов, разбор Mach-O, сборщик и валидатор IPA.
//! Компоненты, требующие проприетарного Apple SDK (SwiftUI/UIKit), честно
//! сообщают об ограничении через диагностику, а не имитируют работу.

pub mod assets;
pub mod build;
pub mod check;
pub mod cli;
pub mod compiler;
pub mod config;
pub mod diagnostics;
pub mod linker;
pub mod macho;
pub mod packager;
pub mod project;
pub mod sdk;
pub mod toolchain;
pub mod validator;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
