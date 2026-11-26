# Участие в разработке

## Ветки

В репозитории две долгоживущие ветки:

| Ветка | Назначение | Что можно |
|---|---|---|
| `main` | релизы | только слияние из `dev` при выпуске, теги, `CHANGELOG.md`; прямые коммиты запрещены |
| `dev` | разработка | вся работа: коммиты, PR, эксперименты |

Порядок:

1. Ветка от `dev`: `<область>/<короткое-имя>`, например `ui/openvpn-form`,
   `core/geo-cache`, `v2rayxhttp/packet-up`.
2. Коммиты в неё, PR обратно в `dev`.
3. Релиз — по команде мейнтейнера: `dev` сливается в `main`, поднимается версия,
   обновляется `CHANGELOG.md`, ставится тег. Дальше `dev` продолжает жить и
   уходит вперёд на следующие изменения.

Синоним `main` для пользователей — «последний стабильный релиз». Незаконченные
фичи в `main` не попадают.

## Коммиты

- Один коммит — одна логическая правка.
- Заголовок на английском, повелительное наклонение, до ~72 символов:
  `core: fix Clash YAML port parsing`, `ui: add XHTTP transport fields`.
- Префиксы: `core:`, `desktop:`, `ui:`, `engine:`, `docs:`, `ci:`, `build:`,
  `deps:`; для патчей форка — подсистема sing-box (`v2rayxhttp:`,
  `tls/reality:`).
- Тело — если не очевидно «зачем»: причина и последствия, а не пересказ diff.
- Без `WIP`, `fixes`, `update` без объекта и без сгенерированных подписей.

## Перед отправкой

Проверки те же, что в CI:

```bash
cargo fmt --all -- --check
cargo test -p protogrid-core
cargo check -p protogrid-desktop
```

Быстрый вариант — `.opencode/skills/git/scripts/precommit.sh`.

ProtoGrid-core (форк sing-box) проверяется отдельно: CI применяет `docs/patches/*.patch` к
`SagerNet/sing-box` v1.14.4 и запускает `go test` и `go build`. Если меняешь
форк — обновляй и патчи, и зависящий от них Rust-код одним PR.

## Что не коммитить

`target/`, `dist/`, `forks/sing-box`, скачанные ядра, `wintun.dll`, кэши
GeoIP/GeoSite, пользовательские `config.json`/`runtime/`/`cores/`/`rulesets/` и
логи. Они перечислены в `.gitignore`.

Перед PR проверь diff на секреты: `vless://` и другие ссылки с UUID,
`password`, `token`, `api_key`, приватные ключи.

## Лицензия

Отправляя код, вы соглашаетесь, что он распространяется по MIT (см.
[LICENSE](LICENSE)). Сторонние компоненты — в
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Патчи к sing-box — производная
от GPLv3-кода, они остаются под GPLv3.
