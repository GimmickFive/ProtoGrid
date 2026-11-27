# Bundled GeoIP/GeoSite seed

Эти `.srs`-файлы служат стартовым кэшем, если профиль ещё не скачал базы.
Android компилирует их в `libprotogrid_mobile.so`; desktop-пакеты кладут их
рядом с `ProtoGrid.exe` (в macOS — в `Contents/Resources/rulesets`). Они **не
заменяют обновления**: `ruleset::prepare` раз в 7 дней скачивает свежие версии
с тех же URL и сохраняет старую/зашитую копию, если GitHub недоступен.

| Файл | Источник |
|---|---|
| `geoip-ru.srs` | <https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs> |
| `geosite-category-ru.srs` | <https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-category-ru.srs> |

Лицензия обоих наборов — **GNU GPL v3** (SagerNet). Подробности — в
`THIRD_PARTY_NOTICES.md`.

Обновить слепок вручную:

```bash
curl -fsSL -o geoip-ru.srs \
  https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs
curl -fsSL -o geosite-category-ru.srs \
  https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-category-ru.srs
```
