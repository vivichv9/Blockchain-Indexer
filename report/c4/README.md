# C4 architecture diagrams (PlantUML)

Source files:
- `c4-component-architecture.puml`
- `c4-deployment-architecture.puml`

## Build

Run from `report/c4`:

```bash
plantuml c4-component-architecture.puml
plantuml c4-deployment-architecture.puml
```

Formats:

```bash
plantuml -tsvg c4-component-architecture.puml
plantuml -tsvg c4-deployment-architecture.puml
plantuml -tpdf c4-component-architecture.puml
plantuml -tpdf c4-deployment-architecture.puml
```

Note:
- The diagram uses C4-PlantUML via `!includeurl`.
- Internet access is required at compile time, or pre-download C4-PlantUML files locally and replace `!includeurl` with `!include`.
