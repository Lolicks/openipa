# Архитектура OpenIPA

## Принцип

OpenIPA — независимый конвейер, каждый этап которого либо использует реальный
открытый инструмент (clang, ld64.lld, ZIP), либо выполняет собственную реальную
реализацию (парсер pbxproj, парсер Mach-O, валидатор). Там, где требуется
проприетарный Apple-артефакт, этап не эмулируется — он выдаёт диагностику.

## Конвейер сборки

1. **Locate** — найти `project.pbxproj` (путь к `.xcodeproj` или каталог).
2. **Parse** — OpenStep-парсер → `Value` → семантическая модель (targets, phases, settings).
3. **Select** — выбрать target и конфигурацию (Debug/Release).
4. **Toolchain** — обнаружить clang/clang++/swiftc/ld64.lld.
5. **SDK** — загрузить реестр, проверить структуру SDK.
6. **Gate** — диагностика неизвестных настроек, отсутствия SDK, Swift-ограничения.
   При наличии ошибок сборка останавливается **до** создания IPA.
7. **Compile** — clang `-target arm64-apple-ios` для C/C++/ObjC/ObjC++ → Mach-O объекты.
   Параллельно через rayon, инкрементально через SHA-256 кэш.
8. **Link** — `ld64.lld -arch arm64 -platform_version ios ...` → MH_EXECUTE.
9. **Assets** — разбор xcassets, копирование изображений/шрифтов/локализаций.
10. **Bundle** — `.app` с Info.plist, исполняемым файлом, Frameworks/, PlugIns/.
11. **Package** — ZIP `Payload/<App>.app/...`, без подписи и provisioning profile.
12. **Validate** — статические проверки структуры и Mach-O.

## Модули и их ответственность

- `project/settings.rs` (.xcconfig + expand) — раскрытие `$(VAR)`/`$(inherited)`,
  разбор `#include`, conditional keys, приоритет слоёв.
- `project/pbxproj.rs` — токенизатор OpenStep: dict, array, string, data, комментарии.
- `project/model.rs` — обход графа объектов pbxproj (PBXNativeTarget → build phases →
  PBXBuildFile → PBXFileReference), разрешение путей через PBXGroup, извлечение
  build settings и перечисление неизвестных ключей.
- `project/info_plist.rs` — чтение и генерация XML plist.
- `macho.rs` — разбор `mach_header_64` и load commands, включая dylib-зависимости,
  LC_CODE_SIGNATURE, LC_BUILD_VERSION, LC_UUID.
- `compiler/clang.rs` — генерация настоящих ARM64 объектов, проверка результата
  через Mach-O парсер.
- `compiler/swift.rs` — проверка toolchain на Swift stdlib для iOS, реальная
  компиляция `-frontend -c -target arm64-apple-ios -emit-object`; при отсутствии
  stdlib — честная диагностика без имитации.
- `linker.rs` — интеграция с ld64.lld, `-framework`/`-F`/`-lSystem`/`-lswiftCore`,
  контроль loadable-признака.
- `assets.rs` — Contents.json (idiom/scale/colors/app icons), SVG→PNG, CFBundleIcons.
- `packager.rs` — Info.plist с ключами для Sideloadly.
- `build/` — граф (топосортировка), кэш, параллельное выполнение.
- `packager.rs` / `validator.rs` — упаковка и проверка IPA.

## Расширяемость

Архитектура позволяет постепенно заменять компоненты: драйвер clang изолирован
за `ClangDriver`, линковка за `Linker`, парсеры за своими модулями. Замена
реализации не требует изменения конвейера.
