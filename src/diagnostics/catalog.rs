//! Каталог кодов диагностики OpenIPA. Каждый код уникален и документирован.

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub code: &'static str,
    pub title: &'static str,
    pub explanation: &'static str,
    pub help: &'static str,
    pub aliases: &'static [&'static str],
}

pub static ENTRIES: &[Entry] = &[
    Entry {
        code: "E0001",
        title: "invalid project file",
        explanation: "Проект не может быть прочитан: неверный путь, отсутствует project.pbxproj или файл повреждён.",
        help: "Укажите корректный путь к .xcodeproj или каталогу с ним.",
        aliases: &[],
    },
    Entry {
        code: "E0002",
        title: "missing source file",
        explanation: "Исходный файл из PBXSourcesBuildPhase не существует на диске.",
        help: "Восстановите файл или удалите ссылку на него из Xcode-проекта.",
        aliases: &[],
    },
    Entry {
        code: "E0003",
        title: "invalid build setting",
        explanation: "Значение build setting имеет неверный формат (например, IPHONEOS_DEPLOYMENT_TARGET не является версией).",
        help: "Исправьте значение в project.pbxproj или .xcconfig.",
        aliases: &[],
    },
    Entry {
        code: "E0004",
        title: "malformed pbxproj structure",
        explanation: "Структура project.pbxproj не соответствует формату OpenStep либо отсутствуют обязательные объекты.",
        help: "Откройте проект в Xcode и сохраните его, либо исправьте синтаксис вручную.",
        aliases: &[],
    },
    Entry {
        code: "E0101",
        title: "SDK not found",
        explanation: "iPhoneOS.sdk не зарегистрирован или не прошёл структурную проверку.",
        help: "Выполните: openipa sdk add /path/to/iPhoneOS.sdk",
        aliases: &["IOS_SDK_MISSING", "SDK_MISSING", "SDK_NOT_APPLE"],
    },
    Entry {
        code: "E0102",
        title: "required framework not found",
        explanation: "Фреймворк, используемый target, отсутствует в подключённом SDK.",
        help: "Используйте полный iPhoneOS.sdk из Xcode.",
        aliases: &["SDK_FRAMEWORK_MISSING"],
    },
    Entry {
        code: "E0103",
        title: "unsupported SDK version",
        explanation: "Версия SDK не может быть прочитана или слишком старая для выбранного deployment target.",
        help: "Подключите актуальный iPhoneOS.sdk из Xcode.",
        aliases: &[],
    },
    Entry {
        code: "E0104",
        title: "libSystem not found",
        explanation: "В SDK отсутствует libSystem (.tbd/.dylib) — ld64.lld не сможет слинковать приложение.",
        help: "Подключите полный iPhoneOS.sdk из Xcode.",
        aliases: &["SDK_LIBSYSTEM_MISSING"],
    },
    Entry {
        code: "E0105",
        title: "deployment target incompatible",
        explanation: "IPHONEOS_DEPLOYMENT_TARGET ниже минимума, поддерживаемого SDK.",
        help: "Повысьте deployment target или используйте другую версию SDK.",
        aliases: &["DEPLOYMENT_TARGET_INCOMPATIBLE"],
    },
    Entry {
        code: "E0201",
        title: "Swift standard library missing",
        explanation: "В Swift toolchain нет Swift stdlib для arm64-apple-ios.",
        help: "Укажите Xcode toolchain через OPENIPA_SWIFT_TOOLCHAIN или соберите C/Objective-C target.",
        aliases: &["SWIFT_IOS_CROSS_UNSUPPORTED"],
    },
    Entry {
        code: "E0202",
        title: "Swift toolchain incompatible",
        explanation: "swiftc не найден либо несовместим с выбранным SDK.",
        help: "Установите Swift toolchain и задайте OPENIPA_SWIFT_TOOLCHAIN.",
        aliases: &["SWIFT_TOOLCHAIN_MISSING"],
    },
    Entry {
        code: "E0301",
        title: "invalid Contents.json",
        explanation: "Contents.json в xcassets повреждён или не является корректным JSON.",
        help: "Исправьте Contents.json в Xcode.",
        aliases: &[],
    },
    Entry {
        code: "E0302",
        title: "unsupported asset compilation",
        explanation: "Для генерации бинарного Assets.car нужен проприетарный actool из Xcode.",
        help: "OpenIPA копирует ресурсы как обычные файлы; используйте Xcode для Assets.car.",
        aliases: &["ASSETCAR_REQUIRES_ACTOOL"],
    },
    Entry {
        code: "E0303",
        title: "missing asset file",
        explanation: "Файл, указанный в imageset Contents.json, не найден на диске.",
        help: "Добавьте файл изображения или уберите запись из Contents.json.",
        aliases: &["ASSET_FILE_MISSING"],
    },
    Entry {
        code: "E0401",
        title: "unsupported Xcode project feature",
        explanation: "Проект использует возможность Xcode, которую OpenIPA пока не разбирает.",
        help: "Перечислите исходники явно в PBXSourcesBuildPhase.",
        aliases: &["XCODE_SYNC_GROUP_UNSUPPORTED"],
    },
    Entry {
        code: "E0501",
        title: "linker unavailable",
        explanation: "ld64.lld не найден в окружении.",
        help: "Установите LLVM lld: apt install lld / choco install llvm.",
        aliases: &["LD64_LLD_MISSING"],
    },
    Entry {
        code: "E0502",
        title: "compiler unavailable",
        explanation: "clang не найден в окружении.",
        help: "Установите Clang/LLVM с поддержкой AArch64.",
        aliases: &["CLANG_MISSING"],
    },
    Entry {
        code: "E0503",
        title: "invalid linker configuration",
        explanation: "Исполняемый файл не будет иметь динамических зависимостей от системных библиотек.",
        help: "Подключите SDK с libSystem и проверьте OTHER_LDFLAGS.",
        aliases: &["LINK_NO_DYLD_DEPS"],
    },
    Entry {
        code: "E0601",
        title: "compiler diagnostic",
        explanation: "Ошибка, сообщённая Clang, Swift или ld64.lld. OpenIPA сохраняет оригинальный вывод.",
        help: "Исправьте код согласно сообщению компилятора; исходный вывод доступен в --verbose.",
        aliases: &["COMPILER_ERROR"],
    },
    Entry {
        code: "W0001",
        title: "unknown build setting",
        explanation: "Build setting не распознана OpenIPA и будет проигнорирована.",
        help: "Проверьте, не опечатка ли это; неизвестные настройки не влияют на сборку.",
        aliases: &["UNKNOWN_BUILD_SETTING"],
    },
    Entry {
        code: "W0002",
        title: "missing resource",
        explanation: "Ресурс из PBXResourcesBuildPhase не найден на диске.",
        help: "Восстановите файл или удалите ссылку на него.",
        aliases: &[],
    },
    Entry {
        code: "W0003",
        title: "unresolved build setting variable",
        explanation: "Переменная $(...) не была раскрыта после обработки build settings.",
        help: "Определите переменную в .xcconfig или project.pbxproj.",
        aliases: &[],
    },
    Entry {
        code: "W0302",
        title: "Assets.car requires actool",
        explanation: "OpenIPA не генерирует бинарный Assets.car — для него нужен actool из Xcode.",
        help: "Ресурсы копируются как обычные файлы; для Assets.car используйте Xcode.",
        aliases: &[],
    },
    Entry {
        code: "W0004",
        title: "SVG not converted",
        explanation: "SVG-ресурс не сконвертирован в растровый формат.",
        help: "Установите ImageMagick с delegate для SVG.",
        aliases: &["ASSET_SVG_UNCONVERTED"],
    },
    Entry {
        code: "W0005",
        title: "SDK settings unreadable",
        explanation: "SDKSettings.plist не удалось прочитать.",
        help: "Проверьте целостность SDK.",
        aliases: &["SDK_SETTINGS_UNREADABLE"],
    },
    Entry {
        code: "W0006",
        title: "no SDK sysroot",
        explanation: "Компиляция/линковка выполняется без SDK — результат не будет загружен iOS.",
        help: "Подключите iPhoneOS.sdk.",
        aliases: &["CLANG_NO_SDK", "LINK_NO_SDK"],
    },
    Entry {
        code: "W0007",
        title: "package resolution unavailable",
        explanation: "SwiftPM на Linux не может разрешить iOS-зависимости Package.swift.",
        help: "Подготовьте Package.resolved в среде с Xcode.",
        aliases: &["NO_PACKAGE_RESOLVED", "PACKAGE_UPDATE_REQUIRES_SWIFTPM"],
    },
    Entry {
        code: "W0008",
        title: "SwiftUI unavailable",
        explanation: "SwiftUI.framework отсутствует в SDK или недоступен toolchain.",
        help: "Используйте UIKit или полный Xcode SDK.",
        aliases: &[],
    },
    Entry {
        code: "W0009",
        title: "unsigned IPA",
        explanation: "IPA неподписан — это ожидаемое поведение OpenIPA.",
        help: "Подпишите IPA через Sideloadly перед установкой.",
        aliases: &["IPA_UNSIGNED"],
    },
    Entry {
        code: "W0010",
        title: "static validation limits",
        explanation: "Статическая проверка не гарантирует запуск на устройстве.",
        help: "Проверьте приложение через Sideloadly на реальном iPhone.",
        aliases: &["STATIC_ONLY"],
    },
    Entry {
        code: "W0011",
        title: "empty imageset",
        explanation: "imageset не содержит изображений.",
        help: "Добавьте PNG/JPEG в каталог imageset.",
        aliases: &["ASSET_IMAGESET_EMPTY"],
    },
    Entry {
        code: "I0001",
        title: "informational diagnostic",
        explanation: "Информационное сообщение OpenIPA.",
        help: "Действие не требуется.",
        aliases: &[
            "SDK_OK",
            "FRAMEWORKS_PRESENT",
            "SWIFT_IOS_STDLIB_FOUND",
            "APP_ICONS_FOUND",
            "SVG_CONVERTED",
            "DYNAMIC_DEPENDENCIES",
            "NO_SWIFT_PACKAGES",
        ],
    },
];

pub fn lookup(code: &str) -> Option<&'static Entry> {
    ENTRIES
        .iter()
        .find(|e| e.code.eq_ignore_ascii_case(code) || e.aliases.contains(&code))
}

pub fn all() -> impl Iterator<Item = &'static Entry> {
    ENTRIES.iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique() {
        let mut codes: Vec<&str> = ENTRIES.iter().map(|e| e.code).collect();
        codes.sort_unstable();
        for pair in codes.windows(2) {
            assert_ne!(pair[0], pair[1], "дублирующийся код {}", pair[0]);
        }
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(lookup("SDK_MISSING").unwrap().code, "E0101");
        assert_eq!(lookup("E0302").unwrap().code, "E0302");
        assert!(lookup("E9999").is_none());
    }
}
