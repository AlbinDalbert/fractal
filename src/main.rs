use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mini-mark")]
#[command(about = "Marky mark, whos you favorit mark?", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Cat {
        file: String,
    },

    Convert {
        input: String,
        output: String,
        compression: String,
    },

    Export {
        input: String,
        output: String,
    }
}

fn main() {
    let cli = Cli::parse();
    println!("Hello, Mark!");

    match cli.command {
        Commands::Cat { file } => {println!("cat this file: {}", file)}
        Commands::Convert { input, output, compression } => {println!("Comvert {} into {} using {}", input, output, compression)}
        Commands::Export { input, output } => {println!("Export {} to {}", input, output)}
    }

}
