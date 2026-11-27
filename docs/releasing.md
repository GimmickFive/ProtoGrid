# Выпуск релиза

Ветка `main` — только релизы и `CHANGELOG.md`. Разработка идёт в `dev`; релиз —
это слияние `dev` в `main` по команде мейнтейнера.

## 1. Подготовка версии

1. Поднять версию один раз — в `Cargo.toml`, `[workspace.package].version`.
   Крейты наследуют её через `version.workspace = true`.
2. Обновить `APP_VERSION` и `VIProductVersion` в `installer/protogrid.nsi`.
3. Если менялся форк — поднять `VERSION` и `LITE_VERSION` в
   `scripts/build-fork.sh` и `ldflags` в `.github/workflows/ci.yml`
   (`1.14.4-protogrid.N` / `1.14.4-protogrid.N-lite.M`).
4. Добавить запись в `CHANGELOG.md`: секция `## [X.Y.Z] — YYYY-MM-DD`, ссылки
   внизу файла.
5. Обновить README, если изменились возможности, требования или ограничения.

## 2. Проверки

```bash
cargo fmt --all -- --check
cargo test -p protogrid-core
cargo check -p protogrid-desktop
cargo check -p protogrid-mobile
```

Плюс то же, что делает CI для форка: применить `docs/patches/*.patch` к
`SagerNet/sing-box` `v1.14.4`, прогнать `go test` и `go build`.

## 3. Слияние в main

```bash
git switch dev
git pull --ff-only
git switch main
git merge --no-ff dev -m "release: vX.Y.Z"
```

Слияние только fast-forward-safe: конфликты решаются в `dev`, а не в `main`.

## 4. Сборка артефактов

Windows-пакеты (установщик NSIS + портативный zip):

```bash
rustup target add x86_64-pc-windows-gnu
./scripts/package-windows.sh
```

Ядро форка, если релиз его затрагивает:

```bash
./scripts/build-fork.sh
```

Артефакты складываются в `dist/` и в git не попадают — они прикрепляются к
релизу на GitHub.

### Android APK

Android-клиент собирается отдельным workflow `.github/workflows/android.yml`
по тегу `android-vX.Y.Z` (например, `android-v0.2.0`). Локально можно проверить
сборку командой:

```bash
ANDROID_HOME=/path/to/sdk ./scripts/build-android.sh
```

Для подписанного релиза в GitHub Actions должны быть заданы секреты
`PROTOGRID_KEYSTORE_BASE64`, `PROTOGRID_KEYSTORE_PASSWORD`,
`PROTOGRID_KEY_ALIAS` и `PROTOGRID_KEY_PASSWORD`. Без них тег `android-v*`
завершится ошибкой.

### Имена файлов для автообновления

Встроенная проверка обновлений читает публичные релизы GitHub, поэтому имена
ассетов важны:

- десктоп — `ProtoGrid-<версия>-windows-x64-setup.exe` и
  `ProtoGrid-<версия>-windows-x64-portable.zip`;
- Android — `ProtoGrid-<версия>-arm64-v8a.apk` (обычное ядро) и
  `ProtoGrid-<версия>-lite-arm64-v8a.apk` (ProtoGrid-core Lite), при сборке
  под эмулятор — `ProtoGrid-<версия>-x86_64.apk` и
  `ProtoGrid-<версия>-lite-x86_64.apk`.

Релиз с ассетами APK может иметь тег `vX.Y.Z` (как в 0.2.1) или
`android-vX.Y.Z`; десктопная проверка теги `android-*` игнорирует. GitHub
отдаёт SHA-256 ассета в поле `digest`, и клиенты сверяют его после загрузки.
Если файла для платформы нет, приложение предлагает открыть страницу релиза.

## 5. Тег и публикация

```bash
git tag -a vX.Y.Z -m "ProtoGrid vX.Y.Z"
git push origin main
git push origin vX.Y.Z
gh release create vX.Y.Z dist/*.zip dist/*setup.exe \
  --title "ProtoGrid vX.Y.Z" --notes-file <(sed -n '/## \[X.Y.Z\]/,/^## \[/p' CHANGELOG.md)
```

Если это бета, в заголовке релиза и в notes явно писать «public beta» и
перечислять известные ограничения из README.

## 6. После релиза

- `dev` продолжает разработку; `main` не трогается до следующего релиза.
- Ссылки в README, `CHANGELOG.md` и релизе должны указывать на `vX.Y.Z`.
- Проверить, что CI зелёный и артефакты скачиваются.
