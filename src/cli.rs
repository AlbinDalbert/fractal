use crate::project::{
    add_note, build_index, delete_page, editor_page_detail, export_page, graph_backlinks_report,
    graph_neighbors_report, graph_notes_report, graph_orphans_report, graph_outlinks_report,
    graph_page_report, graph_related_report, import_markdown, init_project_at, list_editor_pages,
    neighbor_pages, new_page, patch_note, read_page_source, remove_note, rename_page,
    repair_project, search_report, sync_project, update_editor_page, validate_project,
    EditorPageUpdate, OperationEvent, OperationReport, PageRename,
};
use crate::Result;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "fractal")]
#[command(about = "Fractal project scaffolding CLI")]
pub struct Cli {
    /// Project root. Defaults to the current directory.
    #[arg(long, global = true, default_value = ".")]
    project: PathBuf,
    /// Output format.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    /// Alias for --format json.
    #[arg(long, global = true)]
    json: bool,
    /// Print less human output where supported.
    #[arg(long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

impl Cli {
    fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Project-level lifecycle, validation, repair, and sync operations.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Manage generated project data.
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
    /// Query the generated project graph.
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
    /// Search the generated project index.
    Search {
        #[command(subcommand)]
        command: Option<SearchCommand>,
        /// Legacy query form. Prefer `fractal search text <query>`.
        query: Option<String>,
    },
    /// Manage pages in the project.
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
    /// Manage page-local notes.
    Note {
        #[command(subcommand)]
        command: NoteCommand,
    },
    /// Build compact LLM/agent context packets.
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    /// Import external content into Fractal.
    Import {
        #[command(subcommand)]
        command: Option<ImportCommand>,
        /// Legacy markdown source. Prefer `fractal import markdown <source>`.
        source: Option<PathBuf>,
    },
    /// Export Fractal content.
    Export {
        #[command(subcommand)]
        command: Option<ExportCommand>,
        /// Legacy page argument. Prefer `fractal export markdown <page> --to <path>`.
        page: Option<PathBuf>,
        /// Legacy output argument. Prefer `fractal export markdown <page> --to <path>`.
        output: Option<PathBuf>,
    },
    /// Describe machine-readable CLI capabilities.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },

    /// Legacy alias for `project init <path> --name <path>`.
    #[command(hide = true)]
    Init { project_name: String },
    /// Legacy alias for `project validate`.
    #[command(hide = true)]
    Validate,
    /// Legacy alias for `project sync`.
    #[command(hide = true)]
    Sync,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Create a new Fractal project folder with the default layout.
    Init {
        /// Path of the project directory to create.
        path: PathBuf,
        /// Project display name. Defaults to the directory name.
        #[arg(long)]
        name: Option<String>,
    },
    /// Validate the current Fractal project without writing.
    Validate,
    /// Repair missing Fractal scaffold/page markers, then validate.
    Repair,
    /// Rebuild generated data and sync inferred links across pages.
    Sync,
}

#[derive(Debug, Subcommand)]
enum IndexCommand {
    /// Build the generated page index and graph.
    Build,
}

#[derive(Debug, Subcommand)]
enum GraphCommand {
    /// Show backlinks and outlinks for a page.
    Page { page: PathBuf },
    /// Show pages that link to a page.
    Backlinks { page: PathBuf },
    /// Show pages linked from a page.
    Outlinks { page: PathBuf },
    /// Show graph-adjacent pages.
    Related { page: PathBuf },
    /// Show depth-limited neighboring pages.
    Neighbors {
        page: PathBuf,
        #[arg(long, default_value_t = 1)]
        depth: usize,
    },
    /// Show notes contained by a page.
    Notes { page: PathBuf },
    /// List pages with no backlinks.
    Orphans,
}

#[derive(Debug, Subcommand)]
enum SearchCommand {
    /// Keyword search across indexed titles, summaries, tags, notes, and link text.
    Text { query: String },
}

#[derive(Debug, Subcommand)]
enum PageCommand {
    /// List pages with metadata and graph facts.
    List,
    /// Read one page.
    Read {
        page: PathBuf,
        #[arg(long, value_enum, default_value_t = PageView::Agent)]
        view: PageView,
    },
    /// Create a new page.
    #[command(alias = "new")]
    Create { page: PathBuf },
    /// Mutate Fractal-owned page fields.
    Set {
        page: PathBuf,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long = "body-file")]
        body_file: Option<PathBuf>,
    },
    /// Move/rename a page, optionally changing its title.
    Move {
        page: PathBuf,
        #[arg(long)]
        to: PathBuf,
        #[arg(long)]
        title: Option<String>,
    },
    /// Delete a page.
    Delete {
        page: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// Read raw page source.
    Source {
        #[command(subcommand)]
        command: PageSourceCommand,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PageView {
    Agent,
    Metadata,
    Source,
}

#[derive(Debug, Subcommand)]
enum PageSourceCommand {
    Read { page: PathBuf },
}

#[derive(Debug, Subcommand)]
enum NoteCommand {
    /// Add a note whose id is derived from the trigger text.
    Add {
        page: PathBuf,
        trigger: String,
        #[arg(long)]
        content: String,
    },
    /// Remove a note whose id is derived from the trigger text.
    Remove { page: PathBuf, trigger: String },
    /// Replace a note body.
    #[command(alias = "patch")]
    Set {
        page: PathBuf,
        trigger: String,
        #[arg(long)]
        content: String,
    },
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    /// Return compact context for one page.
    Page {
        page: PathBuf,
        #[arg(long)]
        budget: Option<usize>,
    },
}

#[derive(Debug, Subcommand)]
enum ImportCommand {
    /// Import a markdown file.
    Markdown { source: PathBuf },
}

#[derive(Debug, Subcommand)]
enum ExportCommand {
    /// Export a page as markdown.
    Markdown {
        page: PathBuf,
        #[arg(long)]
        to: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    /// List canonical commands.
    Commands,
}

#[derive(Debug, Serialize)]
struct CommandResult<T: Serialize> {
    ok: bool,
    schema: &'static str,
    command: &'static str,
    project: ProjectRef,
    data: T,
}

#[derive(Debug, Serialize)]
struct ProjectRef {
    root: PathBuf,
}

#[derive(Debug, Serialize)]
struct ReportData {
    report: OperationReport,
}

#[derive(Debug, Serialize)]
struct ContextPageData {
    page: crate::project::EditorPageDetail,
    neighbors: Vec<crate::project::GraphNeighborPage>,
    budget: Option<usize>,
}

#[derive(Debug, Serialize)]
struct SchemaCommandEntry {
    name: &'static str,
    kind: &'static str,
    json: bool,
    examples: &'static [&'static str],
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.project.clone();
    let output_format = cli.output_format();

    match cli.command {
        Command::Project { command } => match command {
            ProjectCommand::Init { path, name } => {
                let name = name.unwrap_or_else(|| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("fractal-project")
                        .to_string()
                });
                let report = init_project_at(&path, &name)?;
                print_report_result(output_format, "project.init", &root, &report)
            }
            ProjectCommand::Validate => {
                let report = validate_project(&root)?;
                print_report_result(output_format, "project.validate", &root, &report)
            }
            ProjectCommand::Repair => {
                let report = repair_project(&root)?;
                print_report_result(output_format, "project.repair", &root, &report)
            }
            ProjectCommand::Sync => {
                let report = sync_project(&root)?;
                print_report_result(output_format, "project.sync", &root, &report)
            }
        },
        Command::Index { command } => match command {
            IndexCommand::Build => {
                let report = build_index(&root)?;
                print_report_result(output_format, "index.build", &root, &report)
            }
        },
        Command::Graph { command } => match command {
            GraphCommand::Page { page } => print_text_or_json(
                output_format,
                "graph.page",
                &root,
                graph_page_report(&root, &page)?,
            ),
            GraphCommand::Backlinks { page } => print_text_or_json(
                output_format,
                "graph.backlinks",
                &root,
                graph_backlinks_report(&root, &page)?,
            ),
            GraphCommand::Outlinks { page } => print_text_or_json(
                output_format,
                "graph.outlinks",
                &root,
                graph_outlinks_report(&root, &page)?,
            ),
            GraphCommand::Related { page } => print_text_or_json(
                output_format,
                "graph.related",
                &root,
                graph_related_report(&root, &page)?,
            ),
            GraphCommand::Neighbors { page, depth } => print_text_or_json(
                output_format,
                "graph.neighbors",
                &root,
                graph_neighbors_report(&root, &page, depth)?,
            ),
            GraphCommand::Notes { page } => print_text_or_json(
                output_format,
                "graph.notes",
                &root,
                graph_notes_report(&root, &page)?,
            ),
            GraphCommand::Orphans => print_text_or_json(
                output_format,
                "graph.orphans",
                &root,
                graph_orphans_report(&root)?,
            ),
        },
        Command::Search { command, query } => {
            let query = match command {
                Some(SearchCommand::Text { query }) => query,
                None => query.ok_or("missing search query; use `fractal search text <query>`")?,
            };
            print_text_or_json(
                output_format,
                "search.text",
                &root,
                search_report(&root, &query)?,
            )
        }
        Command::Page { command } => match command {
            PageCommand::List => {
                let pages = list_editor_pages(&root)?;
                print_data(output_format, "page.list", &root, &pages, || {
                    for page in &pages {
                        println!("{}\t{}", page.path, page.title);
                    }
                    Ok(())
                })
            }
            PageCommand::Read { page, view } => match view {
                PageView::Agent => {
                    let detail = editor_page_detail(&root, &page)?;
                    print_data(output_format, "page.read", &root, &detail, || {
                        println!("{}", detail.metadata.title);
                        println!("path: {}", detail.metadata.path);
                        if let Some(summary) = &detail.metadata.summary {
                            println!("summary: {summary}");
                        }
                        if !detail.metadata.tags.is_empty() {
                            println!("tags: {}", detail.metadata.tags.join(", "));
                        }
                        println!("\n{}", detail.body_html);
                        Ok(())
                    })
                }
                PageView::Metadata => {
                    let detail = editor_page_detail(&root, &page)?;
                    print_data(output_format, "page.read", &root, &detail.metadata, || {
                        println!("title: {}", detail.metadata.title);
                        println!("path: {}", detail.metadata.path);
                        if let Some(summary) = &detail.metadata.summary {
                            println!("summary: {summary}");
                        }
                        println!("tags: {}", detail.metadata.tags.join(", "));
                        Ok(())
                    })
                }
                PageView::Source => {
                    let source = read_page_source(&root, &page)?;
                    print_data(output_format, "page.read", &root, &source, || {
                        print!("{}", source.html);
                        Ok(())
                    })
                }
            },
            PageCommand::Create { page } => {
                let report = new_page(&root, &page)?;
                print_report_result(output_format, "page.create", &root, &report)
            }
            PageCommand::Set {
                page,
                title,
                summary,
                tags,
                body_file,
            } => {
                let body_html = match body_file {
                    Some(path) => Some(std::fs::read_to_string(path)?),
                    None => None,
                };
                let update = EditorPageUpdate {
                    title,
                    body_html,
                    summary,
                    tags: if tags.is_empty() { None } else { Some(tags) },
                };
                let report = update_editor_page(&root, &page, update)?;
                print_report_result(output_format, "page.set", &root, &report)
            }
            PageCommand::Move { page, to, title } => {
                let report = rename_page(
                    &root,
                    &page,
                    PageRename {
                        path: Some(to),
                        title,
                    },
                )?;
                print_report_result(output_format, "page.move", &root, &report)
            }
            PageCommand::Delete { page, yes } => {
                if !yes {
                    return Err("page delete requires --yes".into());
                }
                let report = delete_page(&root, &page)?;
                print_report_result(output_format, "page.delete", &root, &report)
            }
            PageCommand::Source { command } => match command {
                PageSourceCommand::Read { page } => {
                    let source = read_page_source(&root, &page)?;
                    print_data(output_format, "page.source.read", &root, &source, || {
                        print!("{}", source.html);
                        Ok(())
                    })
                }
            },
        },
        Command::Note { command } => match command {
            NoteCommand::Add {
                page,
                trigger,
                content,
            } => {
                let report = add_note(&root, &page, &trigger, &content)?;
                print_report_result(output_format, "note.add", &root, &report)
            }
            NoteCommand::Remove { page, trigger } => {
                let report = remove_note(&root, &page, &trigger)?;
                print_report_result(output_format, "note.remove", &root, &report)
            }
            NoteCommand::Set {
                page,
                trigger,
                content,
            } => {
                let report = patch_note(&root, &page, &trigger, &content)?;
                print_report_result(output_format, "note.set", &root, &report)
            }
        },
        Command::Context { command } => match command {
            ContextCommand::Page { page, budget } => {
                let detail = editor_page_detail(&root, &page)?;
                let neighbors = neighbor_pages(&root, &page, 1)?;
                let data = ContextPageData {
                    page: detail,
                    neighbors,
                    budget,
                };
                print_data(output_format, "context.page", &root, &data, || {
                    println!("{}", data.page.metadata.title);
                    if let Some(summary) = &data.page.metadata.summary {
                        println!("summary: {summary}");
                    }
                    println!("neighbors: {}", data.neighbors.len());
                    println!("\n{}", data.page.body_html);
                    Ok(())
                })
            }
        },
        Command::Import { command, source } => {
            let source = match command {
                Some(ImportCommand::Markdown { source }) => source,
                None => {
                    source.ok_or("missing import source; use `fractal import markdown <source>`")?
                }
            };
            let report = import_markdown(&root, &source)?;
            print_report_result(output_format, "import.markdown", &root, &report)
        }
        Command::Export {
            command,
            page,
            output,
        } => {
            let (page, output) = match command {
                Some(ExportCommand::Markdown { page, to }) => (page, to),
                None => (
                    page.ok_or(
                        "missing export page; use `fractal export markdown <page> --to <path>`",
                    )?,
                    output.ok_or(
                        "missing export output; use `fractal export markdown <page> --to <path>`",
                    )?,
                ),
            };
            let report = export_page(&root, &page, &output)?;
            print_report_result(output_format, "export.markdown", &root, &report)
        }
        Command::Schema { command } => match command {
            SchemaCommand::Commands => {
                let commands = schema_commands();
                print_data(output_format, "schema.commands", &root, &commands, || {
                    for command in &commands {
                        println!("{}\t{}", command.name, command.kind);
                    }
                    Ok(())
                })
            }
        },
        Command::Init { project_name } => {
            let path = PathBuf::from(&project_name);
            let report = init_project_at(&path, &project_name)?;
            print_report_result(output_format, "project.init", &root, &report)
        }
        Command::Validate => {
            let report = validate_project(&root)?;
            print_report_result(output_format, "project.validate", &root, &report)
        }
        Command::Sync => {
            let report = sync_project(&root)?;
            print_report_result(output_format, "project.sync", &root, &report)
        }
    }
}

fn print_report_result(
    output_format: OutputFormat,
    command: &'static str,
    root: &Path,
    report: &OperationReport,
) -> Result<()> {
    match output_format {
        OutputFormat::Human => print_operation_report(report),
        OutputFormat::Json => print_data_json(
            command,
            root,
            &ReportData {
                report: report.clone(),
            },
        ),
    }
}

fn print_text_or_json(
    output_format: OutputFormat,
    command: &'static str,
    root: &Path,
    text: String,
) -> Result<()> {
    match output_format {
        OutputFormat::Human => {
            print!("{text}");
            Ok(())
        }
        OutputFormat::Json => print_data_json(command, root, &text),
    }
}

fn print_data<T, F>(
    output_format: OutputFormat,
    command: &'static str,
    root: &Path,
    data: &T,
    print_human: F,
) -> Result<()>
where
    T: Serialize,
    F: FnOnce() -> Result<()>,
{
    match output_format {
        OutputFormat::Human => print_human(),
        OutputFormat::Json => print_data_json(command, root, data),
    }
}

fn print_data_json<T: Serialize>(command: &'static str, root: &Path, data: &T) -> Result<()> {
    let result = CommandResult {
        ok: true,
        schema: "fractal.command_result.v1",
        command,
        project: ProjectRef {
            root: root.to_path_buf(),
        },
        data,
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn print_operation_report(report: &OperationReport) -> Result<()> {
    for event in &report.events {
        match event {
            OperationEvent::AddedDirectory { path } => {
                println!("created directory {}", path.display())
            }
            OperationEvent::AddedNote { page, note_id } => {
                println!("added note {} to {}", note_id, page.display());
            }
            OperationEvent::Built { path } => println!("built {}", path.display()),
            OperationEvent::Created { path } => println!("created {}", path.display()),
            OperationEvent::Exported { page, output } => {
                println!("exported {} -> {}", page.display(), output.display());
            }
            OperationEvent::DeletedPage { path } => println!("deleted page {}", path.display()),
            OperationEvent::Fixed { path } => println!("fixed {}", path.display()),
            OperationEvent::Imported {
                source,
                destination,
            } => {
                println!("imported {} -> {}", source.display(), destination.display());
            }
            OperationEvent::MovedPage { from, to } => {
                println!("moved page {} -> {}", from.display(), to.display());
            }
            OperationEvent::PatchedNote { page, note_id } => {
                println!("patched note {} in {}", note_id, page.display());
            }
            OperationEvent::PageLinksAffected {
                page,
                backlinks,
                outlinks,
            } => println!(
                "affected links for {}: {} backlink(s), {} outlink(s)",
                page,
                backlinks.len(),
                outlinks.len()
            ),
            OperationEvent::RemovedNote { page, note_id } => {
                println!("removed note {} from {}", note_id, page.display());
            }
            OperationEvent::UpdatedPageBody { page } => {
                println!("updated page body for {}", page.display());
            }
            OperationEvent::UpdatedPageTitle { page, title } => {
                println!("updated page title for {} to {}", page.display(), title);
            }
            OperationEvent::UpdatedMetadata {
                page,
                name,
                content,
            } => println!("updated {} for {} to {}", name, page.display(), content),
            OperationEvent::UpdatedPageLinks { page, count } => {
                println!("updated {count} page link(s) in {}", page.display());
            }
            OperationEvent::UpdatedProjectManifest { path } => {
                println!("updated project manifest {}", path.display());
            }
            OperationEvent::SavedPage { path } => println!("saved {}", path.display()),
            OperationEvent::Synced { path } => println!("synced {}", path.display()),
            OperationEvent::SyncComplete { pages_updated } => {
                println!("sync complete: {pages_updated} page(s) updated");
            }
            OperationEvent::ValidProject {
                project_name,
                manifest_path,
            } => println!(
                "valid Fractal project: {} ({})",
                project_name,
                manifest_path.display()
            ),
            OperationEvent::Warning { message } => println!("warning: {message}"),
        }
    }
    Ok(())
}

fn schema_commands() -> Vec<SchemaCommandEntry> {
    vec![
        SchemaCommandEntry {
            name: "project.init",
            kind: "write",
            json: true,
            examples: &["fractal project init ./notes --name Notes"],
        },
        SchemaCommandEntry {
            name: "project.validate",
            kind: "read",
            json: true,
            examples: &["fractal project validate --json"],
        },
        SchemaCommandEntry {
            name: "project.repair",
            kind: "write",
            json: true,
            examples: &["fractal project repair --json"],
        },
        SchemaCommandEntry {
            name: "project.sync",
            kind: "write",
            json: true,
            examples: &["fractal project sync --json"],
        },
        SchemaCommandEntry {
            name: "page.list",
            kind: "read",
            json: true,
            examples: &["fractal page list --json"],
        },
        SchemaCommandEntry {
            name: "page.read",
            kind: "read",
            json: true,
            examples: &["fractal page read index --view agent --json"],
        },
        SchemaCommandEntry {
            name: "page.create",
            kind: "write",
            json: true,
            examples: &["fractal page create topic --json"],
        },
        SchemaCommandEntry {
            name: "page.set",
            kind: "write",
            json: true,
            examples: &["fractal page set index --summary 'Entry point' --json"],
        },
        SchemaCommandEntry {
            name: "page.move",
            kind: "write",
            json: true,
            examples: &["fractal page move old --to new --json"],
        },
        SchemaCommandEntry {
            name: "page.delete",
            kind: "write",
            json: true,
            examples: &["fractal page delete old --yes --json"],
        },
        SchemaCommandEntry {
            name: "note.add",
            kind: "write",
            json: true,
            examples: &["fractal note add index term --content 'Definition' --json"],
        },
        SchemaCommandEntry {
            name: "search.text",
            kind: "read",
            json: true,
            examples: &["fractal search text graph --json"],
        },
        SchemaCommandEntry {
            name: "graph.neighbors",
            kind: "read",
            json: true,
            examples: &["fractal graph neighbors index --depth 1 --json"],
        },
        SchemaCommandEntry {
            name: "context.page",
            kind: "read",
            json: true,
            examples: &["fractal context page index --budget 2000 --json"],
        },
        SchemaCommandEntry {
            name: "schema.commands",
            kind: "read",
            json: true,
            examples: &["fractal schema commands --json"],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_create_command() {
        let cli = Cli::try_parse_from(["fractal", "page", "create", "folder/topic"])
            .expect("parse page create");

        match cli.command {
            Command::Page {
                command: PageCommand::Create { page },
            } => assert_eq!(page, PathBuf::from("folder/topic")),
            command => panic!("unexpected command: {command:?}"),
        }
    }

    #[test]
    fn parses_project_validate_command() {
        let cli = Cli::try_parse_from(["fractal", "project", "validate", "--json"])
            .expect("parse project validate");

        assert_eq!(cli.output_format(), OutputFormat::Json);
        match cli.command {
            Command::Project {
                command: ProjectCommand::Validate,
            } => {}
            command => panic!("unexpected command: {command:?}"),
        }
    }

    #[test]
    fn parses_top_level_note_command() {
        let cli = Cli::try_parse_from([
            "fractal",
            "note",
            "add",
            "index",
            "java",
            "--content",
            "note body",
        ])
        .expect("parse note add");

        match cli.command {
            Command::Note {
                command:
                    NoteCommand::Add {
                        page,
                        trigger,
                        content,
                    },
            } => {
                assert_eq!(page, PathBuf::from("index"));
                assert_eq!(trigger, "java");
                assert_eq!(content, "note body");
            }
            command => panic!("unexpected command: {command:?}"),
        }
    }

    #[test]
    fn parses_search_text_command() {
        let cli = Cli::try_parse_from(["fractal", "search", "text", "rust graph"])
            .expect("parse search text");

        match cli.command {
            Command::Search {
                command: Some(SearchCommand::Text { query }),
                ..
            } => assert_eq!(query, "rust graph"),
            command => panic!("unexpected command: {command:?}"),
        }
    }

    #[test]
    fn parses_focused_graph_commands() {
        let cli = Cli::try_parse_from(["fractal", "graph", "related", "index"])
            .expect("parse graph related");

        match cli.command {
            Command::Graph {
                command: GraphCommand::Related { page },
            } => assert_eq!(page, PathBuf::from("index")),
            command => panic!("unexpected command: {command:?}"),
        }
    }
}
