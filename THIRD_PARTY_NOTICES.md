# Third-party notices

ProtoGrid для desktop и общее Rust-ядро распространяются по MIT (см.
[LICENSE](LICENSE)). Android-клиент линкует libbox и распространяется по
GPL-3.0 (см. `apps/protogrid-android/LICENSE`). В репозитории и в готовых
сборках используются сторонние компоненты, у каждого — своя лицензия.

## sing-box (основа ProtoGrid-core)

- Upstream: <https://github.com/SagerNet/sing-box>, тег `v1.14.4`.
- Лицензия upstream: **GNU GPL v3** (`GPL-3.0-or-later`), файл `LICENSE` в
  upstream-репозитории.
- Продуктовое имя ядра — **ProtoGrid-core**. Все изменения форка хранятся в
  этом репозитории в виде патчей
  `docs/patches/*.patch`; `forks/sing-box` в git не входит.
- Соответствующий исходный код форка = upstream `v1.14.4` + патчи из
  `docs/patches`, применённые по порядку. Сборка: `scripts/build-fork.sh`
  (теги `with_quic,with_utls,with_clash_api,with_gvisor,with_openvpn`).
- Если вы распространяете собранный бинарник форка, вы обязаны выполнить
  требования GPLv3: предоставить этот исходный код и текст лицензии. Текст
  лицензии — `installer/licenses/GPL-3.0.txt`, он входит в Windows-пакеты.

## libbox (Android)

- `apps/protogrid-android` линкует `libbox.aar`, собранный из того же форка
  sing-box (`make lib_install` + `build_libbox`), поэтому APK распространяется
  под **GPL-3.0**. Текст лицензии — `apps/protogrid-android/LICENSE`.
- Исходники Android-оболочки и JNI-моста (`crates/protogrid-mobile`) входят в
  этот репозиторий; исходники libbox — upstream `v1.14.4` + `docs/patches`.

## ZXing (Android QR)

- `com.journeyapps:zxing-android-embedded` и `com.google.zxing:core` —
  **Apache License 2.0**, <https://github.com/journeyapps/zxing-android-embedded>.
- Используются только в Android-клиенте: сканирование QR камерой и разбор QR
  с изображения. На десктопе QR разбирает Rust-библиотека `rqrr`.

## wintun

- `wintun.dll` не входит в репозиторий и берётся из официального архива
  <https://www.wintun.net/builds/wintun-0.14.1.zip> (0.14.1, amd64).
- Исходный код wintun распространяется по **GPLv2** (WireGuard LLC),
  <https://git.zx2c4.com/wintun>. Текст лицензии —
  `installer/licenses/wintun-GPL-2.0.txt`.
- Готовые бинарники из официального архива распространяются по лицензии,
  вложенной в сам архив. Готовые пакеты ProtoGrid кладут текст лицензии рядом
  с `wintun.dll`.

## Шрифты

| Шрифт | Лицензия | Файл |
|---|---|---|
| Inter | SIL Open Font License 1.1 | `apps/protogrid-desktop/assets/Inter-LICENSE.txt` |
| JetBrains Mono | SIL Open Font License 1.1 | `apps/protogrid-desktop/assets/JetBrainsMono-OFL.txt` |
| DejaVu Sans Mono | Bitstream Vera / DejaVu | `apps/protogrid-desktop/assets/DejaVu-LICENSE.txt` |

## Зависимости Rust

Приложение собрано из crates.io-зависимостей, каждая сохраняет свою лицензию.
Полный список с версиями и лицензиями — [docs/third-party-rust.md](docs/third-party-rust.md),
он генерируется из `Cargo.lock` командой:

```bash
python3 scripts/third-party.py
```
