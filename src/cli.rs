//! Определение CLI на clap. Все команды из спецификации.

use clap::{ArgAction, Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum Configuration {
    #[default]
    Debug,
    Release,
}

impl std::fmt::Display for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Configuration::Debug => write!(f, "Debug"),
            Configuration::Release => write!(f, "Release"),
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "openipa",
    version,
    about = "OpenIPA Compiler — сборка iOS-приложений на Linux без macOS и Xcode",
    long_about = None,
    propagate_version = true
)]
pub struct Cli {
    /// Подробный вывод (команды компиляторов,Paths, диагностика)
    #[arg(short, long, global = true, action = ArgAction::SetTrue)]
    pub verbose: bool,

    /// Вывод диагностики в JSON вместо человекочитаемого
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Отключить цветной вывод
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Создать шаблон Xcode-проекта
    Init {
        /// Имя проекта
        name: String,
        /// Тип шаблона: SwiftUI, UIKit или C
        #[arg(long, value_enum, default_value = "swiftui")]
        template: TemplateKind,
        /// Каталог назначения (по умолчанию текущий)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },

    /// Собрать Xcode-проект в неподписанный IPA
    Build {
        /// Путь к .xcodeproj или каталогу проекта
        project: std::path::PathBuf,
        /// Конфигурация сборки
        #[arg(short, long, value_enum, default_value = "debug")]
        configuration: Configuration,
        /// Имя target (по умолчанию первый application target)
        #[arg(long)]
        target: Option<String>,
        /// SDK: iphoneos (по умолчанию) или iphonesimulator
        #[arg(long, default_value = "iphoneos")]
        sdk: String,
        /// Полная пересборка без кэша
        #[arg(long, action = ArgAction::SetTrue)]
        clean: bool,
        /// Число параллельных задач
        #[arg(short = 'j', long)]
        jobs: Option<usize>,
    },

    /// Показать информацию о собранном IPA
    Inspect { ipa: std::path::PathBuf },

    /// Статическая проверка IPA
    Validate { ipa: std::path::PathBuf },

    /// Очистить артефакты сборки
    Clean {
        /// Каталог проекта (по умолчанию текущий)
        #[arg(default_value = ".")]
        project: std::path::PathBuf,
    },

    /// Диагностика окружения и тулчейна
    Doctor,

    /// Версия OpenIPA
    Version,

    /// Управление iOS SDK
    Sdk {
        #[command(subcommand)]
        action: SdkCommand,
    },

    /// Управление Swift Package зависимостями
    Package {
        #[command(subcommand)]
        action: PackageCommand,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TemplateKind {
    Swiftui,
    Uikit,
    C,
}

#[derive(Debug, Subcommand)]
pub enum SdkCommand {
    /// Список зарегистрированных SDK
    List,
    /// Зарегистрировать SDK по пути к iPhoneOS.sdk
    Add {
        path: std::path::PathBuf,
        /// Псевдоним (по умолчанию имя каталога)
        #[arg(long)]
        name: Option<String>,
    },
    /// Подробная информация о SDK
    Inspect {
        /// Имя или путь SDK (по умолчанию активный)
        #[arg(default_value = "")]
        name: String,
    },
    /// Проверить целостность и совместимость SDK
    Validate {
        #[arg(default_value = "")]
        name: String,
    },
    /// Удалить SDK из реестра
    Remove { name: String },
    /// Назначить SDK активным по умолчанию
    Use { name: String },
}

#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    /// Разрешить зависимости (разбор Package.resolved / Package.swift)
    Resolve {
        #[arg(default_value = ".")]
        project: std::path::PathBuf,
    },
    /// Обновить зависимости
    Update {
        #[arg(default_value = ".")]
        project: std::path::PathBuf,
    },
    /// Список зависимостей
    List {
        #[arg(default_value = ".")]
        project: std::path::PathBuf,
    },
}

/// Коды выхода так же, как у clang: 0 — успех, 1 — ошибка сборки/валидации,
/// 2 — ошибка конфигурации/окружения.
pub mod exit {
    pub const OK: i32 = 0;
    pub const FAILURE: i32 = 1;
    pub const USAGE: i32 = 2;
}
