# План: TUN + XHTTP в одном процессе (форк sing-box)

## Цель

Получить одно ядро, которое одновременно умеет:

- TUN-режим (уже есть в sing-box);
- Hysteria2, TUIC, selector, Clash API (уже есть);
- XHTTP-транспорт, совместимый с сервером Xray.

Выбран путь: **форк sing-box v1.14.4 + клиентская реализация XHTTP**. Форк Xray отвергнут: там пришлось бы дописывать TUN, Hysteria2 и TUIC.

## Границы первой версии (MVP)

Входит:

- XHTTP-клиент поверх HTTP/2 + TLS/Reality;
- режимы `stream-one`, `stream-up`, `packet-up`, `auto`;
- `host`, `path`, `headers`, padding (`Referer` + `x_padding`);
- генерация конфига в ProtoGrid и feature-detection форка.

Не входит:

- проверка через CDN и оптимизация батчинга;
- HTTP/3;
- `xmux`, `downloadSettings`, серверная сторона XHTTP;
- полноценная матрица со всеми версиями Xray.

## Архитектура

```
ProtoGrid (Rust)
  └─ sing-box config: outbound vless/vmess/trojan
       └─ transport: { "type": "xhttp", ... }
            └─ форк sing-box: transport/v2rayxhttp
                 ├─ stream-one: один POST, body/response = двунаправленный поток
                 ├─ stream-up: GET (downlink) + POST (uplink), session id в path
                 └─ packet-up: POST на каждый чанк, seq в path
```

Клиент должен повторять wire-протокол Xray:

- session id и seq по умолчанию добавляются к пути;
- padding по умолчанию передаётся в `Referer` как `x_padding`;
- для `stream-up`/`stream-one` upload идёт с `Content-Type: application/grpc`;
- downlink — долгоживущий ответ, который нельзя буферизовать.

## Этапы

### M0. Подготовка (сделано)

- [x] Клон `SagerNet/sing-box` v1.14.4 в `forks/sing-box`.
- [x] Ветка `protogrid/xhttp`.
- [x] Изучены `transport/v2rayhttp`, `option/v2ray_transport.go`, клиент Xray (`splithttp`).
- [x] План и структура интеграции.

Критерий: форк собирается, ветка создана.

### M1. Каркас XHTTP в sing-box

- [x] `constant.V2RayTransportTypeXHTTP`.
- [x] `option.V2RayXHTTPOptions` + marshal/unmarshal/schema.
- [x] `transport/v2rayxhttp` с `NewClient`.
- [x] Ветка в `transport/v2ray/transport.go`.
- [x] Юнит-тесты разбора конфига.

Критерий: `go test ./transport/v2rayxhttp/... ./option/...` проходит; неизвестный транспорт больше не ошибка.

### M2. stream-one / stream-up

- [x] `DialContext` для `stream-one`: POST + `io.Pipe`, чтение ответа.
- [x] `DialContext` для `stream-up`: GET downlink + POST uplink, session id.
- [x] `net.Conn`-обёртка, Close, отмена запросов.
- [x] `Referer` с padding, браузерные заголовки по умолчанию.
- [x] Интеграционный тест с локальным Xray XHTTP inbound.

Критерий: TCP echo и HTTPS-запрос через локальный Xray проходят в обоих режимах.

### M3. Интеграция в ProtoGrid

- [x] `singbox::build_config` умеет `Transport::Xhttp`.
- [x] Feature-detection: XHTTP разрешён только для форка (суффикс версии `-protogrid`).
- [x] Снятие запретов в `model.rs` и `ui.rs` только при поддержке ядром.
- [x] Тест: стоковый sing-box не получает XHTTP-конфиг.

Критерий: ProtoGrid подключается к XHTTP-серверу через форк; со стоковым ядром поведение прежнее.

### M4. TUN end-to-end

- [x] TUN + XHTTP на Linux (root/CAP_NET_ADMIN).
- [x] TCP и UDP (VLESS XUDP) через TUN.
- [x] DNS и QUIC-приложения: XHTTP/3 (QUIC) проверен, DNS/UDP работают.

Критерий: браузер и `curl` работают через TUN, UDP не ломается.

### M5. packet-up и CDN

- [x] `packet-up`, `auto`, sequence, padding.
- [x] Батчинг (`sc_max_each_post_bytes`, `sc_min_posts_interval_ms`).
- [x] Проверка через nginx/Cloudflare-подобный прокси (TLS reverse proxy, packet-up TCP+UDP).

Критерий: соединение через CDN, большие загрузки, реконнекты.

### M6. HTTP/3 и полировка

- [x] HTTP/3 через quic-go.
- [x] `xmux` (HTTP/2/3 pooling) и `downloadSettings` (принимается; downlink идёт через основной сервер).
- [x] Матрица версий Xray, CI, релизы (`.github/workflows/ci.yml`, `scripts/build-fork.sh`).

Критерий: полная совместимость с основными режимами Xray.

## Изменения в ProtoGrid

| Файл | Что меняется |
|---|---|
| `crates/protogrid-core/src/singbox.rs` | `Transport::Xhttp` → объект XHTTP; feature-флаг |
| `crates/protogrid-core/src/model.rs` | запрет XHTTP только если ядро не умеет |
| `apps/protogrid-desktop/src/engine.rs` | передача feature-флага в генератор |
| `apps/protogrid-desktop/src/installer.rs` | определение форка по версии |
| `apps/protogrid-desktop/src/ui.rs` | фильтры и подписи транспорта |

## Тестовая матрица

- Xray: `auto`, `stream-one`, `stream-up`; TLS и Reality.
- HTTP/2; позже HTTP/1.1 и HTTP/3.
- TCP, UDP, большие файлы, half-open, реконнект.
- ProtoGrid: запуск, пинг, переключение серверов, TUN.

## Риски

1. Wire-протокол Xray может отличаться между версиями.
2. UDP over XHTTP требует отдельной проверки.
3. CDN и `packet-up` — главный источник нестабильности.
4. GPL-3.0: форк и бинарники распространяются с исходниками.
5. Форк нужно rebase-ить на upstream.

## Сборка форка

```bash
cd forks/sing-box
go build -tags "with_quic,with_utls,with_clash_api,with_gvisor" \
  -ldflags "-X github.com/sagernet/sing-box/constant.Version=1.14.4-protogrid.1" \
  ./cmd/sing-box
```

Для ML-DSA-65 (Reality Xray 26) нужен **Go 1.27+**; без него собирается
заглушка, а `mldsa65_verify` игнорируется.

Версия форка должна печататься как `sing-box version 1.14.4-protogrid.1`, чтобы ProtoGrid отличал его от стокового.

## Публикация форка

Локальный форк: `forks/sing-box`, ветка `protogrid/xhttp`.
Он исключён из git ProtoGrid через `/forks`, поэтому изменения также
сохранены патчами в `docs/patches/`.

```bash
cd forks/sing-box
git remote add fork git@github.com:<account>/sing-box.git
git push -u fork protogrid/xhttp
```

Патчи можно применить к чистому `v1.14.4`:

```bash
git am docs/patches/*.patch
```

## Статус

- M0 — выполнено: клон v1.14.4, ветка `protogrid/xhttp`, план.
- M1 — выполнено: `xhttp` в option/constant/transport switch, пакет `transport/v2rayxhttp`, юнит-тесты.
- M2 — выполнено для HTTP/2 + TLS: `stream-one`, `stream-up`, padding, session id.
- M3 — код интеграции в ProtoGrid готов: capability-флаг, feature-detection по версии `-protogrid`, UI/валидация.
- M4 — выполнено для TLS и Reality (Xray 25): TUN + XHTTP, DNS и UDP.
- M5 — packet-up, батчинг и CDN-подобный reverse proxy проверены.
- M6 — HTTP/3, `xmux`/`downloadSettings` (приняты), CI и build-скрипт готовы.

### Что уже проверено вручную

Стенд: Xray 26.11.9 (XHTTP inbound, TLS, `mode: auto`) + форк sing-box.

- TCP через `stream-one`, `stream-up` и `packet-up` — работает.
- UDP (VLESS XUDP) через все три режима — работает (DNS-запрос к 8.8.8.8:53).
- XHTTP/3 (QUIC, `alpn=h3`): TCP и UDP — работают.
- mKCP (VLESS через Xray mKCP inbound): TCP и UDP — работают.
- VLESS Encryption (`mlkem768x25519plus`, Xray 26): TCP и UDP — работают.
- packet-up через TLS reverse proxy (CDN-подобный сценарий): TCP и UDP — работают.
- Батчинг packet-up: `sc_max_each_post_bytes` / `sc_min_posts_interval_ms`.
- В ProtoGrid добавлено поле XHTTP mode: `auto`, `packet-up`, `stream-up`, `stream-one`.
- 8 параллельных загрузок по 5 МБ — checksum совпадает.
- Конфиг с `selector` + XHTTP outbound — работает.
- Полный конфиг, сгенерированный ProtoGrid (`selector`, rule-sets, Clash API) — работает; Clash API отдаёт `sing-box 1.14.4-protogrid.1`.
- TUN + XHTTP (Reality, Xray 25.3.6): `tun0` поднимается, DNS и HTTPS идут через туннель, `curl https://example.com` возвращает 200.
- Reality с Xray 26.11.9: TCP и XHTTP работают (ML-KEM), с Xray 25.3.6 — через автоматический fallback на X25519.
- Версия форка: `sing-box version 1.14.4-protogrid.1`.

### Reality и Xray 26

Xray 26 отвергает Reality ClientHello без `X25519MLKEM768`, а sing-box 1.14.4
раньше специально удалял этот key share. В форке это исправлено: клиент
отправляет ML-KEM, а при неудаче автоматически повторяет handshake только с
X25519 для старых серверов Xray 25.

Ограничения текущего MVP:

- `downloadSettings` принимается, но downlink остаётся на основном сервере;
- `xmux` не имеет отдельных настроек: используется пул HTTP/2/3;
- серверная сторона XHTTP в форке отсутствует (нужен только клиент).

### Сборка версии форка

```bash
cd forks/sing-box
go build -tags "with_quic,with_utls,with_clash_api,with_gvisor" \
  -ldflags "-X github.com/sagernet/sing-box/constant.Version=1.14.4-protogrid.1" \
  -o sing-box ./cmd/sing-box
```

`with_clash_api` обязателен: ProtoGrid включает Clash API для замера задержек.
`with_gvisor` обязателен для TUN.
Без `-ldflags` версия будет `unknown`, и ProtoGrid не включит XHTTP.

