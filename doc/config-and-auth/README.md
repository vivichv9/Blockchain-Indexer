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

## Где находится
- Загрузка и валидация конфига: `src/modules/config/mod.rs`.
- Basic Auth в API: `src/modules/api/mod.rs`.
- Подключение конфига в bootstrap: `src/app.rs`.

## Ограничения этапа
- HTTPS в Rust-сервисе и mTLS RPC-клиент будут реализованы следующими шагами.
- Endpoint `/metrics` и переключение его auth-режима пока не добавлены.
