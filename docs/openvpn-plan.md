# OpenVPN в ProtoGrid: план работ

Цель: добавить полноценный клиент OpenVPN в ProtoGrid через штатный
`openvpn-client` endpoint sing-box 1.14+, с импортом файлов `.ovpn`, без
внешнего бинарника `openvpn` и без второго TUN-устройства.

## Что уже есть

- `forks/sing-box` основан на v1.14.4, где OpenVPN — штатный endpoint:
  `protocol/openvpn`, `transport/openvpn`, `include/openvpn.go`.
- Официальные сборки sing-box включают `with_openvpn`
  (`release/DEFAULT_BUILD_TAGS`), поэтому стоковое ядро ≥ 1.14 тоже подходит.
- Сборка форка (`scripts/build-fork.sh`) этот тег **не** включает, поэтому
  сейчас бинарник форка отвечает `OpenVPN is not included in this build`.
- Endpoint виден через outbound-менеджер (`adapter/outbound/manager.go:160`),
  поэтому его тег можно указывать в `selector`/`urltest`, а Clash API отдаёт
  его как обычный прокси (`experimental/clashapi/proxies.go:96`). Пинг и
  маршрутизация работают без отдельной схемы.

## Решения

- Профиль `.ovpn` хранится в `Server.openvpn_profile` как нормализованный
  текст: внешние файлы (`ca`, `cert`, `key`, `tls-auth`, `tls-crypt`,
  `tls-crypt-v2`, `secret`) при импорте встраиваются inline-блоками.
- Логин/пароль `auth-user-pass` — отдельные поля `openvpn_username` и
  `openvpn_password`; профиль может содержать их inline.
- В конфиг OpenVPN попадает в top-level `endpoints`, а не в `outbounds`.
  Id сервера остаётся в `selector`/`urltest` `proxy`, `route.final` не меняется.
- `system: false` и `with_gvisor` — чтобы не требовать root и не создавать
  системный интерфейс.
- Ядро Xray OpenVPN не поддерживает; стоковый sing-box требует версии ≥ 1.14.
- Obfuscation (`scramble`, XOR, obfsproxy), `--plugin`, TAP/L2 и DCO не
  поддерживаются и в план не входят.

## TODO

### Ядро (protogrid-core)

- [x] `Protocol::OpenVpn` (label `OpenVPN`, порт 1194, только sing-box-like)
- [x] `CoreCapabilities.openvpn`, проверка в `validate_with`
- [x] Поля `Server.openvpn_profile`, `openvpn_username`, `openvpn_password`
- [x] Модуль `openvpn.rs`: разбор `.ovpn`, inline-файлы, `import_profile`,
      `endpoint`, `looks_like_profile`
- [x] Генерация `endpoints` в `singbox.rs`, сохранение id в селекторе
- [x] Явные ошибки для Xray и для ядра без `with_openvpn`
- [x] Импорт: `parse_links` принимает сырой `.ovpn` и `ovpn://`/`openvpn://`
      (base64)
- [x] Тесты: парсер, конфиг, импорт, capability-gate

### Приложение (protogrid-desktop)

- [x] `CoreFeatures.openvpn` + probe + эвристика по версии ≥ 1.14
- [x] `CoreInfo::supports_openvpn`, `Snapshot::capabilities`, сборка конфига
- [x] Выбор протокола, цвет, редактор профиля и логина/пароля
- [x] Загрузка `.ovpn` файла через `rfd` (с inline-файлами)
- [x] Импорт из модального окна: вставка профиля и файл
- [x] Совместимость с ядром в UI и подсказки

### Сборка и документация

- [x] `with_openvpn` в `scripts/build-fork.sh` и CI
- [x] Версия форка `1.14.4-protogrid.3`
- [x] README: импорт `.ovpn`, ограничения OpenVPN
- [x] Матрица в `docs/xray-parity-plan.md`

### Проверка

- [x] `cargo fmt --all -- --check`
- [x] `cargo test -p protogrid-core` — 87 тестов
- [x] `cargo test -p protogrid-desktop` — 8 тестов
- [x] `cargo check -p protogrid-desktop`
- [x] `git status` / `git diff` без посторонних правок

Форк пересобран с `with_openvpn` (Go 1.27.4, версия `1.14.4-protogrid.3`).
Probe использует валидный TLS-профиль с тестовым CA, поэтому `sing-box check`
проходит и бейдж OpenVPN включается только тогда, когда ядро действительно
собрано с этим тегом.

## Ограничения

- OpenVPN работает только через sing-box-подобное ядро с `with_openvpn`.
- Профиль содержит приватные ключи и пароли — хранится в `config.json`
  открытым текстом, как и остальные секреты приложения; в логи не пишется.
- Несколько `remote` поддержаны, failover выполняет endpoint.
- `<connection>`-блоки не раскрываются: нужен top-level `remote`.
- Для UDP-серверов TCP-пинг не работает; реальная задержка видна через
  Clash API, когда ядро запущено.
- Серверные `push` маршруты используются endpoint'ом; DNS идёт через
  `proxy` (selector → endpoint), как и для остальных протоколов.
