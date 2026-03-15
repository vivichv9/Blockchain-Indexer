# Nodes

## Что реализовано
- Добавлен runtime-реестр узлов в таблице `nodes_registry`.
- Основной RPC-узел из runtime-конфига автоматически синхронизируется в `nodes_registry` при старте backend.
- Фоновый `NodesRunner` периодически опрашивает все `enabled` узлы из `nodes_registry` и синхронизирует таблицу `node_health`.
- Для каждого узла сохраняются:
  - `status` (`ok` или `down`);
  - `tip_height`;
  - `tip_hash`;
  - `rpc_latency_ms`;
  - `last_seen_at`;
  - `details` с технической диагностикой последней проверки.
- Реализованы REST endpoint'ы:
  - `GET /v1/nodes`
  - `POST /v1/nodes`
  - `GET /v1/nodes/{node_id}/health`
- Узел можно добавить во время работы backend без перезапуска сервиса.
- Новый узел сразу появляется в `/v1/nodes`; до первой успешной проверки он имеет статус `unknown`.
- `GET /v1/jobs` теперь подставляет актуальный `tip_height` из последней успешной node health-проверки. Если успешной проверки еще не было, поле остается `null`.

## Где находится
- Логика node health: `src/modules/nodes/mod.rs`.
- HTTP API: `src/modules/api/mod.rs`.
- Инициализация и запуск runner: `src/app.rs`.

## Ограничения этапа
- Runtime-added узлы участвуют в node health monitoring, но не переключают основной RPC client индексатора/jobs/mempool.
- Статус `degraded`, предусмотренный схемой БД, пока не используется: runner фиксирует только `ok` и `down`.
- Дополнительные метрики и alerting для node health еще не добавлены.
