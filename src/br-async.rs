use brotli::enc::BrotliEncoderParams;
use brotli::{BrotliCompress, BrotliDecompress};
use clap::Parser;
use highway::{HighwayHash, HighwayHasher, Key};
use std::collections::{BTreeSet, LinkedList};
use std::env::current_dir;
use std::path::PathBuf;
use std::time::Instant;
use tokio::fs::{read, read_dir, write};
use tokio::io::{copy, sink};
// use tokio::io::{copy, sink};
use tokio::runtime::Builder;

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
    let t0 = Instant::now();
    let extensions: BTreeSet<&'static str> = BTreeSet::from_iter([
        "html", "htm", "css", "js", "mjs", "cjs", "wasm", "json", "map", "ts", "geojson", "kml",
        "gpx", "csv", "tsv", "txt", "md", "adoc", "glsl", "xml", "xsd", "xslt", "dtd", "manifest",
        "pbf", "pdf", "svg", "ico", "jsonld", "gltf", "glb", "atom",
    ]);
    let extensions = Box::leak(Box::new(extensions));
    let args = Args::parse();
    let dir = args
        .path
        .unwrap_or(current_dir().expect("failed to get current directory"));
    let dry_run = args.dry_run;
    let force = args.force;
    let rt = if let Some(n) = args.threads {
        Builder::new_multi_thread()
            .worker_threads(n as usize)
            .build()
    } else {
        Builder::new_multi_thread().build()
    }
    .expect("failed to create tokio runtime");
    rt.block_on(async {
        let mut list = vec![];
        let mut stack = LinkedList::new();
        stack.push_front(dir);
        while let Some(ref path) = stack.pop_front() {
            let mut entries = read_dir(path).await.unwrap_or_else(|e| {
                panic!(
                    "failed to read directory \"{}\": {e:?}",
                    path.to_string_lossy()
                )
            });
            while let Some(entry) = entries.next_entry().await.unwrap_or_else(|e| {
                panic!(
                    "failed to read directory \"{}\": {e:?}",
                    path.to_string_lossy()
                )
            }) {
                let filename = entry.file_name().into_string().unwrap_or_else(|e| {
                    panic!(
                        "failed to read file name \"{}\": {e:?}",
                        entry.path().to_string_lossy()
                    )
                });
                if filename.starts_with('.') {
                    continue;
                }
                if entry
                    .metadata()
                    .await
                    .unwrap_or_else(|e| {
                        panic!(
                            "failed to read metadata of \"{}\": {e:?}",
                            entry.path().to_string_lossy()
                        )
                    })
                    .is_dir()
                {
                    stack.push_back(entry.path());
                } else {
                    if filename.ends_with(".br") {
                        continue;
                    }
                    if let Some((_, extension)) = filename.rsplit_once('.') {
                        if extensions.contains(extension) {
                            list.push((filename, entry.path()));
                        }
                    }
                }
            }
        }
        let futures = list
            .into_iter()
            .map(|(filename, path)| {
                tokio::spawn(async move {
                    let uncompressed = read(&path).await.unwrap_or_else(|e| {
                        panic!("failed to read file \"{}\": {e:?}", path.to_string_lossy())
                    });
                    let br_path = path.parent().unwrap().join(format!("{filename}.br"));
                    let compressed = read(br_path.clone()).await.ok();
                    let operation = if let Some(compressed) = compressed {
                        if force {
                            Operation::Update
                        } else {
                            let mut decompressed = Vec::with_capacity(uncompressed.len());
                            BrotliDecompress(&mut &*compressed, &mut decompressed).unwrap();
                            if decompressed.len() == uncompressed.len()
                                && hash(&uncompressed) == hash(&decompressed)
                            {
                                Operation::Noop
                            } else {
                                Operation::Update
                            }
                        }
                    } else {
                        Operation::Create
                    };
                    if operation != Operation::Noop {
                        let mut compressed = Vec::with_capacity(uncompressed.len() + 64);
                        let params = BrotliEncoderParams {
                            quality: 11,
                            size_hint: uncompressed.len(),
                            ..BrotliEncoderParams::default()
                        };
                        BrotliCompress(&mut &*uncompressed, &mut compressed, &params)
                            .unwrap_or_else(|e| {
                                panic!(
                                    "failed to compress file \"{}\": {e:?}",
                                    path.to_string_lossy()
                                )
                            });
                        if dry_run {
                            copy(&mut compressed.as_slice(), &mut sink())
                                .await
                                .unwrap_or_else(|e| {
                                    panic!(
                                        "failed to write file \"{}\": {e:?}",
                                        br_path.to_string_lossy()
                                    )
                                });
                        } else {
                            write(&br_path, compressed).await.unwrap_or_else(|e| {
                                panic!(
                                    "failed to write file \"{}\": {e:?}",
                                    br_path.to_string_lossy()
                                )
                            });
                        }
                    }
                    match operation {
                        Operation::Create => {
                            println!("+{}", br_path.to_str().unwrap());
                        }
                        Operation::Update => {
                            println!("*{}", br_path.to_str().unwrap());
                        }
                        _ => {}
                    }
                })
            })
            .collect::<Vec<_>>();
        for future in futures.into_iter() {
            future
                .await
                .unwrap_or_else(|e| panic!("failed to run task: {e:?}"));
        }
    });
    println!("done in {:?}", Instant::now().duration_since(t0));
}

#[derive(PartialEq)]
enum Operation {
    Update,
    Create,
    Noop,
}

const VERSION: [u64; 4] = [2024u64, 4u64, 6u64, 1u64];
fn hash(data: &[u8]) -> String {
    let hash = HighwayHasher::new(Key(VERSION)).hash128(data);
    format!("{:0>16x}{:0>16x}", hash[0], hash[1])
}
