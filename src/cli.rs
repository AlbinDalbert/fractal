use crate::project::{export_page, import_markdown, init_project, validate_project};
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
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { project_name } => init_project(&project_name),
        Command::Validate => validate_project("."),
        Command::Import { path } => import_markdown(".", &path),
        Command::Export { page, output } => export_page(".", &page, &output),
    }
}
