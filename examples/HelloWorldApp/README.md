# HelloWorldApp — Objective-C/UIKit приложение с надписью «Hello, World!»

Реальный iOS-проект: `main.m` использует настоящие классы UIKit через
`objc_msgSend` (UILabel, UIWindow, UIViewController, UIScreen, UIColor, UIFont).
Никаких заглушек UIKit в рантайме нет — IPA линкуется против системных
Frameworks, которые есть на устройстве.

## Что внутри

- `main.m` — объявления минимальных @interface + `UIApplicationMain`,
  `AppDelegate` создаёт окно, `HelloViewController` в `viewDidLoad` добавляет
  `UILabel` с текстом `Hello, World!` по центру экрана.
- `HelloWorldApp.xcodeproj/project.pbxproj` — target `HelloWorldApp`,
  `CLANG_ENABLE_OBJC_ARC = NO`, `OTHER_LDFLAGS = -lobjc -framework UIKit ...`.
- Info.plist генерируется OpenIPA с обязательными ключами.

## Сборка

Нужен iPhoneOS.sdk с заголовками и настоящими .tbd (из Xcode) либо тестовая
фикстура для проверки конвейера:

```sh
python3 scripts/make_sdk_fixture.py ~/.openipa/test-sdk
openipa sdk add ~/.openipa/test-sdk/iPhoneOS.sdk --name test-ios
openipa build examples/HelloWorldApp/HelloWorldApp.xcodeproj --configuration release
openipa validate build/HelloWorldApp.ipa
```

Полученный `HelloWorldApp.ipa` — ARM64 Mach-O с зависимостями:
`libSystem.B.dylib`, `libobjc.A.dylib`, `UIKit`, `Foundation`, `CoreFoundation`.

## Оговорка

Тестовая фикстура содержит только link-time TBD-стабы, не заголовки и не
реализации. Она подтверждает, что конвейер создаёт настоящий IPA, но не заменяет Apple
SDK. Установка на реальный iPhone через Sideloadly и отображение надписи не
проверены в этом Linux-окружении.
