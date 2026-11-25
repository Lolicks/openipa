# Diagnostic Engine

OpenIPA проверяет проект **до запуска Clang, Swift и ld64.lld**. При наличии ошибок
компиляция не начинается и фиктивный IPA не создаётся.

## Команды

```sh
openipa check MyApp.xcodeproj
openipa check MyApp.xcodeproj --configuration release --target MyApp
openipa check MyApp.xcodeproj --strict
openipa check MyApp.xcodeproj --json
openipa explain E0102
```

`openipa build` выполняет те же проверки автоматически и останавливается до
компиляции. `--strict` повышает все warnings до ошибок (код становится
`W_AS_ERROR(...)`).

## Формат

```text
error[E0102]: required framework not found
  --> project.pbxproj:42:18
   |
42 | SDKROOT = iphoneos;
   |           ^^^^^^^^^
   |
   = help: install a complete iOS SDK
```

Уровни: `error`, `warning`, `note`, `help`. Цветной вывод по умолчанию,
отключается `--no-color` или `NO_COLOR`.

## Что проверяется до компиляции

- структура project.pbxproj, наличие исходников и ресурсов
- Build Settings: формат версии и bundle id, `ARCHS`, неизвестные ключи, незакрытые `$(VAR)`
- SDK: наличие, версия, `libSystem`, frameworks, deployment target
- Toolchain: clang, ld64.lld, swiftc
- Swift: stdlib для `arm64-apple-ios`, совместимость toolchain, SwiftUI framework
- Asset catalogs: `Contents.json`, ссылки на файлы, отсутствие `actool`
- Возможности Xcode, которые OpenIPA не разбирает (`PBXFileSystemSynchronizedRootGroup`)

Проверки не выдумывают ошибки: местоположение указывается только если найдено
в исходном файле по конкретному ключу/имени.

## Каталог кодов

| Код | Значение |
|---|---|
| E0001 | invalid project file |
| E0002 | missing source file |
| E0003 | invalid build setting |
| E0004 | malformed pbxproj structure |
| E0101 | SDK not found |
| E0102 | required framework not found |
| E0103 | unsupported SDK version |
| E0104 | libSystem not found |
| E0105 | deployment target incompatible |
| E0201 | Swift standard library missing |
| E0202 | Swift toolchain incompatible |
| E0301 | invalid Contents.json |
| E0302 | unsupported asset compilation |
| E0303 | missing asset file |
| E0401 | unsupported Xcode project feature |
| E0501 | linker unavailable |
| E0502 | compiler unavailable |
| E0503 | invalid linker configuration |
| E0601 | compiler diagnostic (Clang/Swift/ld64.lld) |
| W0001..W0008, W0302 | warnings |
| I0001 | informational |

`openipa explain <code>` печатает объяснение и рекомендацию. Коды уникальны,
поддерживаются псевдонимы старых кодов (`SDK_MISSING` → `E0101` и т.д.).

## Диагностика компиляторов

Сообщения Clang/Swift/ld64.lld разбираются в общий формат: извлекаются файл,
строка, колонка, уровень и текст. Строки-сниппеты компилятора становятся
пояснениями (`notes`) к основной ошибке. Оригинальный вывод сохраняется в поле
`original` и печатается при `--verbose` — если компилятор не указал местоположение,
OpenIPA его не выдумывает.

## JSON

`--json` выдаёт один `DiagnosticBundle` со всеми диагностиками за запуск:

```json
{
  "tool": "openipa",
  "diagnostics": [{
    "code": "E0002",
    "severity": "error",
    "message": "исходный файл не найден: main.swift",
    "file": "MyApp.xcodeproj/project.pbxproj",
    "line": 12,
    "column": 9,
    "source_line": "... main.swift ...",
    "label": "main.swift",
    "help": "создайте файл или удалите ссылку из Xcode-проекта",
    "notes": []
  }]
}
```
