# Acceptance

## Цель
- Зафиксировать единый checklist приёмки MVP.
- Привязать критерии приёмки к уже реализованным модулям и тестовым артефактам.

## Как использовать
- Перед приёмкой пройти пункты ниже сверху вниз.
- Для каждого пункта фиксировать статус:
  - `done` если требование подтверждено кодом и проверкой;
  - `partial` если реализация есть, но нет полной верификации;
  - `todo` если требование ещё не закрыто.
- Итоговая приёмка MVP возможна только когда все MUST-пункты имеют статус `done`.

## MVP Checklist
- `partial` Docker-окружение поднимает PostgreSQL и backend через `docker-compose.yml`.
- `done` Backend применяет SQL-миграции при старте через `Storage::apply_migrations`.
- `partial` Конфиг загружается из YAML и валидируется по обязательным полям, env и путям файлов.
- `partial` Basic Auth включён для API endpoint'ов backend.
- `todo` HTTPS в backend не доведён до подтверждённой рабочей конфигурации.
- `partial` RPC-клиент поддерживает Basic Auth, mTLS и таймауты.
- `done` Схема PostgreSQL покрывает blocks, transactions, tx_inputs, tx_outputs, utxos, balances, jobs и node_health.
- `done` Jobs поддерживают `created`, `running`, `paused`, `failed`, `completed` и API-операции `start/stop/pause/resume/retry`.
- `done` Backend сохраняет `config_snapshot` jobs и прогресс индексации.
- `done` Indexer pipeline сохраняет canonical blocks, transactions, inputs, outputs, UTXO и address balances.
- `done` Идемпотентность записи блока и ожидание предыдущей высоты покрыты интеграционными тестами.
- `done` Reorg-пересборка помечает orphaned данные и пересчитывает агрегаты.
- `done` Mempool хранится отдельно от confirmed chain и поддерживает статус `dropped`.
- `done` REST API для jobs, nodes и data endpoints реализован.
- `done` Admin panel работает через REST API и покрывает Jobs и Node Health.
- `done` CLI на Python поддерживает jobs, nodes и demo-команды для data API.
- `done` JSON-логирование реализовано через `tracing`.
- `done` Prometheus metrics endpoint и базовые метрики indexer реализованы.
- `partial` Интеграционные тесты покрывают API, pipeline, mempool и reorg через PostgreSQL + mock RPC.
- `todo` End-to-end проверка с реальным Bitcoin Core/regtest ещё не выполнена.
- `partial` Метод измерения покрытия зафиксирован через `scripts/coverage.ps1` и `cargo llvm-cov`.
- `todo` Целевое покрытие backend >= 80% ещё не подтверждено фактическим отчётом.

## Опорные артефакты
- Backend bootstrap: `src/app.rs`
- API: `src/modules/api/mod.rs`
- Jobs: `src/modules/jobs/mod.rs`
- Indexer и reorg: `src/modules/indexer/mod.rs`
- Mempool: `src/modules/mempool/mod.rs`
- Metrics: `src/modules/metrics/mod.rs`
- CLI: `cli/indexer_cli.py`
- Admin panel: `admin-panel/src/App.tsx`
- Интеграционные тесты API: `tests/integration_jobs_api.rs`
- Интеграционные тесты pipeline: `tests/integration_indexer_pipeline.rs`
- Интеграционные тесты runtime runner-сценариев: `tests/integration_runtime_runners.rs`

## Что осталось до полного `done`
- Подтвердить рабочий HTTPS-контур backend.
- Прогнать end-to-end сценарий с реальным Bitcoin Core/regtest.
- Снять фактический отчёт покрытия и проверить достижение порога >= 80%.
