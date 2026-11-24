# iOS SDK Manager

## Принцип

OpenIPA **не скачивает, не генерирует и не распространяет** Apple SDK. Проприетарный
`iPhoneOS.sdk` может быть получен только законно — в составе Xcode на macOS.
Пользователь самостоятельно указывает путь к такому SDK:

```sh
openipa sdk add /path/to/iPhoneOS.sdk
```

## Что проверяется

`openipa sdk inspect` / `openipa sdk validate` выполняют структурную проверку:

| Проверка | Что ищется |
|---|---|
| `directory` | каталог существует |
| `sdksettings` | `SDKSettings.plist` или `Settings.plist`, читается как plist |
| `frameworks_dir` | `System/Library/Frameworks/` |
| `required_frameworks` | `Foundation.framework`, `UIKit.framework`, `CoreGraphics.framework` |
| `usr_lib` | `usr/lib/` (системные библиотеки, `libSystem`) |
| `usr_include` | `usr/include/` (заголовки) |
| `ui_frameworks` | наличие `SwiftUI.framework` и `UIKit.framework` |
| `framework_binaries` | реальные бинарники/tbd внутри `.framework` |
| `libsystem` | `usr/lib/libSystem.tbd` / `libSystem.dylib` — без него линковка невозможна |
| `swift_stdlib_iphoneos` | `usr/lib/swift/iphoneos` — необходим для Swift-сборки |

Из `SDKSettings.plist` извлекаются `Version`, `DisplayName`, `CanonicalName` и
`IPHONEOS_DEPLOYMENT_TARGET`. Результат доступен в человекочитаемом виде и в JSON (`--json`).

`openipa sdk inspect` дополнительно печатает путь к `libSystem`, Swift stdlib,
число framework-бинарников и флаги `can_link` / `can_compile_swift_ios`.
`openipa build` сверяет запрошенный `IPHONEOS_DEPLOYMENT_TARGET` с минимумом SDK
(`DEPLOYMENT_TARGET_INCOMPATIBLE`).

Для Swift Toolchain:
```sh
export OPENIPA_SWIFT_TOOLCHAIN=/path/to/XcodeDefault.xctoolchain
openipa doctor     # покажет swiftc из этого toolchain
```

## Реестр

Состояние хранится в `~/.openipa/sdks/registry.json`:

```json
{
  "active": "iPhoneOS17",
  "sdks": {
    "iPhoneOS17": { "name": "iPhoneOS17", "path": "...", "version": "17.0" }
  }
}
```

Команды: `list`, `add PATH [--name NAME]`, `inspect [NAME]`, `validate [NAME]`,
`remove NAME`, `use NAME`.

## Что не является проверкой

Структурная проверка не проверяет криптографическую подлинность SDK, не
устанавливает сертификаты и не заменяет Xcode. Наличие каталога с нужными именами
ещё не делает SDK рабочим: настоящая линковка дополнительно проверит наличие
`libSystem` через `ld64.lld`.

## Отсутствие SDK

Если SDK не зарегистрирован:

- `openipa doctor` показывает `MISSING iOS SDK`;
- `openipa build` останавливается с `IOS_SDK_MISSING` и не создаёт IPA;
- `openipa sdk inspect /nonexistent` возвращает `SDK_MISSING` с подсказкой.
