use crate::project::{
    add_note, build_index, export_page, import_markdown, init_project, new_page, patch_note,
    remove_note, validate_project,
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
    Validate,
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
    /// Manage pages in the project.
    Page {
        #[arg(required = true)]
        args: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum IndexCommand {
    /// Build the generated page index.
    Build,
}

enum ParsedPageCommand {
    New {
        path: PathBuf,
    },
    NoteAdd {
        path: PathBuf,
        trigger: String,
        content: String,
    },
    NoteRemove {
        path: PathBuf,
        trigger: String,
    },
    NotePatch {
        path: PathBuf,
        trigger: String,
        content: String,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { project_name } => init_project(&project_name),
        Command::Validate => validate_project("."),
        Command::Import { path } => import_markdown(".", &path),
        Command::Export { page, output } => export_page(".", &page, &output),
        Command::Index { command } => match command {
            IndexCommand::Build => build_index("."),
        },
        Command::Page { args } => match parse_page_command(args)? {
            ParsedPageCommand::New { path } => new_page(".", &path),
            ParsedPageCommand::NoteAdd {
                path,
                trigger,
                content,
            } => add_note(".", &path, &trigger, &content),
            ParsedPageCommand::NoteRemove { path, trigger } => remove_note(".", &path, &trigger),
            ParsedPageCommand::NotePatch {
                path,
                trigger,
                content,
            } => patch_note(".", &path, &trigger, &content),
        },
    }
}

fn parse_page_command(args: Vec<String>) -> Result<ParsedPageCommand> {
    match args.as_slice() {
        [command, path] if command == "new" => Ok(ParsedPageCommand::New {
            path: PathBuf::from(path),
        }),
        [path, note, add, trigger, content] if note == "note" && add == "add" => {
            Ok(ParsedPageCommand::NoteAdd {
                path: PathBuf::from(path),
                trigger: trigger.clone(),
                content: content.clone(),
            })
        }
        [path, note, remove, trigger] if note == "note" && remove == "remove" => {
            Ok(ParsedPageCommand::NoteRemove {
                path: PathBuf::from(path),
                trigger: trigger.clone(),
            })
        }
        [path, note, patch, trigger, content] if note == "note" && patch == "patch" => {
            Ok(ParsedPageCommand::NotePatch {
                path: PathBuf::from(path),
                trigger: trigger.clone(),
                content: content.clone(),
            })
        }
        _ => Err(
            "invalid `page` command. Use `fractal page new <page/path>` or `fractal page <page/path> note add|remove|patch ...`"
                .into(),
        ),
    }
}
