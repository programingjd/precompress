use crate::op::Operation;
use crate::sync::compress;
use clap::Parser;
use std::env::current_dir;
use std::path::PathBuf;
use std::time::Instant;

mod ext;
mod hash;
mod op;
mod sync;

#[derive(Parser, Debug)]
#[command(
    name = "br",
    bin_name = "br",
    version = None,
    about = "Pre-compresses web content with brotli",
    long_about = None)
]
struct Args {
    #[arg(short = 'p', long, value_name = ".")]
    path: Option<PathBuf>,
    #[arg(short = 'n', long, value_name = "4")]
    threads: Option<u8>,
    #[arg(short, long)]
    force: bool,
    #[arg(long)]
    dry_run: bool,
}

fn main() {
    let args = Args::parse();
    let t0 = Instant::now();
    let dir = args
        .path
        .unwrap_or(current_dir().expect("failed to get current directory"));
    let dry_run = args.dry_run;
    let force = args.force;
    compress(
        dir,
        dry_run,
        force,
        args.threads,
        |path: &PathBuf, op: Operation| match op {
            Operation::Create => {
                println!("+{}", path.to_str().unwrap());
            }
            Operation::Update => {
                println!("*{}", path.to_str().unwrap());
            }
            _ => {}
        },
    )
    .unwrap_or_else(|e| panic!("{}", e));
    println!("done in {:?}", Instant::now().duration_since(t0));
}
