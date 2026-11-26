# Third-party notices

ProtoGrid распространяется по MIT (см. [LICENSE](LICENSE)). В репозитории и в
готовых сборках используются сторонние компоненты, у каждого — своя лицензия.

## sing-box (форк ProtoGrid)

- Upstream: <https://github.com/SagerNet/sing-box>, тег `v1.14.4`.
- Лицензия upstream: **GNU GPL v3** (`GPL-3.0-or-later`), файл `LICENSE` в
  upstream-репозитории.
- Все изменения форка хранятся в этом репозитории в виде патчей
  `docs/patches/*.patch`; `forks/sing-box` в git не входит.
- Соответствующий исходный код форка = upstream `v1.14.4` + патчи из
  `docs/patches`, применённые по порядку. Сборка: `scripts/build-fork.sh`
  (теги `with_quic,with_utls,with_clash_api,with_gvisor,with_openvpn`).
- Если вы распространяете собранный бинарник форка, вы обязаны выполнить
  требования GPLv3: предоставить этот исходный код и текст лицензии.

## wintun

- `wintun.dll` не входит в репозиторий и скачивается отдельно с
  <https://www.wintun.net/>.
- Исходники wintun — GPLv2 (WireGuard LLC); готовые бинарники из официального
  zip распространяются по лицензии, вложенной в архив. При перепаковке
  `wintun.dll` переносите этот файл лицензии в пакет.

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
