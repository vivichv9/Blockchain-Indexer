# BPMN diagram (PlantUML)

Source file:
- `bpmn-indexer-process.puml`
- `bpmn-data-serving-process.puml`

## Build diagram

Run from `report/bpmn`:

```bash
plantuml bpmn-indexer-process.puml
plantuml bpmn-data-serving-process.puml
```

This generates separate PNG files for each process.

## Other formats

```bash
plantuml -tsvg bpmn-indexer-process.puml
plantuml -tsvg bpmn-data-serving-process.puml
plantuml -tpdf bpmn-indexer-process.puml
plantuml -tpdf bpmn-data-serving-process.puml
```

Generated files can be inserted into the appendix and referenced from Chapter 4.
