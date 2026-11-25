# FlappyBirdApp — игра на UIKit + CoreGraphics

Простая игра, собранная OpenIPA из обычного Xcode-проекта:

- перо птицы — касание экрана (`touchesBegan:withEvent:`);
- гравитация и скорость — `NSTimer` с интервалом 0.02 c;
- отрисовка — настоящий `drawRect:` через `UIGraphicsGetCurrentContext()` и
 и CoreGraphics (`CGContextFillRect`, `CGContextFillEllipseInRect`);
- счёт — `UILabel`, обновляется при прохождении трубы;
- после столкновения касание перезапускает игру.

Никаких заглушек UIKit/CoreGraphics: IPA линкуется против системных
`UIKit`, `Foundation`, `CoreFoundation`, `CoreGraphics`, `libobjc`, `libSystem.B.dylib`.

## Сборка

```sh
python3 scripts/make_sdk_fixture.py ~/.openipa/test-sdk
openipa sdk add ~/.openipa/test-sdk/iPhoneOS.sdk --name test-ios
openipa build examples/FlappyBirdApp/FlappyBirdApp.xcodeproj --configuration release
openipa validate build/FlappyBirdApp.ipa
```

## Оговорка

Сборка проверена статически и автотестом `flappy_bird_app_builds_and_validates`.
Игровой процесс на физическом iPhone нужно проверить через Sideloadly.
