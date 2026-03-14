# Testing

## Что реализовано
- Добавлены интеграционные тесты для lifecycle jobs API, nodes API, data API и indexer storage pipeline.
- Тесты поднимают PostgreSQL через `testcontainers`, применяют SQL-миграции и собирают актуальный `AppState` backend-монолита.
- Контейнер PostgreSQL удерживается на всём времени теста, чтобы наборы не зависели от времени жизни локальных переменных helper-функций.
- Проверяются сценарии:
  - список jobs;
  - `start`, `pause`, `resume`, `stop`;
  - требование Basic Auth;
  - ответ `404` для отсутствующего job;
  - ответ `409` при невалидном переходе состояния.
  - список nodes;
  - получение `node health` по `node_id`;
  - ответ `404` для отсутствующего node.
  - текущий и исторический balance по адресу;
  - список UTXO по адресу;
  - список confirmed transactions по фильтру адреса;
  - список mempool transactions;
  - список blocks по фильтрам `address` и `has_txid`;
  - ответ `422` при невалидной пагинации.
  - запись canonical blocks через `IndexerPipeline::persist_block`;
  - идемпотентность повторной записи блока;
  - ожидание предыдущей высоты при gap в chain;
  - mempool lookup по адресу через связи `inputs/outputs`.

## Где находится
- Интеграционные тесты: `tests/integration_jobs_api.rs`.
- Интеграционные тесты pipeline/storage: `tests/integration_indexer_pipeline.rs`.

## Требования к запуску
- Нужен доступный Docker daemon, потому что тесты используют `testcontainers`.
- Интеграционные тесты помечены `#[ignore]` и запускаются вручную:
  - `cargo test -- --ignored`
  - `cargo test --test integration_jobs_api -- --ignored`
- Если Docker недоступен, тесты завершаются без падения и печатают диагностическое сообщение.

## Ограничения текущего этапа
- Сейчас покрыты jobs API, nodes API, основные endpoint'ы data API и storage-часть indexer pipeline.
- Полные сценарии `MempoolRunner::sync_once`, `IndexerService::reconcile_chain` и live RPC-взаимодействие всё ещё не имеют собственного интеграционного покрытия.
- Для полной проверки нужен запуск `cargo check` и `cargo test`, но в текущей среде инструменты Rust CLI могут быть недоступны в `PATH`.
