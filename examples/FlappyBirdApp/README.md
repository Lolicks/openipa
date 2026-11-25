# FlappyBirdApp — игра с 3D-фоном и liquid-glass HUD

Классический Flappy Bird + три дополнительные возможности.

## Что внутри

**Игра:** касание — взмах, гравитация, трубы, счёт, рестарт после проигрыша.

**3D-графика:** software-рендер вращающегося куба в `drawRect:` —
8 вершин, проекция с перспективой, 6 граней с alpha-смешиванием через
`CGContextMoveToPoint` / `CGContextAddLineToPoint` / `CGContextFillPath`.
Синус/косинус считаются локально (без extern libm-символов).

**Liquid glass:** HUD на `UIVisualEffectView` с `UIBlurEffect` (style 2)
и полупрозрачной подложкой; счёт и подсказки лежат в `contentView`.

**Ещё придумали:**
- монеты — собираются при пересечении с птицей, +1 к счётчику;
- каждые 5 монет дают щит — одно столкновение поглощается;
- частицы — вспышка при сборе монеты и столкновении.

## Сборка

```sh
python3 scripts/make_sdk_fixture.py ~/.openipa/test-sdk
openipa sdk add ~/.openipa/test-sdk/iPhoneOS.sdk --name test-ios
openipa build examples/FlappyBirdApp/FlappyBirdApp.xcodeproj --configuration release
openipa validate build/FlappyBirdApp.ipa
```

## Тесты

- `flappy_bird_app_builds_and_validates` — сборка и валидация IPA;
- `flappy_bird_binary_has_3d_and_glass_features` — в Mach-O есть зависимости
  CoreGraphics и метаданные классов `UIVisualEffectView` / `UIBlurEffect`,
  в исходнике — 3D-проекция, монеты, щит и частицы.

## Оговорка

Сборка проверена статически и автотестами с тестовой SDK-фикстурой (link-time
TBD-стабы). Запуск игры на реальном iPhone через Sideloadly не проверялся.
