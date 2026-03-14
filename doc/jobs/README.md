# Jobs

## Что реализовано
- Хранение jobs в PostgreSQL через таблицу `jobs`.
- Синхронизация jobs из YAML-конфига при старте backend (upsert по `job_id`).
- Бизнес-логика переходов состояний jobs:
  - `start`: `created -> running`
  - `stop`: `running|paused|failed -> created`
  - `pause`: `running -> paused`
  - `resume`: `paused -> running`
  - `retry`: `failed -> running`
- Добавлены unit-тесты для валидации переходов состояний.
- REST API для управления jobs по ТЗ:
  - `GET /v1/jobs`
  - `GET /v1/jobs/{job_id}`
  - `POST /v1/jobs/{job_id}/start`
  - `POST /v1/jobs/{job_id}/stop`
  - `POST /v1/jobs/{job_id}/pause`
  - `POST /v1/jobs/{job_id}/resume`
  - `POST /v1/jobs/{job_id}/retry`
- Добавлен фоновый `JobsRunner`, который:
  - периодически читает jobs со статусом `running`,
  - ограничивает количество одновременно исполняемых jobs через `indexer.concurrency.max_jobs`,
  - для каждого job индексирует батч высот до `indexer.batching.blocks_per_batch`,
  - обновляет `progress_height` после каждого успешно записанного блока,
  - переводит job в `failed` при ошибке индексации/RPC и пишет текст ошибки в `last_error`.

## Где находится
- Бизнес-логика jobs: `src/modules/jobs/mod.rs`.
- API jobs: `src/modules/api/mod.rs`.
- Инициализация, синхронизация и запуск runner при старте: `src/app.rs`.

## Ограничения этапа
- Поле `tip_height` в API возвращается как `null` до реализации node health.
- Jobs пока обрабатываются последовательно по высотам без reorg/mempool-логики.
- Для `address_list` пока не добавлена специализированная стратегия выборки адресов: используется общий pipeline индексации.
