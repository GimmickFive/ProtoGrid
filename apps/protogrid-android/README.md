# ProtoGrid Android

Тонкий Android-клиент ProtoGrid: Kotlin/Compose сверху, `VpnService` + ProtoGrid-core
(`libbox`) для туннеля и общий Rust-код для ссылок и конфигурации.

Это MVP: обзор, серверы, импорт share-ссылок, маршрутизация по приложениям
и трафику, мониторинг (график скорости и счётчики из Clash API). Подписок,
правил по процессам и пинга пока нет.

## Что используется

- `apps/protogrid-android` — UI, `VpnService`, `PlatformInterface`, профили.
- `crates/protogrid-mobile` — JNI-мост к `protogrid-core`
  (`parseLinks`, `buildConfig`, `prepareRuleSets`).
- `forks/sing-box` → `libbox.aar` — upstream-исходники, из которых собирается
  ProtoGrid-core и запускается внутри процесса через gomobile.

ProtoGrid-core линкуется в APK как `libbox` (sing-box, GPLv3), поэтому Android-
модуль распространяется под GPL-3.0 (см. [LICENSE](LICENSE)). Общий Rust-код
остаётся MIT. Собираются два AAR из одного форка: `libbox.aar` для обычной
сборки и `libbox-lite.aar` (`protogrid_lite`) для Lite-сборки.

## Маршрутизация

Два независимых режима на экране **«Маршрутизация»**.

**По приложениям** — это per-app VPN Android:

- «Все» — туннель для всех;
- «Только выбранные» — в туннель попадают только отмеченные приложения;
- «Все кроме» — в туннель идут все, кроме отмеченных.

Для списка нужен `QUERY_ALL_PACKAGES` (нормальное разрешение, для GitHub-сборки
допустимо). Изменения применяются к активному туннелю кнопкой
**«Применить и переподключить»**.

**По трафику** — правила ProtoGrid-core внутри туннеля:

- «Глобальный прокси» — всё через сервер, локальная сеть напрямую;
- «Жизнь в РФ» — RU-домены и локальная сеть напрямую, остальное через прокси;
- «Прямой» — без прокси;
- «Свой профиль» — финальное действие и свои правила: домен, суффикс,
  ключевое слово, IP/CIDR, GeoIP, GeoSite; действия «Прокси / Напрямую / Блок».

GeoIP/GeoSite требуют `.srs`-базы. Они скачиваются с GitHub SagerNet по кнопке
**«Скачать / обновить базы»** и кэшируются в `filesDir/rulesets`; сервис
проверяет/обновляет кэш перед сборкой конфига. При блокировке GitHub
используйте домены/IP/CIDR или заранее прогретый кэш.

Правила уходят в `build_config_with_capabilities` с `rule_set_dir`; per-app
списки — в `OverrideOptions` (`includePackage` / `excludePackage`) и далее в
`VpnService.Builder`.

## Локальная сборка

Нужны Linux/macOS, JDK 17, Android SDK 35 и платформа 24 (libbox собирается
с `-androidapi 24`), NDK 28.0.13004108, Go 1.27, Rust target
`aarch64-linux-android` и `cargo-ndk`.

Всю цепочку можно запустить одной командой:

```bash
ANDROID_HOME=/path/to/android-sdk ./scripts/build-android.sh
```

Или по шагам:

```bash
# 1. ProtoGrid-core (libbox): sing-box v1.14.4 + патчи, arm64.
#    Собираются оба варианта — полный и Lite.
export CGO_LDFLAGS_ALLOW='-l:.*'   # Go 1.27 отклоняет -l:libcronet.a
git clone --depth 1 --branch v1.14.4 https://github.com/SagerNet/sing-box.git forks/sing-box
cd forks/sing-box
git am ../../docs/patches/*.patch
make lib_install
go run ./cmd/internal/build_libbox -target android -platform android/arm64 \
  -output libbox.aar
go run ./cmd/internal/build_libbox -target android -platform android/arm64 \
  -tags "protogrid_lite,with_utls,with_clash_api,with_gvisor" \
  -output libbox-lite.aar -legacy=false
cd ../..

# 2. AAR и Rust-мост
cp forks/sing-box/libbox.aar apps/protogrid-android/app/libs/libbox.aar
cp forks/sing-box/libbox-lite.aar apps/protogrid-android/app/libs/libbox-lite.aar
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.0.13004108"
cargo ndk -t arm64-v8a -o apps/protogrid-android/app/src/main/jniLibs \
  build -p protogrid-mobile --release

# 3. APK
cd apps/protogrid-android
./gradlew :app:assembleStandardRelease :app:assembleLiteRelease
```

Результат: `app/build/outputs/apk/standard/release/app-standard-release.apk` и
`app/build/outputs/apk/lite/release/app-lite-release.apk`. Lite-сборка
линкует `libbox-lite.aar` (`protogrid_lite`, только VLESS/Reality по TCP) и
ставится отдельным приложением `app.protogrid.client.lite` с названием
«ProtoGrid Lite», поэтому оба варианта могут жить рядом. Без переменных
подписи Gradle подписывает debug-ключом — такой APK ставится, но обновлять его
поверх релизного нельзя.

### Эмулятор (x86_64)

Релизный APK собирается только под `arm64-v8a`, поэтому в обычный x86_64 AVD
он не установится. Для эмулятора:

```bash
ANDROID_HOME=/path/to/android-sdk ./scripts/build-android.sh x86_64
adb install -r apps/protogrid-android/app/build/outputs/apk/standard/release/app-standard-release.apk
# или Lite-вариант:
adb install -r apps/protogrid-android/app/build/outputs/apk/lite/release/app-lite-release.apk
```

Скрипт пересоберёт libbox (полный и Lite) под `android/amd64`, Rust — под
`x86_64-linux-android`, а Gradle запустит с `PROTOGRID_ABIS=x86_64`.
Стандартный AVD поддерживает `VpnService`; ARM-образы на x86_64-хосте
практически не пригодны из-за скорости.

## Релиз на GitHub

Workflow `.github/workflows/android.yml` запускается по тегу `android-v*`
или вручную. Он собирает libbox (полный и Lite), Rust `.so`, оба APK и
прикрепляет файлы к Release.

Приложение проверяет публичные релизы `GimmickFive/ProtoGrid` на GitHub при
запуске. Если в ассетах есть APK для ABI устройства (обычно
`ProtoGrid-<версия>-arm64-v8a.apk`, для Lite —
`ProtoGrid-<версия>-lite-arm64-v8a.apk`), на «Обзоре» появляется карточка
обновления: APK скачивается с прогрессом, сверяется SHA-256 из GitHub и
открывается системный установщик. Lite-сборка ищет только Lite-ассет, поэтому
варианты не перепутаются. Ядро ProtoGrid-core находится внутри APK, поэтому
отдельного скачивания ядра на Android нет — оно обновляется вместе с
приложением.

Секреты для стабильной подписи:

| Секрет | Значение |
|---|---|
| `PROTOGRID_KEYSTORE_BASE64` | `release.keystore` в base64 |
| `PROTOGRID_KEYSTORE_PASSWORD` | пароль хранилища |
| `PROTOGRID_KEY_ALIAS` | alias ключа |
| `PROTOGRID_KEY_PASSWORD` | пароль ключа |

Создать ключ:

```bash
keytool -genkeypair -v -keystore release.keystore -alias protogrid \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 release.keystore
```

Храните keystore и пароли: потеря ключа означает, что обновление поверх
установленного APK больше не встанет.

## Ограничения MVP

- релизный APK — `arm64-v8a`, для эмулятора собирается `x86_64`;
- Lite-сборка принимает только VLESS/Reality по TCP; остальные профили
  импортируются, но при подключении ядро вернёт ошибку «ProtoGrid-core Lite
  поддерживает только VLESS» / «…только транспорт TCP»;
- нет импорта подписок и `ovpn://` (парсинг уже есть в ядре — подключим дальше);
- правила по процессам пока не выведены; свои домены, суффиксы, IP/CIDR и GeoIP/GeoSite работают;
- TUN-стек `gvisor`, системный прокси Android не используется;
- профили (включая UUID и пароли) хранятся в приватном `SharedPreferences`
  открытым текстом, как `config.json` на desktop;
- приложение не публикуется в Google Play, только GitHub Releases.
