# Changelog

Все заметные изменения ProtoGrid. Формат — [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
версии — [Semantic Versioning](https://semver.org/lang/ru/).

Ветка `dev` — разработка, `main` — только релизы. Запись попадает сюда в момент
выпуска, порядок действий — в [docs/releasing.md](docs/releasing.md).

## [Unreleased]

## [0.1.3] — 2026-11-26

### Исправлено

- Карточка «Свой профиль» в маршрутизации обрезала подпись при большом числе
  правил.

### Изменено

- Скриншоты интерфейса пересняты на актуальной сборке.

## [0.1.2] — 2026-11-26

Первая публичная бета. Windows-сборка, Linux проверен на реальном подключении.

### Добавлено

- **OpenVPN**: импорт `ovpn://`/`openvpn://` и файлов `.ovpn`, TLS,
  `auth-user-pass`, `tls-auth`/`tls-crypt`/`tls-crypt-v2`, UDP и TCP через
  endpoint sing-box 1.14+ с тегом `with_openvpn`.
- **GeoIP/GeoSite**: кэш баз правил и флаги стран в списке серверов.
- **Интерфейс**: переработанный нативный лейаут — сайдбар, обзор с живыми
  графиками, карточки, таблица серверов, тёмная и светлая темы, поиск `Ctrl+K`,
  быстрые переходы `Ctrl 1…7`, бейджи протоколов.
- **Windows-пакеты**: установщик NSIS и портативный zip; `protogrid-core.exe` и
  `wintun.dll` лежат рядом с `ProtoGrid.exe`, а не внутри него.
- Лаунчеры `run-protogrid*.bat` и `run-portable*.bat`.

### Изменено

- Ядро `protogrid-core` вынесено в отдельный процесс и больше не встраивается в
  лончер: приложение стартует быстрее и не раздувается.
- Ядро ProtoGrid (форк sing-box) подхватывается рядом с приложением; стоковые
  sing-box и Xray-core скачиваются из настроек.
- Релизный профиль: `opt-level = "s"`, LTO, `codegen-units = 1`, `strip`.
- Установщик и портативный архив включают тексты лицензий: MIT, GPLv3 для ядра
  форка sing-box и GPLv2 для `wintun.dll`, плюс `THIRD_PARTY_NOTICES`.

### Исправлено

- Журнал дублировал префикс ядра в каждой строке.
- Портативный профиль больше не попадает в zip-архив.

[Unreleased]: https://github.com/GimmickFive/ProtoGrid/compare/v0.1.3...dev
[0.1.3]: https://github.com/GimmickFive/ProtoGrid/releases/tag/v0.1.3
[0.1.2]: https://github.com/GimmickFive/ProtoGrid/releases/tag/v0.1.2
