---

## Gemma4 26b

---

# Fractal CLI Design Wishlist

This document outlines the preferred taxonomy and ergonomic design for the Fractal CLI, optimized for both human usability and seamless orchestration by LLM agents.

## Core Philosophy
- **Resource-Oriented Hierarchy**: Use a `[resource] [action]` pattern (e.g., `page inspect`). This makes capabilities discoverable via logical grouping.
- **Noun + Verb Syntax**: Avoid "creative" command names. Stick to standard CRUD-inspired verbs: `create`, `inspect`, `mutate`, `delete`, `list`, `search`.
 
## Proposed Command Hierarchy

### 1. `project` (Lifecycle & Integrity)
*Focus: Managing the project boundary and adherence to the Fractal contract.*
- `init`: Initialize a new Fractal project.
- `validate`: Check if the current project follows all structural and metadata rules.
- `repair`: Attempt to fix broken links or malformed metadata where safe.
- `status`: High-level overview of project health, file count, and recent changes.

### 2. `page` (The Atomic Unit)
*Focus: Manipulation of individual documents.*
- `create`: Generate a new page with predefined templates/metadata.
- `inspect`: Read metadata, notes, and structure of a specific page.
- `mutate`: Modify specific parts of a page (e.g., change title, add tag) without touching user content.
- `rename`: Rename a page while automatically updating all incoming links in the project.
- `delete`: Remove a page and report on orphaned relationships or broken backlinks.

### 3. `graph` (Structure & Relationships)
*Focus: Navigating the web of connections.*
- `index`: Trigger/refresh the generation of project-wide indexes and relationship maps.
- `links`: View outgoing links, incoming backlinks, or notes for a specific node.
- `query`: Find subsets of the graph (e.g., "find all orphaned pages", "find pages connected to Page X").

### 4. `search` (Retrieval)
*Focus: Efficiently finding information within the project.*
- `text`: Full-text search across page content.
- `metadata`: Search by tags, titles, or custom metadata keys.
- `semantic`: (Future) Perform vector/embedding-based similarity searches.

### 5. `io` (Boundary Operations)
*Focus: Interfacing with the outside world.*
- `import`: Convert Markdown, HTML, or other formats into Fractal pages.
- `export`: Transform Fractal projects into static HTML sites or other readable formats.

## Agent-First Ergonomics (The "LLM UX")

To minimize the cognitive load and token usage for LLMs, the CLI should support:

### 1. The "Mutation Report" Pattern
Every command that modifies state MUST return a structured summary of changes.
*Example Output:* `[SUCCESS] page:mutate src/doc.html -> Updated 'tags' (added 'research'); updated 2 backlinks.`
*Why:* This allows the agent to verify its work without re-running a full `project validate`.

### 2. Identity Dualism
Commands should accept both **Stable IDs** (for graph traversing) and **File Paths** (for filesystem exploration).
*Why:* Agents often discover files via `ls` but need stable identifiers to perform complex mutations.

### 3. Machine-Readable Standard (`--json`)
Every command must support a `--json` flag that returns the full, parsed object/state.
*Why:* Eliminates fragile regex parsing of human-readable text by the LLM.

### 4. Capability Discovery via Schema
The CLI should be able to export its own command structure and metadata requirements as a schema (e.g., `fractal capabilities --json`).
*Why:* Allows an agent to "learn" the API surface on the fly without reading extensive documentation.

### 5. Context-Specific "Snippet" Extraction
A command like `page context --for <id> --depth 1` that returns a minimized, LLM-friendly representation of a page and its immediate neighbors.
*Why:* Reduces token consumption by preventing the agent from needing to read entire large files just to understand local relationships.

### 6. Dry Run Mode (`--dry-run`)
A mode that simulates a command and shows what *would* happen without altering the filesystem.
*Why:* Safe exploration for agents performing complex, multi-step refactors.

## Summary of Design Goals
| Feature | Benefit to Human | Benefit to LLM (Agent) |
| :--- de/template | Easy to remember commands | Predictable discovery & automation |
| **JSON Output** | Harder to read manually | Essential for reliable parsing |
| **Mutation Reports** | Confirms visibility of changes | Provides immediate verification loop |
| **Context Snippets** | High-level overview | Drastic reduction in token/cost overhead |

---

## Gemma4 12b

---

# LLM & CLI Interaction Wishlist for Fractal

To minimize cognitive load, reduce context window usage, and eliminate ambiguity for local and remote LLMs interacting with the Fractal engine, the following features are proposed for the CLI and API design:

## 1. Resource-Oriented Hierarchy
Instead of a flat list of commands with complex flags, use a tiered approach based on core entities. This allows an agent to navigate via specific types rather than "fishing" through available tools.
- `fractal project [subcommand]`: Operations on the whole container (e.g., `validate`, `repair`, `status`).
- `fractal page [action]`: Actions targeted at specific content (e.g., `get`, `create`, `move`, `rename`).
- `fractal graph [query]`: Analysis and traversal of relations (e.g., `neighbors`, `path_to`, `orphans`).
- `fractal search [query]`: Multi-modal discovery across titles, tags, content, and metadata.

## 2. Machine-Readable Outputs
To eliminate parsing errors for LLMs, provide deterministic output formats:
- **Strict JSON Mode:** A `--json` flag that overrides human-friendly formatting to ensure the LLM receives valid objects.
- **Structured Status Codes:** Instead of generic "Error" strings, return specific codes (e.g., `ERR_PAGE_NOT_FOUND`, `ERR_LINK_MALFORMED`) so the agent can decide on a recovery path.

## 3. LLM-Optimized Context Extraction
Support for capabilities that allow an LLM to map a large project without reading every file:
- **Summaries by Default:** A `--summary` flag that returns only the page title, tags, and a short description (if available in metadata).
- **Context Scoping:** Commands that return "neighborhoods" (the current page plus its immediate graph neighbors) to help the LLM understand surrounding context.
- **Graph Diffing:** Highlighting exactly what changed during an operation so the agent can confirm success without re-fetching the whole state.

## 4. Automated Reasoning Guards
Features to simplify logic that usually requires multiple "thinking" steps:
- **Automatic Linking:** A robust backend process for resolving links, keeping it out of the LLM's loop.
- **Validation as a first-class citizen:** Fast validation commands that allow an agent to check its own work before presenting it to a human.
- **Breadcrumb Context:** Returning the "path" or location of a page in the graph so the LLM maintains spatial awareness within the hierarchy.

## 5. Explicit API Boundaries (The "Core vs UI" Split)
- Ensure that the CLI is a thin wrapper over a powerful Rust library. This allows agents to use the high-level Rust SDK directly if they are integrated into other tools, bypassing even the CLI's parsing stage.

---

## Gemma4 e4b

---

# 🛠️ Fractal CLI Interface Wishlist & Design Blueprint

**Target Audience:** LLM Agents (Local, Small Models) / Human Power Users
**Primary Goal:** Maximize reliable automation by minimizing required context, reducing structural uncertainty, and making state changes explicit. The interface should be highly predictable for NLU/LLMs.
**Output Format Standard:** All complex API endpoints (Graph, Mutation) must default to structured JSON or YAML output to facilitate programmatic parsing by agents, unless expressly overridden by a `--human-readable` flag.

---

## 🏛️ I. Core Principles for Cognitive Load Reduction

1.  **Explicit Intent is King:** Every operation that changes state (Mutation) or discovers structure (Graph Query) must require explicit parameters and flags to confirm intent (`--force`, `--validate`).
2.  **Granular Errors:** Instead of general failures, the system must report *specific* violations of invariants (e.g., "Page 'X' references non-existent label 'Y'" in `graph validate` output).
3.  **Self-Correction & Validation:** The CLI should prioritize validation commands that generate a differential report detailing potential issues before allowing an unsafe write operation.

---

## 🌳 II. Proposed Command Hierarchy (Resource/Operation Model)

The fundamental structure follows the pattern: `fractal <resource> <action> [params...]`

### `fractal project` (Project-Level Management)
*   **Purpose:** Initialization, structural integrity checks, and large-scale bulk operations. Handles the "project contract."
*   **Key Commands:**
    *   `project validate`: Runs a full check of all invariants across *all* pages/links in the vault. **Output Detail:** Structured report detailing *broken contracts* (e.g., duplicate label detected, orphaned index pointers).
    *   `project repair --scope <area>`: Attempts non-destructive, safe repairs based on common failure modes. Must generate a detailed `--report-changes` flag outputting what was repaired and why.
    *   `project structure audit`: Reports global metrics: link density, average backlink count, most/least connected nodes, potential circular dependencies.

### `fractal page` (Page & Metadata Operations)
*   **Purpose:** Content modification and structured metadata management for a single document. Defaults to working on the current directory or specified path.
*   **Key Commands:**
    *   `page read <path>`: Reads content, but *also* returns all associated project-owned metadata (Title, Summary, Tags) in an easy-to-parse JSON blob alongside the raw file content.
    *   `page set-metadata`: Dedicated for structured data updates. Prevents ambiguity between prose and metadata.
        *   Example: `fractal page meta set --key tags --value "AI,Draft" <path>`
        *   Example: `fractal page meta summarize --llm internal` (Triggers AI summary generation/update)
    *   `page move`: Handles renaming/moving paths while meticulously rewriting all internal hard links and metadata pointers. Must confirm affected link count *before* execution.

### `fractal graph` (Structural & Linkage Operations)
*   **Purpose:** The central brain. Exposing the linked knowledge base structure for discovery and query, minimizing the need for LLMs to manually traverse file paths. This is where LLMs operate safely.
*   **Key Commands:**
    *   `graph search <query> [filters]`: Primary NLP-powered lookup (e.g., `search "cognitive load reduction" near:2`). Structured output including `[title, location, snippet, relevance_score]`.
    *   `graph traverse --depth <N> --from <path>`: Simulates neighborhood traversal for precise context snippets. Output limited to the raw link text and surrounding prose (max 5 sentences) to minimize token usage.
    *   `graph backlinks <page_path>`: List all source pages linking *to* the specified path, providing both title and snippet preview on each result.

### `fractal sync` (Import & Export)
*   **Purpose:** Making data ingress/egress predictable for CI/CD pipelines or external services.
*   **Key Commands:**
    *   `sync import <format> --target-schema <yaml>`: Accepts a defined schema to process formats like Obsidian links, Notion blocks, etc., into valid Fractal content. **Crucial addition:** Ability to map *external* field names (`NotionProperty`) to *internal* metadata keys (`Summary`).
    *   `sync export <format> --metadata-filter=<key>`: Extracts data in a highly structured format (e.g., CSV or JSON lines) that retains the explicit relationship pointers, making it useful for training/data warehousing.

---

## ✨ III. Advanced Considerations for LLM Agents

1.  **Differential Change Detection:** Any mutation command must not just report success; it must generate an actionable diff:
    *   `[CREATED]`: What was added (e.g., "New metadata tag").
    *   `[MODIFIED]`: What changed (e.g., "Summary updated from X to Y").
    *   `[DELETED]`: What structural elements were removed or broken (e.g., "Broken link detected at old path Z").
2.  **Semantic Context Extraction:** Implement a `graph context generate` command that doesn't just summarize the page, but extracts and lists structured potential entities/concepts found within the current page content in an easy-to-consume format:
    *   Example Output: `{"entities": [{"type": "Concept", "name": "Cognitive Load", "mention_count": 3}, {"type": "Tool", "name": "LLM Agent", ...}]}`.
3.  **Atomic Operations:** Ideally, complex operations (e.g., "Update meta and rename") should be broken down into a single atomic transaction that fails entirely if *any* step fails, preventing inconsistent project states.
***"````*, the generated markdown is comprehensive, structured, and focuses on machine-agent interaction while maintaining human workflow clarity. I have added sections for advanced considerations to specifically meet your goal of reducing cognitive load for LLMs.`

---

## Gemma4 e2b

---

# Fractal CLI Wishlist

## Core Structure & Navigation
Focus on resource-based hierarchy:

*   **Projects:** Commands for lifecycle management (create, import, export).
*   **Pages:** Operations specific to individual pages (read, mutate, delete).
*   **Links/Graph:** Operations managing relationships and structure.
*   **Search:** Advanced methods for discovery across the graph and content.

## LLM & Cognitive Load Reduction Features
Features aimed at reducing context usage and uncertainty for agents:

1.  **Inferred Structure Reporting:** Commands that explicitly report on inferred structure (e.g., `fractal project validate` or a dedicated `graph report`) to let LLMs understand the baseline state before making changes.
2.  **Validated Mutations:** Operations that return explicit reports on *what* structural links, indexes, or graph data were updated, ensuring LLMs don't rely on stale assumptions about the project invariants.
3.  **Semantic Search First:** Tighter integration for semantic search (`search` command) that natively leverages entity extraction and embeddings to suggest context-aware navigation before raw text searching.
4.  **Orphan Detection Automation:** A dedicated command or feature that actively runs discovery algorithms (orphaned page detection, link validation) as a default step during project operations, rather than forcing the user to manually run separate checks.
5.  **Structured Output for Graph Queries:** Providing query interfaces specifically for graph traversal (`query`, `traverse`) that return structured JSON/YAML representations of local neighborhoods, making it easier for LLMs to ingest and reason about relationships.

## Mutability & Safety
*   **Safe Editing Contexts:** Mechanisms to scope mutations, ensuring edits respect Fractal invariants (e.g., automatically flagging links as invalid if a page is deleted).
*   **Explicit Change Reporting:** All write/mutate operations must return a structured report detailing specific changes to managed data (metadata, links, graph nodes) rather than just file diffs.
