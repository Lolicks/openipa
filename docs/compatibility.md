# Совместимость и ограничения

Этот документ — честный перечень того, что OpenIPA может и чего не может.
Здесь нет «планов Supported», выдаваемых за реализацию.

## Реально работает

- Разбор Xcode-проектов: `project.pbxproj` в формате OpenStep, targets,
  build phases, build settings, пути файлов через PBXGroup, PBXVariantGroup
  (локализации `Base.lproj`), Info.plist.
- **Этап 6:** разбор `.xcconfig` (включая `#include`), раскрытие переменных
  `$(SRCROOT)`, `$(PROJECT_DIR)`, `$(TARGET_NAME)`, `$(PRODUCT_NAME)`,
  `$(CONFIGURATION)`, `$(inherited)`, conditional keys `KEY[sdk=iphoneos*]`,
  приоритет xcconfig < project < target, unresolved settings в диагностике.
  Обнаруживаются `PBXFileSystemSynchronizedRootGroup` (Xcode 16) с предупреждением.
- **Этап 1:** глубокий анализ SDK: `libSystem`, бинарники Framework, Swift stdlib
  для iOS, версия SDK, минимальный deployment target, флаги пригодности к линковке.
- Компиляция C, C++, Objective-C, Objective-C++ в настоящие объектные файлы
  ARM64 Mach-O через `clang -target arm64-apple-ios`.
- **Этап 2:** Objective-C runtime (`-fobjc-runtime=ios-X.Y`), `-fmodules`,
  `-F` framework search paths, линковка `-framework UIKit/Foundation/...`
  и `-lSystem` через `ld64.lld`.
- **Этап 5:** полный разбор `Contents.json`: imagesets с `idiom`/`scale`
  (`@2x`, `@3x`, `~iphone`, `~ipad`), colorsets, app icons, SVG → PNG через
  ImageMagick (при установке), генерация `CFBundleIcons`.
- **Этап 7:** Info.plist с обязательными ключами для Sideloadly:
  `CFBundleSupportedPlatforms`, `LSRequiresIPhoneOS`,
  `UIRequiredDeviceCapabilities=arm64`, `MinimumOSVersion`, `DTSDKName`.
- Проверка Mach-O: magic, CPU type, filetype, load commands, dylib-зависимости,
  LC_CODE_SIGNATURE, LC_BUILD_VERSION, LC_UUID.
- Инкрементальная сборка: SHA-256 хэш исходника и настроек, кэш в `~/.openipa/cache`.
- Параллельная компиляция (`--jobs`).
- Упаковка неподписанного IPA и его статическая валидация.
- CLI: `init`, `build`, `inspect`, `validate`, `clean`, `doctor`, `version`,
  `sdk *`, `package *`, флаги `--verbose`, `--json`, `--no-color`.

## Фундаментальные блокеры

### Swift / SwiftUI / UIKit (этапы 3–4)

Открытый Swift toolchain для Linux (`swift.org`) не содержит:
- Swift stdlib, собранный для `arm64-apple-ios`;
- module maps и binary-библиотеки `UIKit.framework` / `SwiftUI.framework`;
- системные Frameworks Apple (они проприетарны и поставляются только с Xcode).

OpenIPA не подменяет эти фреймворки заглушками. Вместо этого:
- проверяется наличие Swift stdlib для iOS в toolchain (`usr/lib/swift/iphoneos`);
- путь к подходящему toolchain задаётся через `OPENIPA_SWIFT_TOOLCHAIN=/path/to/toolchain`;
- при наличии stdlib и SDK **выполняется реальная компиляция** Swift-файлов
  через `swiftc -frontend -c -target arm64-apple-ios -emit-object` и линковка
  с `-lswiftCore`;
- при отсутствии — диагностика `SWIFT_IOS_CROSS_UNSUPPORTED`, сборка останавливается.

SwiftUI требует `SwiftUI.framework` в SDK и `can_compile_swift_ios=true`; это
проверяется физически, а не по флагу. На практике такой toolchain доступен только
в составе Xcode на macOS — в чистом Linux-окружении этапы 3–4 остаются
заблокированными, и OpenIPA честно сообщает об этом.

Это означает, что критерии готовности «поддержка SwiftUI/UIKit» и «запуск на
реальном iPhone» для Swift-приложений в текущей версии **не выполнены**.

### Assets.car (этап 5)

Компилятор ассетов `actool` — часть Xcode. OpenIPA полностью разбирает
`Contents.json`: imagesets с idiom/scale, colorsets, app icons, launch images;
конвертирует SVG в PNG через ImageMagick, если он доступен; копирует ресурсы в
`.app` как обычные файлы и формирует `CFBundleIcons`. Бинарный `Assets.car`
не генерируется и не имитируется — выдаётся `ASSETCAR_REQUIRES_ACTOOL`.

`openipa validate` дополнительно проверяет `CFBundleSupportedPlatforms`,
`UIRequiredDeviceCapabilities`, формат bundle id, Mach-O ARM64 и отсутствие
подписи. Статическая проверка по-прежнему не гарантирует запуск на устройстве.

### Подпись

Подпись, entitlements, provisioning profiles и Apple Developer Account намеренно
не реализованы — IPA предназначен для подписания через Sideloadly.

## Условия, которые должен обеспечить пользователь

- Легально полученный `iPhoneOS.sdk` (из Xcode) — для C/ObjC линковки.
- Для Swift/SwiftUI: инструменты Xcode. Это противоречит цели «без macOS»,
  поэтому в текущей версии такие проекты не собираются — честно, без симуляции.

## Статическая валидация ≠ запуск на устройстве

`openipa validate` проверяет структуру архива, Info.plist, Mach-O, архитектуру,
отсутствие подписи. Она не гарантирует, что iOS загрузит приложение. В отчёте
всегда присутствует disclaimer. Подтверждение требует Sideloadly и реального iPhone.
