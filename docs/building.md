# Сборка и тестирование

## Требования

- Linux x86_64 (Ubuntu 24.04+, Debian 12+), **Windows 10/11 x86_64** или macOS
- Rust 1.80+ (`rust-toolchain.toml`)
- Clang/LLVM с target AArch64: `clang`, `clang++`
- `ld64.lld` из пакета `lld` (Linux: `which ld64.lld-22`; Windows: `C:\Program Files\LLVM\bin\ld64.lld.exe`)
- `zip` (для диагностики; IPA пишется встроенной Rust-реализацией ZIP)

macOS, Xcode и удалённый Mac **не нужны** для C/Objective-C конвейера — сборка
одинаково работает на Linux и Windows.

### Windows

```powershell
choco install llvm -y
cargo build --release
$env:Path += ";$PWD\target\release"
openipa doctor
```

Если LLVM установлен не в PATH, задайте явные пути:

```powershell
$env:OPENIPA_CLANG = "C:\Program Files\LLVM\bin\clang.exe"
$env:OPENIPA_CLANGXX = "C:\Program Files\LLVM\bin\clang++.exe"
$env:OPENIPA_LD64_LLD = "C:\Program Files\LLVM\bin\ld64.lld.exe"
```

Создание тестовой SDK-фикстуры кроссплатформенно (нужен Python):

```powershell
python scripts\make_sdk_fixture.py $env:USERPROFILE\.openipa\test-sdk
openipa sdk add $env:USERPROFILE\.openipa\test-sdk\iPhoneOS.sdk --name test-ios
openipa build examples\HelloWorldApp\HelloWorldApp.xcodeproj --configuration release
openipa validate build\HelloWorldApp.ipa
```

Внутри IPA всегда используются `/` как разделитель, независимо от хоста.

## Сборка OpenIPA

```sh
cargo build --release
cargo test
```

## Тестовая инфраструктура

- `tests/project_parser.rs` — разбор реального `project.pbxproj`, проверка targets,
  настроек, источников и ресурсов, загрузка проекта с диска.
- `tests/ipa.rs` — интеграционный тест:
  1. `clang -target arm64-apple-ios -c main.c` → настоящий Mach-O объект;
  2. `ld64.lld -arch arm64` → настоящий `MH_EXECUTE`;
  3. сборка `.app`, упаковка IPA;
  4. статическая валидация IPA.

Тест пропускается, если clang/ld64.lld недоступны, но в текущем окружении
(probed: clang 21/22, ld64.lld-22) он выполняется полностью.

## Воспроизводимая сборка IPA в Linux

Проверен полный конвейер на C-проекте с **тестовой SDK-фикстурой**:

```
openipa init Demo --template c
openipa sdk add /path/to/iPhoneOS.sdk          # SDK с libSystem.tbd
openipa build Demo/Demo.xcodeproj --configuration release
openipa validate Demo/build/Demo.ipa
```

Результат: `Demo/build/Demo.ipa` (~34 КБ распакованный), внутри
`Payload/Demo.app/Demo` — настоящий `Mach-O 64-bit arm64 executable` с
`LC_LOAD_DYLIB → /usr/lib/libSystem.B.dylib`, 15 load commands. Все 14 проверок
валидатора проходят. Повторная сборка использует SHA-256 кэш (`[cache]`).

Важная оговорка: в этом демо использовался **минимальный link-time TBD-стаб**
для `libSystem` (и пустые TBD для Framework). Он корректно описывает путь к
системной библиотеке, которая реально присутствует на iOS, и позволяет
линкеру создать настоящий бинарник, но это **не настоящий Apple SDK**. Для
реальной установки нужен легальный iPhoneOS.sdk; запуск на физическом iPhone
этим тестом не доказывается.

## Что тест не доказывает

Интеграционный тест подтверждает **структурную** корректность: настоящий ARM64
Mach-O, корректный ZIP, корректный Info.plist. Он не подтверждает, что бинарник
может быть загружен iOS: для этого нужен легальный iPhoneOS.sdk с `libSystem`
(и, для Swift/SwiftUI, toolchain от Xcode).

## Локальная проверка вручную

```sh
cd examples/CApp
openipa build CApp.xcodeproj --configuration Release
openipa validate build/CApp.ipa
openipa inspect build/CApp.ipa
```

Без SDK сборка корректно остановится с диагностикой — это ожидаемое поведение,
а не ошибка теста.
