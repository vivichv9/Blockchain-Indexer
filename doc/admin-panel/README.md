# Admin Panel

## Что реализовано
- Добавлен отдельный React/Vite модуль админ-панели в `admin-panel/`.
- Панель использует только REST API backend и не обращается к БД напрямую.
- Для docker-compose добавлен production-like контейнер admin panel: Vite собирается в статику, `nginx` отдаёт UI и проксирует `/api` в backend.
- Реализованы две страницы из ТЗ:
  - `Jobs`
  - `Node Health`
- На странице `Jobs` доступны:
  - таблица job'ов;
  - форма создания нового job без перезапуска backend;
  - просмотр деталей выбранного job;
  - действия `start`, `stop`, `pause`, `resume`, `retry`;
  - показ `last_error` и `config_snapshot`.
- На странице `Node Health` доступны:
  - форма добавления нового узла без перезапуска backend;
  - список узлов;
  - расширенная карточка выбранного узла;
  - отображение `status`, `tip_height`, `tip_hash`, `rpc_latency_ms`, `last_seen_at`, `details`.

## Конфигурация
- Базовый URL backend и Basic Auth передаются только через env:
  - `VITE_INDEXER_API_BASE_URL`
  - `VITE_INDEXER_API_USERNAME`
  - `VITE_INDEXER_API_PASSWORD`
- Пример переменных вынесен в `admin-panel/.env.example`.
- В docker-compose UI собирается с `VITE_INDEXER_API_BASE_URL=/api`, поэтому браузер ходит в backend через nginx proxy без отдельной настройки CORS.

## Docker Compose
- После `docker compose up --build` админ-панель доступна на `http://127.0.0.1:4173`.
- Запросы `/api/*` проксируются в backend (`http://backend:8080/*`) внутри docker-сети.

## Где находится
- Исходники панели: `admin-panel/src/`.
- Клиент API: `admin-panel/src/api.ts`.
- Основной UI: `admin-panel/src/App.tsx`.

## Ограничения этапа
- Поскольку backend пока не предоставляет endpoint для логов, в панели отображается `last_error`, а не поток логов.
- Локальная сборка панели в текущем окружении не проверялась, потому что `node` и `npm` отсутствуют в `PATH`.
