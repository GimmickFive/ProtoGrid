# ProtoGrid Android

Тонкий Android-клиент ProtoGrid: Kotlin/Compose сверху, `VpnService` + ProtoGrid-core
(`libbox`) для туннеля и общий Rust-код для ссылок и конфигурации.

Это MVP: подключение, серверы и подписки, импорт ссылок и QR, маршрутизация по
приложениям и трафику, мониторинг (график скорости и счётчики из Clash API).
Правил по процессам и пинга пока нет.

## Навигация

Пять вкладок снизу. Соседние вкладки также переключаются горизонтальным
свайпом по содержимому: влево — следующий раздел, вправо — предыдущий.

- **Подключение** — статус туннеля, активный сервер, маршрут, трафик сессии и
  баннер обновления.
- **Серверы** — серверы и подписки (внутренние вкладки), импорт ссылок.
- **Маршруты** — per-app VPN и правила трафика.
- **Трафик** — график скорости, счётчики и длительность сессии.
- **Настройки** — постоянная работа VPN и автозапуск, уведомления, виджет,
  обновления и «О приложении».

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

## Импорт

Одно поле принимает всё: `vless://`, `vmess://`, `trojan://`, `ss://`,
`hysteria2://`, `tuic://`, `ovpn://`, URL подписки, Clash YAML и профиль `.ovpn`.
Формат определяется автоматически: одиночный `http(s)://` — подписка, остальное —
ссылки на серверы или профиль.

- **Вставить из буфера** — подставляет текст в поле.
- **Сканировать QR** — камера (если устройство её имеет).
- **QR из фото** — разбор QR с изображения из галереи.
- **Добавить конфиг `.ovpn`** — только в обычной сборке; Lite OpenVPN не поддерживает.

Lite-сборка принимает только VLESS/Reality по TCP: остальные серверы при импорте
пропускаются с предупреждением.

## Маршруты

Два независимых режима на экране **«Маршруты»**.

**Приложения** — это per-app VPN Android:

- «Все» — туннель для всех;
- «Только выбранные» — в туннель попадают только отмеченные приложения;
- «Все кроме» — в туннель идут все, кроме отмеченных.

Для списка нужен `QUERY_ALL_PACKAGES` (нормальное разрешение, для GitHub-сборки
допустимо). Изменения применяются к активному туннелю кнопкой
**«Применить и переподключить»**.

**Правила** — первая вкладка (открывается по умолчанию) с пресетами и списком
правил. По умолчанию выбран **«Жизнь в РФ»**; смена пресета на активном туннеле
применяется кнопкой **«Применить и переподключить»**. Под пресетами виден
список правил: у встроенного пресета он только для чтения, рядом стоят
**«+ Добавить»** (заблокировано) и **«Копировать»** — копия создаёт «Свой
профиль».

- «Глобальный прокси» — всё через сервер, локальная сеть напрямую;
- «Жизнь в РФ» — RU-домены, `geoip ru`/`category-ru` и локальная сеть
  напрямую, остальное через прокси;
- «Прямой» — без прокси;
- «Свой профиль» — финальное действие и свои правила: домен, суффикс,
  ключевое слово, IP/CIDR, GeoIP, GeoSite; действия «Прокси / Напрямую / Блок».

GeoIP/GeoSite требуют `.srs`-базы. Стартовый слепок `geoip-ru.srs` и
`geosite-category-ru.srs` зашит в `libprotogrid_mobile.so` (`assets/rulesets`)
и копируется в `filesDir/rulesets`, если кэш пуст. Кнопка **«Скачать / обновить базы»** принудительно обновляет
базы с GitHub SagerNet, сервис делает это автоматически раз в 7 дней. Если
GitHub недоступен, используется зашитая или последняя скачанная копия.

Правила уходят в `build_config_with_capabilities` с `rule_set_dir`; per-app
списки — в `OverrideOptions` (`includePackage` / `excludePackage`) и далее в
`VpnService.Builder`.

## Постоянная работа VPN

Сам туннель держит Android: `ProtoGridVpnService` работает как foreground-сервис
с постоянным уведомлением, поэтому Doze и режим ожидания приложения на него не
действуют. Достаточно двух вещей:

- **Системный «Всегда включённый VPN»** — кнопка «Открыть настройки VPN» ведёт в
  системный экран VPN, где включается этот режим (и, при необходимости,
  «Блокировать подключения без VPN»). Android сам поднимает сервис после
  перезагрузки и перезапускает его, если он остановился. При этом нажатие
  «Стоп» в уведомлении или плитке не разрывает туннель.
  В разделе «Настройки → Постоянная работа» показывается живой статус
  always-on: жёлтая подсказка, если он выключен, зелёная отметка, если включён.
  Состояние обновляется сервисом каждые несколько секунд; на Android 10+
  источник — `VpnService.isAlwaysOn()` внутри сервиса, из Activity флаг недоступен.
- **«Автозапуск»** — резервный путь для прошивок без включённого always-on:
  после перезагрузки `BootReceiver` поднимает туннель, если разрешение на VPN
  уже выдано и есть хотя бы один сервер. `BOOT_COMPLETED` — обычное разрешение,
  отдельного диалога ОС для него нет. При включении показывается диалог с
  инструкцией и кнопкой «Открыть настройки»; после возврата приложение спрашивает
  подтверждение, и «Нет / не нашёл» выключает тумблер. На Xiaomi/Huawei/Oppo/Vivo
  автозапуск дополнительно разрешают в приложении безопасности производителя.
  Туннель появляется не мгновенно: доставка широковещания и старт
  foreground-сервиса на эмуляторе заняли около 25 секунд после загрузки.

Исключение из энергосбережения и «Приостановка неиспользуемых» удалены: для
foreground-VPN они не нужны и только загрязняли настройки. Если на вендорной
прошивке туннель всё же отключается, «Без ограничений» можно включить вручную в
системных настройках ProtoGrid → Батарея — отдельного тумблера в приложении для
этого больше нет.

## Предупреждение при установке

Приложение само не скачивает и не устанавливает APK: кнопка обновления открывает
`https://github.com/GimmickFive/ProtoGrid/releases/latest`, а файл ставится
вручную через браузер/системный установщик. Разрешение
`REQUEST_INSTALL_PACKAGES` и `FileProvider` удалены, поэтому больше нет
встроенного APK-инсталлера, из-за которого Play Protect видел dropper-профиль
«VPN + установка APK».

Стартовый диалог Play Protect для свежего sideload-APK может остаться; это
системная эвристика, а не ошибка подписи: релизы подписаны `CN=ProtoGrid` и
проверяются в CI. Разбор и варианты решения —
[`docs/android-sideload-warning.md`](../../docs/android-sideload-warning.md).

## Локальная сборка

Нужны Linux/macOS, JDK 17, Android SDK 37 (Build Tools 36.0.0) и платформа 24
(libbox собирается с `-androidapi 24`), NDK 28.0.13004108, Go 1.27, Rust target
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

Workflow `.github/workflows/android.yml` запускается по тегу `v*` (вместе с
десктопным релизом), по тегу `android-v*` (только Android) или вручную. Он
собирает libbox (полный и Lite), Rust `.so`, оба APK и прикрепляет файлы к
Release. Теговые сборки требуют release-keystore и проверяют, что APK не
подписан debug-сертификатом.

Приложение проверяет публичные релизы `GimmickFive/ProtoGrid` на GitHub при
запуске. Если найдена более новая версия с APK под ABI устройства (обычно
`ProtoGrid-<версия>-arm64-v8a.apk`, для Lite —
`ProtoGrid-<версия>-lite-arm64-v8a.apk`), на «Подключении» появляется баннер,
а полная карточка — в «Настройках». Кнопка «Открыть последний релиз на GitHub»
ведёт на `https://github.com/GimmickFive/ProtoGrid/releases/latest`; GitHub сам
переадресует на последний релиз. Скачивание и установка APK остаются за
пользователем: приложение не запрашивает `REQUEST_INSTALL_PACKAGES`, не хранит
APK и не открывает системный установщик. Lite-сборка учитывает только Lite-ассет,
поэтому варианты не перепутаются. Ядро ProtoGrid-core находится внутри APK,
поэтому отдельного скачивания ядра на Android нет — оно обновляется вместе с
приложением.

Секреты для стабильной подписи:

| Секрет | Значение |
|---|---|
| `PROTOGRID_KEYSTORE_BASE64` | новый `release.keystore` в base64 |
| `PROTOGRID_KEYSTORE_PASSWORD` | пароль нового хранилища |
| `PROTOGRID_KEY_ALIAS` | alias нового ключа |
| `PROTOGRID_KEY_PASSWORD` | пароль нового ключа |
| `PROTOGRID_LEGACY_KEYSTORE_BASE64` | старый debug-keystore в base64 |
| `PROTOGRID_LEGACY_KEYSTORE_PASSWORD` | пароль старого хранилища |
| `PROTOGRID_LEGACY_KEY_ALIAS` | alias старого ключа |
| `PROTOGRID_LEGACY_KEY_PASSWORD` | пароль старого ключа |

`apps/protogrid-android/signing/lineage.bin` связывает старый и новый ключи.
CI подписывает v1/v2 старым ключом, v3 — новым с lineage, поэтому обновления
поверх 0.3–0.4.1 устанавливаются без удаления приложения.

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
- Lite-сборка принимает только VLESS/Reality по TCP; остальные серверы
  пропускаются при импорте с предупреждением;
- правила по процессам пока не выведены; свои домены, суффиксы, IP/CIDR и GeoIP/GeoSite работают;
- TUN-стек `gvisor`, системный прокси Android не используется;
- профили (включая UUID и пароли) хранятся в приватном `SharedPreferences`
  открытым текстом, как `config.json` на desktop;
- приложение не публикуется в Google Play, только GitHub Releases.
