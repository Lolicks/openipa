#!/usr/bin/env python3
"""Создаёт ТЕСТОВУЮ SDK-фикстуру для проверки конвейера OpenIPA в Linux.

ВНИМАНИЕ: это НЕ настоящий Apple SDK. Это минимальные link-time TBD-стабы,
которые описывают install-name системных библиотек и фреймворков, реально
присутствующих на iOS. Они позволяют ld64.lld слинковать настоящий ARM64
Mach-O бинарник, но не содержат заголовков и реализаций. Для реальной сборки
используйте легально полученный iPhoneOS.sdk из Xcode.
"""
import sys
from pathlib import Path

FRAMEWORKS = {
    "UIKit": [
        "_UIApplicationMain",
        "_OBJC_CLASS_$_UIColor",
        "_OBJC_CLASS_$_UIFont",
        "_OBJC_CLASS_$_UILabel",
        "_OBJC_CLASS_$_UIResponder",
        "_OBJC_METACLASS_$_UIResponder",
        "_OBJC_CLASS_$_UIScreen",
        "_OBJC_CLASS_$_UIViewController",
        "_OBJC_METACLASS_$_UIViewController",
        "_OBJC_CLASS_$_UIWindow",
    ],
    "Foundation": ["_OBJC_CLASS_$_NSObject", "_OBJC_METACLASS_$_NSObject"],
    "CoreFoundation": ["___CFConstantStringClassReference"],
    "CoreGraphics": [],
    "SwiftUI": [],
}
OBJC = [
    "_objc_msgSend",
    "_objc_msgSendSuper2",
    "_objc_alloc",
    "_objc_alloc_init",
    "__objc_empty_cache",
    "_objc_retain",
    "_objc_release",
    "_objc_autoreleasePoolPush",
    "_objc_autoreleasePoolPop",
    "_objc_storeStrong",
    "_objc_retainAutoreleasedReturnValue",
]


def tbd(path: Path, install: str, symbols: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "--- !tapi-tbd-v3",
        "archs: [ arm64 ]",
        "platform: ios",
        f"install-name: {install}",
        "exports:",
        "  - archs: [ arm64 ]",
        "    symbols: [",
        *[f"      {s}," for s in symbols],
        "    ]",
        "...",
        "",
    ]
    path.write_text("\n".join(lines))


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: make_sdk_fixture.py <output-dir>")
        return 2
    sdk = Path(sys.argv[1]) / "iPhoneOS.sdk"
    for name in FRAMEWORKS:
        tbd(
            sdk / "System/Library/Frameworks" / f"{name}.framework" / f"{name}.tbd",
            f"/System/Library/Frameworks/{name}.framework/{name}",
            FRAMEWORKS[name],
        )
    tbd(
        sdk / "usr/lib/libSystem.tbd",
        "/usr/lib/libSystem.B.dylib",
        [],
    )
    tbd(sdk / "usr/lib/libobjc.tbd", "/usr/lib/libobjc.A.dylib", OBJC)
    (sdk / "usr/include").mkdir(parents=True, exist_ok=True)
    (sdk / "usr/lib/swift/iphoneos").mkdir(parents=True, exist_ok=True)
    (sdk / "SDKSettings.plist").write_text(
        """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CanonicalName</key><string>iphoneos</string>
<key>DisplayName</key><string>iOS 17.0 (TEST FIXTURE)</string>
<key>Version</key><string>17.0</string>
<key>DefaultProperties</key><dict><key>IPHONEOS_DEPLOYMENT_TARGET</key><string>12.0</string></dict>
</dict></plist>
"""
    )
    print(sdk)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
