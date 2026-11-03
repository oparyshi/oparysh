use clap::Parser;

#[derive(Parser)]
#[command(name = "oparysh", version, about = "Source-based package manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Verify a tarball's SHA-256 (placeholder, stage 2)
    Verify,
}

fn main() {
    let cli = Cli::parse();
    if cli.command.is_none() {
        println!("oparysh placeholder");
    }
}
