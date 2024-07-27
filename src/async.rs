use crate::ext::EXTENSIONS;
use crate::hash::hash;
use crate::Operation;
use brotli::enc::BrotliEncoderParams;
use brotli::{BrotliCompress, BrotliDecompress};
use std::collections::LinkedList;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use tokio::fs::{read, read_dir, write};
use tokio::io::{copy, sink};
use tokio::runtime::Builder;
use tokio::task::{spawn_blocking, JoinSet};

pub fn compress(
    dir: PathBuf,
    dry_run: bool,
    force: bool,
    worker_threads: Option<u8>,
    blocking_threads: Option<u8>,
    callback: impl for<'a> Fn(&'a PathBuf, Operation) + Sync + Send + Copy + 'static,
) -> Result<(), String> {
    let mut builder = Builder::new_multi_thread();
    if let Some(n) = worker_threads {
        builder.worker_threads(n as usize);
    }
    if let Some(n) = blocking_threads {
        builder.max_blocking_threads(n as usize);
    }
    let rt = builder
        .build()
        .map_err(|e| format!("failed to create tokio runtime: {e:?}"))?;
    rt.block_on(compress_future(dir, dry_run, force, callback))
}

async fn compress_future(
    dir: PathBuf,
    dry_run: bool,
    force: bool,
    callback: impl for<'a> Fn(&'a PathBuf, Operation) + Sync + Send + Copy + 'static,
) -> Result<(), String> {
    let extensions = EXTENSIONS.deref();
    let mut list = vec![];
    let mut stack = LinkedList::new();
    stack.push_front(dir);
    while let Some(ref path) = stack.pop_front() {
        let mut entries = read_dir(path).await.map_err(|e| {
            format!(
                "failed to read directory \"{}\": {e:?}",
                path.to_string_lossy()
            )
        })?;
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            format!(
                "failed to read directory \"{}\": {e:?}",
                path.to_string_lossy()
            )
        })? {
            let filename = entry.file_name().into_string().map_err(|e| {
                format!(
                    "failed to read file name \"{}\": {e:?}",
                    entry.path().to_string_lossy()
                )
            })?;
            if filename.starts_with('.') {
                continue;
            }
            if entry
                .metadata()
                .await
                .map_err(|e| {
                    format!(
                        "failed to read metadata of \"{}\": {e:?}",
                        entry.path().to_string_lossy()
                    )
                })?
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
    let mut futures: JoinSet<Result<(), String>> = JoinSet::new();
    list.into_iter().for_each(|(filename, path)| {
        futures.spawn(fut(force, dry_run, filename, path, callback));
    });

    while let Some(future) = futures.join_next().await {
        let future = future.map_err(|e| format!("failed to run task: {e:?}"))?;
        future?;
    }
    Ok(())
}

async fn fut(
    force: bool,
    dry_run: bool,
    filename: String,
    path: PathBuf,
    callback: impl for<'a> Fn(&'a PathBuf, Operation) + Sync + Send + Copy + 'static,
) -> Result<(), String> {
    let uncompressed = read(&path)
        .await
        .map_err(|e| format!("failed to read file \"{}\": {e:?}", path.to_string_lossy()))?;
    let br_path = path.parent().unwrap().join(format!("{filename}.br"));
    let compressed = read(br_path.clone()).await.ok();
    let operation = if let Some(compressed) = compressed {
        if force {
            Operation::Update
        } else {
            let decompressed = Vec::with_capacity(uncompressed.len());
            let decompressed = brotli_decompress(&br_path, compressed, decompressed).await?;
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
        let compressed = Vec::with_capacity(uncompressed.len() + 64);
        let compressed = brotli_compress(&path, uncompressed, compressed).await?;
        if dry_run {
            copy(&mut compressed.as_slice(), &mut sink())
                .await
                .map_err(|e| {
                    format!(
                        "failed to write file \"{}\": {e:?}",
                        br_path.to_string_lossy()
                    )
                })?;
        } else {
            write(&br_path, compressed).await.map_err(|e| {
                format!(
                    "failed to write file \"{}\": {e:?}",
                    br_path.to_string_lossy()
                )
            })?;
        }
    }
    callback(&br_path, operation);
    Ok(())
}

async fn brotli_decompress(
    compressed_path: &Path,
    compressed: Vec<u8>,
    mut decompressed: Vec<u8>,
) -> Result<Vec<u8>, String> {
    spawn_blocking(move || {
        BrotliDecompress(&mut &*compressed, &mut decompressed).map(|_| decompressed)
    })
    .await
    .map_err(|e| format!("failed to run blocking task: {e:?}"))?
    .map_err(|e| {
        format!(
            "failed to decompress file \"{}\": {e:?}",
            compressed_path.to_string_lossy()
        )
    })
}

async fn brotli_compress(
    path: &Path,
    uncompressed: Vec<u8>,
    mut compressed: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let params = BrotliEncoderParams {
        quality: 11,
        size_hint: uncompressed.len(),
        ..BrotliEncoderParams::default()
    };
    spawn_blocking(move || {
        BrotliCompress(&mut &*uncompressed, &mut compressed, &params).map(|_| compressed)
    })
    .await
    .map_err(|e| format!("failed to run blocking task: {e:?}"))?
    .map_err(|e| {
        format!(
            "failed to compress file \"{}\": {e:?}",
            path.to_string_lossy()
        )
    })
}
