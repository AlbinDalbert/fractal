# Fractal Wishlist: Enabling Agentic Knowledge Workflows

To transform Fractal from a structural engine into an optimal knowledge backend for LLM-based agents, the following capabilities are requested:

## 1. Semantic Search Integration
*   **Goal**: Minimize token usage by retrieving contextually relevant snippets via embeddings rather than simple keyword matching.
*   **Requirement**: A `search` command extension that supports vector-based semantic retrieval (e.g., integrating with a local embedding model or a lightweight vector store).

## 2. Context-Optimized Extraction
*   **Goal**: Reduce the "noise" passed to the LLM by stripping non-essential HTML boilerplate and structural metadata.
*   **Requirement**: A command/flag (e.g., `fractal extract --clean <page_path>`) that returns only the textual content within the `<main>` element of a page, optimized for minimal token consumption.

## 3. Advanced Graph Querying
*   **Goal**: Allow agents to perform complex structural reasoning without having to manually traverse nodes and edges in memory.
*   **Requirement**: High-level CLI commands for specific graph patterns:
    *   `fractal graph neighbors <page_path> --depth N`
    *   `fractal graph identify-clusters --tags "rust, async"`
    *   `fractal graph find-orphan-links`

## 5. Intuitive CLI Ergonomics & Taxonomy

To ensure an agent (or human) can navigate and manipulate the knowledge base efficiently, the command structure should follow a hierarchical, resource-oriented taxonomy. This avoids "flat" command structures that become difficult to discover as features grow.

### Proposed Taxonomy Structure

*   **`fractal project <command>`**: Operations involving the project state or global settings.
    *   `init`, `validate`, `sync`, `import`
*   **`fractimit page <path> <command>`**: Operations targeting a specific HTML page's content and metadata.
    *   `meta set-summary`, `meta set-tags`, `note add`, `extract --clean`, `extract --raw`
*   **`fractal graph <type> <path> [args]`**: Structural operations investigating relationships.
    *   `graph backlinks`, `graph outlinks`, `graph neighbors`, `graph clusters`
*   **`fractal search <query>`**: The primary discovery interface.
    *   Supports keyword, semantic (new), and metadata-specific queries (e.g., `-t rust`).

### Ergonomic Principles for Agents
*   **Predictable Pathing**: Commands should always accept paths relative to the `pages/` directory or absolute paths from root, with a consistent behavior.
*   **Discoverability via Help**: Every sub-command must have a clear `--help` and its usage should be easily parsable by an LLM's tool-use logic.
*   **Atomic Output**: Commands should return structured text (e.g., simple strings or JSON) that is easy for an agent to parse without complex regex.*