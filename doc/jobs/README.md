# Jobs

## Что реализовано
- Хранение jobs в PostgreSQL через таблицу `jobs`.
- Синхронизация jobs из YAML-конфига при старте backend (upsert по `job_id`).
- Создание jobs во время работы backend через `POST /v1/jobs` без перезапуска сервиса.
- После синхронизации backend автоматически восстанавливает jobs с `enabled: true`:
  - `created -> running`
  - `paused -> running`
  - `failed -> running`
- Бизнес-логика переходов состояний jobs:
  - `start`: `created -> running`
  - `stop`: `running|paused|failed -> created`
  - `pause`: `running -> paused`
  - `resume`: `paused -> running`
  - `retry`: `failed -> running`
- Добавлены unit-тесты для валидации переходов состояний.
- REST API для управления jobs по ТЗ:
  - `GET /v1/jobs`
  - `POST /v1/jobs`
  - `GET /v1/jobs/{job_id}`
  - `POST /v1/jobs/{job_id}/start`
  - `POST /v1/jobs/{job_id}/stop`
  - `POST /v1/jobs/{job_id}/pause`
  - `POST /v1/jobs/{job_id}/resume`
  - `POST /v1/jobs/{job_id}/retry`
- Runtime-created job с `enabled: true` сразу переводится в `running`.
- Runtime-created job с `enabled: false` создается в статусе `created`.
- Для `address_list` runtime create требует непустой `addresses`.
- Для `all_addresses` runtime create требует пустой `addresses`.
- Добавлен фоновый `JobsRunner`, который:
  - периодически читает jobs со статусом `running`,
  - ограничивает количество одновременно исполняемых jobs через `indexer.concurrency.max_jobs`,
  - перед обработкой батча проверяет расхождение canonical-цепочки в окне `reorg_depth`,
  - корректно начинает индексирование с genesis-высоты, если в БД ещё нет canonical block `0`,
  - для каждого job индексирует батч высот до `indexer.batching.blocks_per_batch`,
  - при пересечении jobs по одним и тем же данным не пишет соседние высоты вне порядка canonical-цепочки,
  - обновляет `progress_height` после каждого успешно записанного блока,
  - переводит job в `failed` при ошибке индексации/RPC и пишет текст ошибки в `last_error`.

## Где находится
- Бизнес-логика jobs: `src/modules/jobs/mod.rs`.
- API jobs: `src/modules/api/mod.rs`.
- Инициализация, синхронизация и запуск runner при старте: `src/app.rs`.

## Ограничения этапа
- Поле `tip_height` в API заполняется из последней успешной записи в `node_health`; если успешной проверки еще не было, оно возвращается как `null`.
- Jobs обрабатывают только confirmed/canonical индексацию; mempool синхронизируется отдельным runner.
- Для `address_list` пока не добавлена специализированная стратегия выборки адресов: используется общий pipeline индексации.
- Если нужная предыдущая высота еще не зафиксирована другим worker, job просто ждет следующую итерацию runner без продвижения `progress_height`.
