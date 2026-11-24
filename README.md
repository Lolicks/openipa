# OpenIPA Compiler

Независимая консольная система сборки iOS-приложений под Linux на Rust.
Цель — собрать обычный Xcode-проект в неподписанный IPA **без macOS, Xcode и удалённого Mac**.

```
openipa build ./MyApp.xcodeproj
# -> build/MyApp.ipa
```

## Важное заявление о возможностях

OpenIPA — не имитатор сборки. Компоненты, которые можно реализовать на открытых
технологиях, реализованы по-настоящему. Компоненты, требующие проприетарных
Apple-артефактов, **не подменяются заглушками**: система выдаёт диагностику и
останавливает сборку, вместо того чтобы произвести фиктивный IPA.

| Подсистема | Статус | Что именно |
|---|---|---|
| CLI, диагностика, JSON-вывод | Работает | Все команды из спецификации |
| Парсер `project.pbxproj` | Работает | OpenStep-парсер, targets, build settings, источники, ресурсы, зависимости |
| `Info.plist` | Работает | Чтение XML/binary, генерация XML |
| Toolchain detection | Работает | clang, clang++, swiftc, ld64.lld, zip |
| SDK Manager | Работает | Регистрация и структурная проверка предоставленного SDK |
| C / C++ / Objective-C / Objective-C++ | Работает | clang `-target arm64-apple-ios` → **настоящие ARM64 Mach-O объекты** |
| Mach-O | Работает | Собственный парсер заголовка и load commands |
| Линковка | Работает при наличии SDK | `ld64.lld` (LLVM Mach-O linker) |
| Build system | Работает | Граф зависимостей, SHA-256 кэш, параллельная компиляция |
| Asset compiler | Частично | xcassets/imagesets/colorsets разбираются и копируются; `Assets.car` требует Apple `actool` |
| IPA packager | Работает | Корректная структура Payload/.app + ZIP, без подписи |
| Валидатор IPA | Работает | 11 проверок, включая Mach-O и ARM64 |
| `.xcconfig`, переменные, приоритет настроек | Работает | `$(inherited)`, `$(SRCROOT)`, `$(TARGET_NAME)`, conditional keys; неизвестные ключи — в диагностике |
| Глубокий анализ SDK | Работает | libSystem, framework binaries, Swift stdlib, версии, deployment target |
| UIKit-линковка | Работает при SDK | `-framework`, `-F`, `-fobjc-runtime=ios-X.Y`, `-lSystem` |
| Swift / SwiftUI | Реальная проверка + компиляция при toolchain | `OPENIPA_SWIFT_TOOLCHAIN` + iOS stdlib; иначе `SWIFT_IOS_CROSS_UNSUPPORTED`, без симуляции |
| **SwiftUI на чистом Linux** | **Заблокировано** | Настоящие SwiftUI/UIKit поставляются только в iPhoneOS.sdk из Xcode |

Подробности и причины — в [docs/compatibility.md](docs/compatibility.md).

## Установка

Требуется Linux x86_64, Rust 1.80+, Clang с поддержкой AArch64 и LLVM `ld64.lld`.

```sh
sudo apt install clang lld          # Debian/Ubuntu: ld64.lld входит в lld
cargo build --release
install -m 755 target/release/openipa /usr/local/bin/openipa
openipa doctor
```

## Быстрый старт

```sh
openipa init MyApp --template c       # или swiftui / uikit
cd MyApp
openipa build MyApp.xcodeproj
openipa build MyApp.xcodeproj --configuration Release --target MyApp
openipa validate build/MyApp.ipa
openipa inspect build/MyApp.ipa
```

### SDK

OpenIPA не скачивает и не распространяет Apple SDK. SDK должен быть получен
законно (в составе Xcode) и указан вручную:

```sh
openipa sdk add /path/to/iPhoneOS.sdk
openipa sdk list
openipa sdk inspect
openipa sdk validate
openipa sdk remove <name>
```

### Остальные команды

```sh
openipa clean
openipa doctor
openipa version
openipa package resolve|update|list [project]
```

Глобальные флаги: `--verbose`, `--json`, `--no-color`.

## Как выглядит настоящая сборка

```
[OpenIPA] Анализ проекта...
[OpenIPA] проект 'MyApp': targets: 1
[OpenIPA] Проверка тулчейна...
[OpenIPA] Компиляция C/C++/Objective-C...
[clang] /usr/bin/clang -target arm64-apple-ios -c ... -o build/objects/000_main.o
[OpenIPA] Линковка Mach-O (ld64.lld)...
[OpenIPA] Обработка ресурсов...
[OpenIPA] Сборка .app и упаковка IPA...
SUCCESS: build/MyApp.ipa
```

Если SDK не подключён, сборка останавливается с `IOS_SDK_MISSING` и пояснением,
что полученный без SDK бинарник не может быть загружен iOS. Если в проекте есть
Swift-исходники, сборка останавливается с `SWIFT_IOS_CROSS_UNSUPPORTED`.

## Архитектура

```
src/
  main.rs          CLI-диспетчер
  cli.rs           clap-определения команд
  config.rs        конфигурация ~/.openipa
  diagnostics.rs   ошибки/предупреждения + JSON-бандл
  project/         pbxproj (OpenStep), model, Info.plist
  sdk.rs           реестр и проверка iPhoneOS.sdk
  toolchain.rs     детекция clang/swiftc/ld64.lld
  macho.rs         парсер Mach-O 64
  compiler/        clang (C/C++/ObjC), swift (честная диагностика)
  linker.rs        ld64.lld
  build/           граф, кэш, параллельная компиляция
  assets.rs        xcassets → файлы bundle
  packager.rs      .app + IPA (ZIP)
  validator.rs     статическая проверка IPA
```

## Пример «Hello, World!» на UIKit

`examples/HelloWorldApp` — настоящее Objective-C/UIKit приложение: `UILabel` с
текстом `Hello, World!` по центру экрана через `UIApplicationMain`. Собирается
в ARM64 IPA с зависимостями `UIKit`, `Foundation`, `CoreFoundation`, `libobjc`,
`libSystem`:

```sh
python3 scripts/make_sdk_fixture.py ~/.openipa/test-sdk   # только для проверки конвейера
openipa sdk add ~/.openipa/test-sdk/iPhoneOS.sdk --name test-ios
openipa build examples/HelloWorldApp/HelloWorldApp.xcodeproj --configuration release
openipa validate build/HelloWorldApp.ipa
```

Фикстура содержит только link-time TBD-стабы — это не Apple SDK. См.
[examples/HelloWorldApp/README.md](examples/HelloWorldApp/README.md).

## Тесты

```sh
cargo test
```

22 теста, включая интеграционные: реальная компиляция C и Objective-C/UIKit в
ARM64 Mach-O через clang, линковка через `ld64.lld`, сборка HelloWorldApp в IPA и
его валидация, разбор xcconfig и раскрытие переменных, imageset/app icon с idiom
и scale.

## Ограничения readiness-критериев

Критерии 1–9, 12 выполняются в рамках доступных компонентов. Критерии 10–11
(установка через Sideloadly, запуск на реальном iPhone) и полноценная поддержка
SwiftUI/UIKit не могут быть подтверждены в Linux-окружении без легального iPhoneOS.sdk
и физического устройства — это честно зафиксировано, а не симулировано.

## Лицензия

MIT OR Apache-2.0. См. [LICENSE](LICENSE).
