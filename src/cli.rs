use crate::project::{
    add_note, build_index, export_page, graph_backlinks_report, graph_notes_report,
    graph_orphans_report, graph_outlinks_report, graph_page_report, graph_related_report,
    import_markdown, init_project, new_page, page_metadata_report, patch_note, remove_note,
    reset_page_metadata, search_report, set_page_summary, set_page_tags, sync_project,
    validate_project, OperationEvent, OperationReport,
};
use crate::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "fractal")]
#[command(about = "Fractal project scaffolding CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new Fractal project folder with the default layout.
    Init {
        /// Name of the project directory to create.
        project_name: String,
    },
    /// Validate the current Fractal project.
    Validate {
        /// Add missing Fractal scaffold files and page markers before validating.
        #[arg(long)]
        fix: bool,
    },
    /// Import a markdown file into the current project.
    Import {
        /// Path to a markdown file.
        path: PathBuf,
    },
    /// Export a Fractal page to a destination file.
    Export {
        /// Path to a page inside the Fractal project, usually under pages/.
        page: PathBuf,
        /// Destination path for the export.
        output: PathBuf,
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
        /// Keyword query to search for.
        query: String,
    },
    /// Rebuild the project index and sync inferred links across pages.
    Sync,
    /// Manage pages in the project.
    Page {
        /// Page path relative to pages/ for page-scoped commands.
        path: Option<PathBuf>,
        #[command(subcommand)]
        command: PageCommand,
    },
}

#[derive(Debug, Subcommand)]
enum IndexCommand {
    /// Build the generated page index.
    Build,
}

#[derive(Debug, Subcommand)]
enum GraphCommand {
    /// Show backlinks and outlinks for a page.
    Page {
        /// Page path relative to pages/, with or without .html.
        page: PathBuf,
    },
    /// Show pages that link to a page.
    Backlinks {
        /// Page path relative to pages/, with or without .html.
        page: PathBuf,
    },
    /// Show pages linked from a page.
    Outlinks {
        /// Page path relative to pages/, with or without .html.
        page: PathBuf,
    },
    /// Show graph-adjacent pages.
    Related {
        /// Page path relative to pages/, with or without .html.
        page: PathBuf,
    },
    /// Show notes contained by a page.
    Notes {
        /// Page path relative to pages/, with or without .html.
        page: PathBuf,
    },
    /// List pages with no backlinks.
    Orphans,
}

#[derive(Debug, Subcommand)]
enum PageCommand {
    /// Create a new page.
    New {
        /// Page path relative to pages/, with or without .html.
        path: PathBuf,
    },
    /// Manage notes in a page.
    Note {
        #[command(subcommand)]
        command: NoteCommand,
    },
    /// Manage page metadata.
    Meta {
        #[command(subcommand)]
        command: MetaCommand,
    },
}

#[derive(Debug, Subcommand)]
enum NoteCommand {
    /// Add a note whose id is derived from the trigger text.
    Add {
        /// Trigger text to normalize into the note id.
        trigger: String,
        /// Note body text.
        content: String,
    },
    /// Remove a note whose id is derived from the trigger text.
    Remove {
        /// Trigger text to normalize into the note id.
        trigger: String,
    },
    /// Replace a note body.
    Patch {
        /// Trigger text to normalize into the note id.
        trigger: String,
        /// Replacement note body text.
        content: String,
    },
}

#[derive(Debug, Subcommand)]
enum MetaCommand {
    /// Show page metadata.
    Show,
    /// Set the page summary.
    SetSummary {
        /// Summary text.
        summary: String,
    },
    /// Replace the page tag list.
    SetTags {
        /// Tags. Values may be repeated or comma-separated.
        tags: Vec<String>,
    },
    /// Reset summary and tags to generated defaults.
    Reset,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    let report = match cli.command {
        Command::Init { project_name } => init_project(&project_name),
        Command::Validate { fix } => validate_project(".", fix),
        Command::Import { path } => import_markdown(".", &path),
        Command::Export { page, output } => export_page(".", &page, &output),
        Command::Index { command } => match command {
            IndexCommand::Build => build_index("."),
        },
        Command::Graph { command } => match command {
            GraphCommand::Page { page } => {
                print!("{}", graph_page_report(".", &page)?);
                return Ok(());
            }
            GraphCommand::Backlinks { page } => {
                print!("{}", graph_backlinks_report(".", &page)?);
                return Ok(());
            }
            GraphCommand::Outlinks { page } => {
                print!("{}", graph_outlinks_report(".", &page)?);
                return Ok(());
            }
            GraphCommand::Related { page } => {
                print!("{}", graph_related_report(".", &page)?);
                return Ok(());
            }
            GraphCommand::Notes { page } => {
                print!("{}", graph_notes_report(".", &page)?);
                return Ok(());
            }
            GraphCommand::Orphans => {
                print!("{}", graph_orphans_report(".")?);
                return Ok(());
            }
        },
        Command::Search { query } => {
            print!("{}", search_report(".", &query)?);
            return Ok(());
        }
        Command::Sync => sync_project("."),
        Command::Page { path, command } => match command {
            PageCommand::New { path } => new_page(".", &path),
            PageCommand::Note { command } => {
                let path = path.ok_or("missing page path for note command")?;
                match command {
                    NoteCommand::Add { trigger, content } => {
                        add_note(".", &path, &trigger, &content)
                    }
                    NoteCommand::Remove { trigger } => remove_note(".", &path, &trigger),
                    NoteCommand::Patch { trigger, content } => {
                        patch_note(".", &path, &trigger, &content)
                    }
                }
            }
            PageCommand::Meta { command } => {
                let path = path.ok_or("missing page path for meta command")?;
                match command {
                    MetaCommand::Show => {
                        print!("{}", page_metadata_report(".", &path)?);
                        return Ok(());
                    }
                    MetaCommand::SetSummary { summary } => set_page_summary(".", &path, &summary),
                    MetaCommand::SetTags { tags } => {
                        let tags = tags.iter().flat_map(|tag| tag.split(','));
                        set_page_tags(".", &path, tags)
                    }
                    MetaCommand::Reset => reset_page_metadata(".", &path),
                }
            }
        },
    }?;

    print_operation_report(&report);
    Ok(())
}

fn print_operation_report(report: &OperationReport) {
    for event in &report.events {
        match event {
            OperationEvent::AddedNote { page, note_id } => {
                println!("added note {} to {}", note_id, page.display());
            }
            OperationEvent::Built { path } => {
                println!("built {}", path.display());
            }
            OperationEvent::Created { path } => {
                println!("created {}", path.display());
            }
            OperationEvent::Exported { page, output } => {
                println!("exported {} -> {}", page.display(), output.display());
            }
            OperationEvent::Fixed { path } => {
                println!("fixed {}", path.display());
            }
            OperationEvent::Imported {
                source,
                destination,
            } => {
                println!("imported {} -> {}", source.display(), destination.display());
            }
            OperationEvent::PatchedNote { page, note_id } => {
                println!("patched note {} in {}", note_id, page.display());
            }
            OperationEvent::RemovedNote { page, note_id } => {
                println!("removed note {} from {}", note_id, page.display());
            }
            OperationEvent::UpdatedMetadata {
                page,
                name,
                content,
            } => {
                println!("updated {} for {} to {}", name, page.display(), content);
            }
            OperationEvent::SavedPage { path } => {
                println!("saved {}", path.display());
            }
            OperationEvent::Synced { path } => {
                println!("synced {}", path.display());
            }
            OperationEvent::SyncComplete { pages_updated } => {
                println!("sync complete: {pages_updated} page(s) updated");
            }
            OperationEvent::ValidProject {
                project_name,
                manifest_path,
            } => {
                println!(
                    "valid Fractal project: {} ({})",
                    project_name,
                    manifest_path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_new_command() {
        let cli = Cli::try_parse_from(["fractal", "page", "new", "folder/topic"])
            .expect("parse page new");

        match cli.command {
            Command::Page {
                path: None,
                command: PageCommand::New { path },
            } => assert_eq!(path, PathBuf::from("folder/topic")),
            command => panic!("unexpected command: {command:?}"),
        }
    }

    #[test]
    fn parses_existing_page_note_command_shape() {
        let cli = Cli::try_parse_from([
            "fractal",
            "page",
            "index",
            "note",
            "add",
            "java",
            "note body",
        ])
        .expect("parse page note add");

        match cli.command {
            Command::Page {
                path: Some(path),
                command:
                    PageCommand::Note {
                        command: NoteCommand::Add { trigger, content },
                    },
            } => {
                assert_eq!(path, PathBuf::from("index"));
                assert_eq!(trigger, "java");
                assert_eq!(content, "note body");
            }
            command => panic!("unexpected command: {command:?}"),
        }
    }

    #[test]
    fn parses_existing_page_meta_command_shape() {
        let cli = Cli::try_parse_from([
            "fractal",
            "page",
            "index",
            "meta",
            "set-tags",
            "rust",
            "graphs,parsing",
        ])
        .expect("parse page meta set-tags");

        match cli.command {
            Command::Page {
                path: Some(path),
                command:
                    PageCommand::Meta {
                        command: MetaCommand::SetTags { tags },
                    },
            } => {
                assert_eq!(path, PathBuf::from("index"));
                assert_eq!(tags, vec!["rust".to_string(), "graphs,parsing".to_string()]);
            }
            command => panic!("unexpected command: {command:?}"),
        }
    }

    #[test]
    fn parses_search_command() {
        let cli = Cli::try_parse_from(["fractal", "search", "rust graph"]).expect("parse search");

        match cli.command {
            Command::Search { query } => assert_eq!(query, "rust graph"),
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
