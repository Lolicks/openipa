# UIKitTestApp — тест реального UIKit-интерфейса

Проверяет пять возможностей сразу:

1. `UILabel` — «Hello, World!» на первом экране.
2. `UIButton` — «Open Detail» и «Back».
3. Обработка нажатий через `addTarget:action:forControlEvents:` (target-action).
4. Два экрана с переходом через `UINavigationController` (`push`/`pop`).
5. Загрузка ресурсов из `.app` через `NSBundle`: `message.txt` и `banner.png`
   (дисплей `UILabel` и `UIImageView`).

Всё это — вызовы настоящих классов UIKit/Foundation через Objective-C runtime;
в IPA нет заглушек фреймворков. Зависимости: `UIKit`, `Foundation`,
`CoreFoundation`, `libobjc`, `libSystem.B.dylib`.

## Сборка

```sh
python3 scripts/make_sdk_fixture.py ~/.openipa/test-sdk
openipa sdk add ~/.openipa/test-sdk/iPhoneOS.sdk --name test-ios
openipa build examples/UIKitTestApp/UIKitTestApp.xcodeproj --configuration release
openipa validate build/UIKitTestApp.ipa
```

В архиве:
```
Payload/UIKitTestApp.app/
  UIKitTestApp        # ARM64 Mach-O, MH_EXECUTE
  Info.plist
  message.txt
  banner.png
```

## Оговорка

Сборка проверена статически и автоматическим тестом `uikit_test_app_builds_with_resources`.
Поведение на физическом iPhone (переходы, отрисовка картинки) должно быть
проверено установкой через Sideloadly.
