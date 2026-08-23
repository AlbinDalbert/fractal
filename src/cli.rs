use crate::{Project, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Engine for native Fractal documents and raw HTML")]
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
    List,
    Read {
        page: PathBuf,
        #[arg(long)]
        source: bool,
    },
    New {
        title: String,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    Write {
        page: PathBuf,
        #[arg(long)]
        file: PathBuf,
    },
    Move {
        page: PathBuf,
        destination: PathBuf,
    },
    Delete {
        page: PathBuf,
    },
    Search {
        query: String,
    },
    Links {
        page: PathBuf,
    },
    Iframes {
        page: PathBuf,
    },
    Backlinks {
        page: PathBuf,
    },
    EmbeddedBy {
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
    Check,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
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
        command => {
            let mut project = Project::open(&cli.project)?;
            match command {
                Command::List => output(&project.pages(), cli.json),
                Command::Read { page, source } => {
                    if source {
                        output(&project.source(page)?, cli.json)
                    } else {
                        output(&project.page(page)?, cli.json)
                    }
                }
                Command::New { title, path } => {
                    let result = if let Some(path) = path {
                        project.create_page_at(path, &title)?
                    } else {
                        project.create_page(&title)?
                    };
                    output(&result, cli.json);
                }
                Command::Write { page, file } => output(
                    &project.write_page(page, &std::fs::read_to_string(file)?)?,
                    cli.json,
                ),
                Command::Move { page, destination } => {
                    output(&project.move_page(page, destination)?, cli.json)
                }
                Command::Delete { page } => output(&project.delete_page(page)?, cli.json),
                Command::Search { query } => output(&project.search(&query), cli.json),
                Command::Links { page } => output(&project.links(page)?, cli.json),
                Command::Iframes { page } => output(&project.iframes(page)?, cli.json),
                Command::Backlinks { page } => output(&project.backlinks(page)?, cli.json),
                Command::EmbeddedBy { page } => output(&project.iframe_backlinks(page)?, cli.json),
                Command::DerivedLinks { page } => output(&project.derived_links(page)?, cli.json),
                Command::Link { page, text, target } => {
                    output(&project.insert_link(page, &text, target)?, cli.json)
                }
                Command::Check => output(&project.validate(), cli.json),
                Command::Init { .. } => unreachable!(),
            }
        }
    }
    Ok(())
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
