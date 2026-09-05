use crate::{NativePageDraft, Project, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(version, about = "Engine for native Fractal documents")]
struct Cli {
    #[arg(short, long, global = true, default_value = ".")]
    project: PathBuf,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Inspect,
    Recover,
    List,
    Folders,
    Folder {
        folder: PathBuf,
    },
    NewFolder {
        title: String,
        #[arg(long, default_value = ".")]
        parent: PathBuf,
    },
    SetFolderTitle {
        folder: PathBuf,
        title: String,
    },
    SetPageTitle {
        page: PathBuf,
        title: String,
        #[arg(long)]
        expected_hash: Option<String>,
    },
    ReorderFolder {
        folder: PathBuf,
        children: Vec<String>,
    },
    Read {
        page: PathBuf,
        #[arg(long)]
        source: bool,
    },
    Parts {
        page: PathBuf,
    },
    SetContent {
        page: PathBuf,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        expected_hash: String,
    },
    SetStyle {
        page: PathBuf,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        expected_hash: String,
    },
    RestoreStyle {
        page: PathBuf,
        #[arg(long)]
        expected_hash: String,
    },
    SetMetadata {
        page: PathBuf,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        expected_hash: String,
    },
    RepairPage {
        page: PathBuf,
    },
    RepairProject,
    New {
        title: String,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    Recreate {
        page: PathBuf,
        #[arg(long)]
        draft: PathBuf,
    },
    Move {
        page: PathBuf,
        destination: PathBuf,
    },
    MoveFolder {
        folder: PathBuf,
        destination: PathBuf,
    },
    Delete {
        page: PathBuf,
    },
    DeletePages {
        pages: Vec<PathBuf>,
    },
    DeleteFolder {
        folder: PathBuf,
    },
    Search {
        query: String,
    },
    Links {
        page: PathBuf,
    },
    Backlinks {
        page: PathBuf,
    },
    DerivedLinks {
        page: PathBuf,
    },
    Link {
        page: PathBuf,
        text: String,
        target: PathBuf,
    },
    ExportHtml {
        page: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        include_derived_links: bool,
    },
    ExportFolderHtml {
        folder: PathBuf,
        selections: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        number_sections: bool,
        #[arg(long)]
        include_derived_links: bool,
        #[arg(long)]
        force: bool,
    },
    Check,
}

/// Parses process arguments, runs the selected command, and writes its output.
pub fn run() -> Result<()> {
    execute(Cli::parse())
}

/// Runs the CLI and reports failures in the requested output format.
///
/// This is the process entry point used by the `fractal` binary. Library
/// callers should use [`run`] when they need to handle errors themselves.
#[doc(hidden)]
pub fn run_and_report() -> ExitCode {
    let json_requested = std::env::args_os().any(|argument| argument == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if error.exit_code() == 0 {
                let _ = error.print();
                return ExitCode::SUCCESS;
            }
            if json_requested {
                report_error(&crate::FractalError::invalid_input(error.to_string()), true);
            } else {
                let _ = error.print();
            }
            return ExitCode::from(error.exit_code() as u8);
        }
    };
    let json = cli.json;
    match execute(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report_error(&error, json);
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init { path, name } => {
            let name = name
                .or_else(|| {
                    path.file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "Fractal project".into());
            let project = Project::init(&path, name)?;
            output(&project.manifest(), cli.json);
        }
        Command::Inspect => output(&Project::inspect(&cli.project)?, cli.json),
        Command::Recover => output(&Project::recover(&cli.project)?, cli.json),
        command => {
            let mut project = Project::open(&cli.project)?;
            match command {
                Command::List => output(&project.pages(), cli.json),
                Command::Folders => output(&project.folders(), cli.json),
                Command::Folder { folder } => output(&project.folder(folder)?, cli.json),
                Command::NewFolder { title, parent } => {
                    output(&project.create_folder(parent, &title)?, cli.json)
                }
                Command::SetFolderTitle { folder, title } => {
                    output(&project.set_folder_title(folder, &title)?, cli.json)
                }
                Command::SetPageTitle {
                    page,
                    title,
                    expected_hash,
                } => output(
                    &if let Some(expected_hash) = expected_hash {
                        project.set_page_title_if_unchanged(page, &title, &expected_hash)?
                    } else {
                        project.set_page_title(page, &title)?
                    },
                    cli.json,
                ),
                Command::ReorderFolder { folder, children } => {
                    output(&project.reorder_folder(folder, children)?, cli.json)
                }
                Command::Read { page, source } => {
                    if source {
                        output(&project.source(page)?, cli.json)
                    } else {
                        output(&project.page(page)?, cli.json)
                    }
                }
                Command::Parts { page } => output(&project.native_document_parts(page)?, cli.json),
                Command::SetContent {
                    page,
                    file,
                    expected_hash,
                } => output(
                    &project.set_page_content(
                        page,
                        &std::fs::read_to_string(file)?,
                        &expected_hash,
                    )?,
                    cli.json,
                ),
                Command::SetStyle {
                    page,
                    file,
                    expected_hash,
                } => output(
                    &project.set_page_style(
                        page,
                        &std::fs::read_to_string(file)?,
                        &expected_hash,
                    )?,
                    cli.json,
                ),
                Command::RestoreStyle {
                    page,
                    expected_hash,
                } => output(
                    &project.restore_default_page_style(page, &expected_hash)?,
                    cli.json,
                ),
                Command::SetMetadata {
                    page,
                    file,
                    expected_hash,
                } => output(
                    &project.set_page_metadata(
                        page,
                        &std::fs::read_to_string(file)?,
                        &expected_hash,
                    )?,
                    cli.json,
                ),
                Command::RepairPage { page } => {
                    output(&project.repair_page_structure(page)?, cli.json)
                }
                Command::RepairProject => output(&project.repair()?, cli.json),
                Command::New { title, path } => {
                    let result = if let Some(path) = path {
                        project.create_page_at(path, &title)?
                    } else {
                        project.create_page(&title)?
                    };
                    output(&result, cli.json);
                }
                Command::Recreate { page, draft } => {
                    let draft: NativePageDraft =
                        serde_json::from_str(&std::fs::read_to_string(draft)?)?;
                    output(&project.recreate_page_from_draft(page, &draft)?, cli.json)
                }
                Command::Move { page, destination } => {
                    output(&project.move_page(page, destination)?, cli.json)
                }
                Command::MoveFolder {
                    folder,
                    destination,
                } => output(&project.move_folder(folder, destination)?, cli.json),
                Command::Delete { page } => output(&project.delete_page(page)?, cli.json),
                Command::DeletePages { pages } => output(&project.delete_pages(pages)?, cli.json),
                Command::DeleteFolder { folder } => {
                    output(&project.delete_folder(folder)?, cli.json)
                }
                Command::Search { query } => output(&project.search(&query), cli.json),
                Command::Links { page } => output(&project.links(page)?, cli.json),
                Command::Backlinks { page } => output(&project.backlinks(page)?, cli.json),
                Command::DerivedLinks { page } => output(&project.derived_links(page)?, cli.json),
                Command::Link { page, text, target } => {
                    output(&project.insert_link(page, &text, target)?, cli.json)
                }
                Command::ExportHtml {
                    page,
                    output: destination,
                    include_derived_links,
                } => output(
                    &project.export_html(
                        page,
                        destination,
                        crate::HtmlExportOptions {
                            include_derived_links,
                        },
                    )?,
                    cli.json,
                ),
                Command::ExportFolderHtml {
                    folder,
                    selections,
                    output: destination,
                    number_sections,
                    include_derived_links,
                    force,
                } => output(
                    &project.export_folder_html(
                        folder,
                        destination,
                        crate::FolderHtmlExportOptions {
                            selections,
                            number_sections,
                            include_derived_links,
                            force,
                        },
                    )?,
                    cli.json,
                ),
                Command::Check => output(&project.validate(), cli.json),
                Command::Init { .. } | Command::Inspect | Command::Recover => unreachable!(),
            }
        }
    }
    Ok(())
}

fn report_error(error: &crate::FractalError, json: bool) {
    if json {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(error).expect("serializable error")
        );
    } else {
        eprintln!("Error: {error}");
    }
}

fn output(value: &impl Serialize, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(value).expect("serializable output")
        );
    } else {
        let value = serde_json::to_value(value).expect("serializable output");
        match value {
            serde_json::Value::String(value) => println!("{value}"),
            _ => println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("serializable output")
            ),
        }
    }
}
