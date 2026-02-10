# Architecture Diagrams

This directory contains interactive Mermaid diagrams that visualize the Kage system architecture.

## Available Diagrams

1. **system-architecture.md** - High-level system architecture showing entry points, core components, processing pipeline, and storage layer
2. **data-flow.md** - Sequence diagram showing the complete data flow from initialization through block processing to queries
3. **component-interaction.md** - Component interaction diagram showing relationships between modules
4. **database-structure.md** - Entity-relationship diagram of the RocksDB schema with all 13 column families
5. **processing-pipeline.md** - Flowchart showing the complete block processing pipeline
6. **query-flow.md** - Sequence diagram showing how balance/delta queries work with fill-forward algorithm

## Viewing the Diagrams

These diagrams use Mermaid syntax and can be viewed:

- **GitHub**: Automatically rendered when viewing the `.md` files
- **VS Code**: Install the "Markdown Preview Mermaid Support" extension
- **Online**: Copy the Mermaid code to [mermaid.live](https://mermaid.live)
- **Other editors**: Many markdown previewers support Mermaid natively

## ASCII Art Versions

For compatibility with all markdown previewers, ASCII art versions of these diagrams are included in the main [README.md](../../README.md).
