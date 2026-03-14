# DFD diagrams (PlantUML)

Files:
- `dfd-level-0-context.puml` - DFD 0 level (context)
- `dfd-level-1-decomposition.puml` - DFD 1 level
- `dfd-level-2-indexing.puml` - DFD 2 level, indexing process
- `dfd-level-2-admin.puml` - DFD 2 level, jobs administration process
- `dfd-level-2-data-serving.puml` - DFD 2 level, data serving process with fallback to node
- `dfd-notation.puml` - shared notation/style definitions

Compile example:

```bash
plantuml dfd-level-0-context.puml
plantuml dfd-level-1-decomposition.puml
plantuml dfd-level-2-indexing.puml
plantuml dfd-level-2-admin.puml
plantuml dfd-level-2-data-serving.puml
```
