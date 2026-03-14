# Nodes

## Что реализовано
- Фоновый `NodeHealthRunner`, который периодически опрашивает Bitcoin RPC-узел и синхронизирует таблицу `node_health`.
- Для активного `node_id` сохраняются:
  - `status` (`ok` или `down`);
  - `tip_height`;
  - `tip_hash`;
  - `rpc_latency_ms`;
  - `last_seen_at`;
  - `details` с технической диагностикой последней проверки.
- Реализованы REST endpoint'ы:
  - `GET /v1/nodes`
  - `GET /v1/nodes/{node_id}/health`
- `GET /v1/jobs` теперь подставляет актуальный `tip_height` из последней успешной node health-проверки. Если успешной проверки еще не было, поле остается `null`.

## Где находится
- Логика node health: `src/modules/nodes/mod.rs`.
- HTTP API: `src/modules/api/mod.rs`.
- Инициализация и запуск runner: `src/app.rs`.

## Ограничения этапа
- Сейчас мониторится один RPC-узел из текущего runtime-конфига `rpc.node_id`.
- Статус `degraded`, предусмотренный схемой БД, пока не используется: runner фиксирует только `ok` и `down`.
- Дополнительные метрики и alerting для node health еще не добавлены.
