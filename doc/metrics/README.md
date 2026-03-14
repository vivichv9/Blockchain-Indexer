# Metrics

## Что реализовано
- Добавлен Prometheus endpoint `GET /metrics`.
- Endpoint проходит через тот же Basic Auth middleware, что и остальной API, поэтому `/metrics` по умолчанию защищен.
- Реализован in-memory реестр метрик с экспортом в формате Prometheus text exposition.
- На `/metrics` публикуются метрики из ТЗ:
  - `indexer_tip_height`
  - `indexer_progress_height{job_id=...}`
  - `indexer_lag_blocks{job_id=...}`
  - `indexer_blocks_processed_total{job_id=...}`
  - `indexer_txs_processed_total{job_id=...}`
  - `indexer_rpc_requests_total{method=...}`
  - `indexer_rpc_request_duration_seconds{method=...}`
  - `indexer_db_write_duration_seconds{table=...}`
  - `indexer_errors_total{type=...}`

## Как считается
- `indexer_tip_height`, `indexer_progress_height` и `indexer_lag_blocks` вычисляются на момент scrape из PostgreSQL, поэтому отражают фактическое состояние БД.
- RPC counters и histogram обновляются внутри `RpcClient`.
- Метрики обработанных блоков и транзакций обновляются из `JobsRunner` только для новых canonical-блоков.
- DB write histogram обновляется на ключевых путях записи в `indexer` и `node_health`.
- `indexer_errors_total` инкрементируется для RPC, reorg, job batch, node health и DB write ошибок.

## Где находится
- Реестр и рендер Prometheus: `src/modules/metrics/mod.rs`.
- HTTP endpoint: `src/modules/api/mod.rs`.
- Интеграция с `rpc`, `jobs`, `indexer`, `nodes`: соответствующие модули в `src/modules`.

## Ограничения этапа
- `/metrics` пока всегда использует тот же auth-режим, что и основной API; отдельного конфига для отключения auth еще нет.
- Histogram по DB write покрывает основные точки записи, но не каждый SQL path проекта.
