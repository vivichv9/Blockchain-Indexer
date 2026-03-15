# API Docs

## Что доступно

Backend публикует OpenAPI-документацию и интерактивный Swagger UI прямо из Axum-приложения.

Доступные endpoints:

- `GET /openapi.json` возвращает OpenAPI-документ в формате JSON
- `GET /docs` открывает интерактивный Swagger UI

## Авторизация

Endpoints документации используют тот же Basic Auth middleware, что и остальной API.

Пример:

```powershell
curl -u admin:admin http://127.0.0.1:8080/openapi.json
```

Открыть в браузере:

- `http://127.0.0.1:8080/docs`

## Что покрыто документацией

Сгенерированная документация включает:

- системные endpoints: `health`, `metrics`
- jobs API
- nodes API
- data API

## Примечания

- документация описывает текущий HTTP-интерфейс Axum
- endpoint `metrics` описан как `text/plain`
- Swagger UI отдается самим backend, отдельный контейнер для документации не нужен
