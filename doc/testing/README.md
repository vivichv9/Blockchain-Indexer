# Testing

## Что реализовано
- Добавлены интеграционные тесты для lifecycle jobs API, nodes API, data API, indexer storage pipeline и runtime runner-сценариев.
- Тесты поднимают PostgreSQL через `testcontainers`, применяют SQL-миграции и собирают актуальный `AppState` backend-монолита.
- Контейнер PostgreSQL удерживается на всём времени теста, чтобы наборы не зависели от времени жизни локальных переменных helper-функций.
- Метод измерения покрытия зафиксирован через `cargo llvm-cov` и обёртку `scripts/coverage.ps1`.
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
  - `MempoolRunner::sync_once` для новых и dropped mempool transactions через mock JSON-RPC;
  - `IndexerService::reconcile_chain` с пометкой orphaned-данных и пересборкой агрегатов после reorg.

## Где находится
- Интеграционные тесты: `tests/integration_jobs_api.rs`.
- Интеграционные тесты pipeline/storage: `tests/integration_indexer_pipeline.rs`.
- Интеграционные тесты runtime runner-сценариев: `tests/integration_runtime_runners.rs`.

## Требования к запуску
- Нужен доступный Docker daemon, потому что тесты используют `testcontainers`.
- Для coverage-отчёта нужен установленный `cargo-llvm-cov`.
- Интеграционные тесты помечены `#[ignore]` и запускаются вручную:
  - `cargo test -- --ignored`
  - `cargo test --test integration_jobs_api -- --ignored`
- Coverage-отчёт запускается через PowerShell:
  - `powershell -ExecutionPolicy Bypass -File scripts/coverage.ps1`
- Скрипт coverage формирует:
  - `report/coverage/index.html`
  - `report/coverage/summary.txt`
  - `coverage.json`
  - `lcov.info`
- Если Docker недоступен, тесты завершаются без падения и печатают диагностическое сообщение.

## Ограничения текущего этапа
- Сейчас покрыты jobs API, nodes API, основные endpoint'ы data API, storage-часть indexer pipeline и ключевые runtime runner-сценарии.
- Пока используется mock JSON-RPC server, а не реальный Bitcoin Core/regtest, поэтому end-to-end цепочка с настоящим узлом ещё не проверяется.
- Для полной проверки нужен запуск `cargo check`, `cargo test` и фактического `cargo llvm-cov`, но в текущей среде инструменты Rust CLI могут быть недоступны в `PATH` или не иметь настроенного default toolchain.
