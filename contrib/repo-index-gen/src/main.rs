//! repo-index-gen: сгенерировать `index.toml` по каталогу репозитория.
//!
//! Ожидаемая структура каталога:
//!
//! ```text
//! <root>/
//! ├── recipes/<name>/<ver>/pkg.toml
//! └── dist/<name>/<ver>/<tarball>
//! ```
//!
//! Пример:
//!
//! ```sh
//! repo-index-gen /srv/repo/stable --name stable --origin https://repo.example.com/stable
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use oparysh_repo::{write_index, GenIndexOptions};

#[derive(Parser)]
#[command(name = "repo-index-gen", version, about = "Сгенерировать index.toml по каталогу рецептов")]
struct Args {
    /// Корень репозитория (содержит recipes/ и dist/).
    dir: PathBuf,

    /// Имя репозитория ([repo] name).
    #[arg(long)]
    name: String,

    /// Origin репозитория ([repo] .origin), например https://repo.example.com/stable.
    #[arg(long)]
    origin: Option<String>,

    /// Куда записать index.toml (по умолчанию <dir>/index.toml).
    #[arg(long)]
    output: Option<PathBuf>,

    /// Переопределить updated (unix-секунды); полезно для воспроизводимых сборок.
    #[arg(long)]
    updated: Option<u64>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let mut options = GenIndexOptions::new(&args.name);
    options.origin = args.origin;
    options.updated = args.updated;

    let output = args.output.unwrap_or_else(|| args.dir.join("index.toml"));

    match write_index(&args.dir, &options, &output) {
        Ok(index) => {
            println!(
                "index.toml: {} пакет(ов) -> {}",
                index.pkg.len(),
                output.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("repo-index-gen: {e}");
            ExitCode::FAILURE
        }
    }
}
