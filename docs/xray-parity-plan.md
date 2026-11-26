# Паритет с Xray: что добавить в форк sing-box

Принцип: **база — sing-box**, в неё добавляем всё, что есть в Xray и чего нет в sing-box. TUIC, selector и Clash API остаются преимуществами sing-box.

## Матрица

| Возможность | Xray 26 | sing-box 1.14.4 | Статус в форке |
|---|---|---|---|
| TUN | да | да | есть |
| Hysteria2 | да | да | есть |
| TUIC | нет | да | есть, преимущество sing-box |
| OpenVPN | нет | да, endpoint с 1.14 | включено в форке (with_openvpn) |
| XHTTP: stream-one / stream-up / packet-up | да | нет | добавлено |
| XHTTP: батчинг, CDN, `xmux`, `downloadSettings` | да | нет | батчинг/CDN; `xmux` pooling; `downloadSettings` с отдельным address/host/path и соединением |
| XHTTP: HTTP/3 | да | нет | добавлено |
| Reality: X25519 | да | да | есть |
| Reality: X25519MLKEM768 (Xray 26) | да | нет | добавлено + fallback на X25519 |
| Reality: ML-DSA-65 (`mldsa65Verify`) | да | нет | реализовано, Go 1.27+; проверено на production Xray 26.10.21 (TCP/UDP через SSH-туннель) |
| mKCP | да | нет | добавлено, проверено с Xray 26 (TCP/UDP) |
| VLESS Encryption / post-quantum | да | нет | добавлено, проверено с Xray 26 (TCP/UDP) |
| XDRIVE | да | нет | local и S3 проверены TCP/UDP; Google Drive не тестировался |
| MASQUE | да | нет | outbound-адаптер + first-class в ProtoGrid; TCP/UDP проверены |
| WireGuard | да | да | есть |
| Balancers: leastPing / leastLoad | да | частично (`urltest`) | sing-box `urltest`; Xray `observatory` + `leastLoad` |
| fakedns | да | нет | FakeDNS через sing-box `fakeip` |
| sockopt / finalmask / fragment / noise | да | частично | TFO, interface, mark, fragment; `finalmask.udp` noise в форке (mKCP и UDP-диалеры QUIC/HTTP3); произвольные TCP/UDP finalmask-маски JSON — только Xray |
| Xray Stats API | да | Clash API | совместимость через Clash API |
| Серверные inbounds Xray | да | частично | не цель клиента |

## Порядок работ

### P0 — совместимость с серверами (выполнено)

1. **Reality ML-KEM** — сделано, проверено с Xray 26 и 25.
2. **Reality ML-DSA-65** — приём `mldsa65Verify`, проверка подписи, Go 1.27+; проверено на production.
3. **XHTTP полный** — stream-one/up/packet-up, batching, CDN, H3; `xmux` через pooling; `downloadSettings` с отдельным downlink-соединением.
4. **mKCP** — сделано через реализацию Xray, проверено с Xray 26.
5. **VLESS Encryption** — сделано, проверено с Xray 26.

### Reality ML-DSA-65 — проверено

`mldsa65_verify` реализован и проверен на production-сервере Xray 26.10.21
с `mldsa65Seed`. Прямой внешний порт был недоступен из окружения, поэтому
использован SSH-туннель к серверу. Результат:

```text
Xray-клиент:        200
Наш sing-box:       200
UDP DNS:            71 байт
```

Сборка форка требует Go 1.27+ для `crypto/mldsa`; на Go < 1.27 включается
заглушка, и `mldsa65_verify` игнорируется.

### P1 — маршрутизация и поведение

6. Balancers: sing-box `urltest`; Xray `observatory` + `leastLoad` — сделано.
7. fakedns: FakeDNS через sing-box `fakeip` — сделано.
8. sockopt/finalmask: TFO, interface, mark, fragment; noise (`finalmask.udp`) реализован в форке для UDP-сокетов — mKCP через Xray-совместимый `MemoryStreamConfig.FinalMask` и QUIC/UDP-диалеры через обёртку `dialer`. Raw JSON для произвольных TCP/UDP масок и `filter`/`header-custom` остаются только в Xray.
9. Полный набор полей XHTTP/Reality/TLS из share-ссылок и Clash — частично.

### P2 — экзотика

10. XDRIVE — local и S3 (локальный S3-совместимый сервер) проверены TCP/UDP. Google Drive требует OAuth-учётных данных и остаётся непроверенным.
11. MASQUE — outbound-адаптер + модель/UI/импорт ссылок/Clash; TCP (`301`) и UDP DNS (`71` байт) проверены.
12. Серверные inbounds (если понадобится запускать сервер, а не клиент).
13. Точный паритет Stats API.

## Проверка

Для каждого пункта:

- юнит-тест генерации конфига;
- интеграционный тест с реальным Xray-сервером;
- проверка через TUN и локальный прокси;
- обратная совместимость со старыми Xray (fallback).

### Результаты P0

- Reality Xray 26.11.9 (TCP и XHTTP) — 200; Xray 25.3.6 — через fallback.
- XHTTP stream-one / stream-up / packet-up — TCP и UDP.
- XHTTP через TLS reverse proxy (CDN-подобный) — TCP и UDP.
- XHTTP/3 (`alpn=h3`, QUIC) — TCP и UDP.
- mKCP (Xray 26 inbound) — TCP и UDP.
- VLESS Encryption (`mlkem768x25519plus`, Xray 26) — TCP и UDP.
- XDRIVE (`local` и `s3`): TCP и UDP — работают.
- Reality ML-DSA-65: production Xray 26.10.21 + SSH-туннель — Xray-клиент 200, наш sing-box 200, UDP DNS 71 байт.
- MASQUE: локальный Xray MASQUE-сервер + TLS-пиннинг; TCP → `301`, UDP DNS → `71` байт.
- XHTTP downloadSettings: отдельный host/path/соединение (`DL GET /dl/...`), TCP → `301`, UDP DNS → `71` байт.
- Finalmask: `fragment`, `noise`, `filter`, `header-custom`; `xray run -test` → `Configuration OK`.
- Finalmask noise в форке: mKCP и QUIC (`transport: quic`) — первый UDP-пакет к серверу `abcd`, затем KCP/QUIC-пакеты (проверено локальным сниффером); `sing-box check` принимает `finalmask`.
- `cargo test --workspace`: 46 + 1 тест; `cargo clippy` без предупреждений.
- FakeDNS: `dns.final=dns-remote`, правило A/AAAA → fakeip; `dig @1.1.1.1 example.com` вернул `198.18.0.2`.
- auto-select: `urltest` вместо `selector`; Xray — `observatory` + `leastLoad`, `xray run -test` → `Configuration OK`.
- sockopt: `tcpFastOpen`, `interface`, `mark`; finalmask: fragment/noise; `xray run -test` → `Configuration OK`.
- sing-box TLS fragment: `fragment/record_fragment`; `sing-box check` → `CHECK_EXIT:0`.
- Go-тесты форка: `protocol/vless`, `transport/v2rayxhttp`, `option`, `common/tls`.

## Ссылки

- План XHTTP: `docs/xhttp-fork-plan.md`
- Патчи форка: `docs/patches/`
- E2E: `scripts/xhttp-e2e.sh`
