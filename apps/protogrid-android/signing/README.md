# Android signing lineage

`lineage.bin` — публичная цепочка ротации подписи Android:

1. `CN=Android Debug, O=Android, C=US`
   SHA-256 `e1070ced67a42bb0d0786d39b78d5e5ea298d6f919a89d56c01c5cfc593483c3`
   — старый ключ, которым подписаны опубликованные APK 0.3–0.4.1.
2. `CN=ProtoGrid, OU=Android Release, O=ProtoGrid, C=RU`
   SHA-256 `e2f8fb5f1ea6aee448d2c053ad413fff68dade790d0962b258b3c1bb3f9a9b78`
   — новый release-ключ.

Каждый релизный APK подписывается так:

- v1/v2 — старым ключом (его видят Android 8.x и ниже);
- v3 — новым ключом с этой lineage (Android 9+ видит новый сертификат).

Без старого ключа обновление поверх уже установленных 0.3–0.4.1 перестанет
устанавливаться. Не удаляйте `lineage.bin` и не меняйте порядок сертификатов.

Команда подписи (CI делает это автоматически после сборки):

```bash
apksigner sign \
  --ks legacy.keystore \
  --next-signer --ks release.keystore \
  --lineage apps/protogrid-android/signing/lineage.bin \
  --rotation-min-sdk-version 28 \
  --out app-rotated.apk app-release.apk
```

Секреты CI: `PROTOGRID_KEYSTORE_*` — новый ключ,
`PROTOGRID_LEGACY_KEYSTORE_*` — старый ключ.
