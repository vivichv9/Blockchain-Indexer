# Docker LaTeX Builder

Отдельный контейнер для сборки ВКР-документации из папки `diploma`.

## Что делает
- собирает независимый Docker-образ только для LaTeX;
- не использует основной `Dockerfile` проекта;
- позволяет собрать `diploma/main.tex` внутри контейнера.

## Команды

Сборка образа:

```powershell
docker build -t bitcoin-indexer-diploma-latex .\diploma-docker
```

Компиляция документа:

```powershell
docker run --rm `
  -v ${PWD}:/work `
  -w /work/diploma `
  bitcoin-indexer-diploma-latex `
  latexmk -xelatex -interaction=nonstopmode -halt-on-error main.tex
```
