# Config And Auth

## Что реализовано
- Поддержка полной YAML-конфигурации backend по структуре из ТЗ (`server`, `rpc`, `indexer`, `jobs`).
- Строгая валидация конфига при старте:
  - существование и читаемость TLS/mTLS файлов,
  - наличие паролей через `password_env`,
  - `indexer.reorg_depth >= 0`,
  - допустимые значения `indexer.network`,
  - уникальность `jobs[*].job_id`,
  - непустой `addresses` для `address_list`.
- Разрешение секретов из environment variables в runtime-конфиг.
- Обязательный Basic Auth middleware для API (на текущем этапе для всех маршрутов).
- Формат ошибки авторизации приведен к контракту API (`AUTH_FAILED`, HTTP 401).
- mTLS для RPC можно отключить через `rpc.mtls.enabled: false`.
- Для self-signed TLS на стороне RPC можно явно отключить проверку доверия через `rpc.insecure_skip_verify: true`.
- В публичном шаблоне репозитория `config/indexer.yaml` содержит только примерные значения, поэтому перед запуском обязательно нужно заменить `rpc.url` и `rpc.auth.basic.username` на параметры реального Bitcoin JSON-RPC endpoint.

## Что нужно заполнить перед первым запуском

- В `.env`:
  - `INDEXER_API_USERNAME`
  - `INDEXER_API_PASSWORD`
  - `BITCOIN_RPC_PASSWORD`
- В `config/indexer.yaml`:
  - `rpc.url`
  - `rpc.auth.basic.username`
  - `indexer.network`
  - `jobs`
- При использовании self-signed сертификата у RPC:
  - включить `rpc.insecure_skip_verify: true`
- При использовании mTLS:
  - положить сертификаты в `certs/mtls/`

## Где находится
- Загрузка и валидация конфига: `src/modules/config/mod.rs`.
- Basic Auth в API: `src/modules/api/mod.rs`.
- Подключение конфига в bootstrap: `src/app.rs`.

## Ограничения этапа
- HTTPS в Rust-сервисе и mTLS RPC-клиент будут реализованы следующими шагами.
- Endpoint `/metrics` и переключение его auth-режима пока не добавлены.
