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
- REST API для управления jobs по ТЗ:
  - `GET /v1/jobs`
  - `GET /v1/jobs/{job_id}`
  - `POST /v1/jobs/{job_id}/start`
  - `POST /v1/jobs/{job_id}/stop`
  - `POST /v1/jobs/{job_id}/pause`
  - `POST /v1/jobs/{job_id}/resume`
  - `POST /v1/jobs/{job_id}/retry`

## Где находится
- Бизнес-логика jobs: `src/modules/jobs/mod.rs`.
- API jobs: `src/modules/api/mod.rs`.
- Инициализация и синхронизация при старте: `src/app.rs`.

## Ограничения этапа
- Поле `tip_height` в API возвращается как `null` до реализации node health.
- Поле `progress_height` хранится, но пока не обновляется индексатором.
