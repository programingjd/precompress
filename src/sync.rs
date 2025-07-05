use crate::ext::EXTENSIONS;
use crate::hash::hash;
use crate::op::Operation;
use brotli::enc::BrotliEncoderParams;
use brotli::{BrotliCompress, BrotliDecompress};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::collections::LinkedList;
use std::fs::{read, read_dir, write};
use std::io::{copy, sink};
use std::ops::Deref;
use std::path::PathBuf;

pub fn compress(
    dir: PathBuf,
    dry_run: bool,
    force: bool,
    threads: Option<u8>,
    callback: impl Fn(&PathBuf, Operation) + Sync,
) -> Result<(), String> {
    let extensions = EXTENSIONS.deref();
    let mut list = vec![];
    let mut stack = LinkedList::new();
    stack.push_front(dir);
    while let Some(ref path) = stack.pop_front() {
        let entries = read_dir(path).map_err(|e| {
            format!(
                "failed to read directory \"{}\": {e:?}",
                path.to_string_lossy()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                format!(
                    "failed to read directory \"{}\": {e:?}",
                    path.to_string_lossy()
                )
            })?;
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
                if let Some((_, extension)) = filename.rsplit_once('.')
                    && extensions.contains(extension)
                {
                    list.push((filename, entry.path()));
                }
            }
        }
    }
    if let Some(n) = threads {
        ThreadPoolBuilder::new()
            .num_threads(n as usize)
            .build_global()
            .map_err(|e| format!("failed to create thread pool: {e:?}"))?;
    }
    list.into_par_iter()
        .map(|it| {
            let (filename, path) = it;
            let uncompressed = read(&path).map_err(|e| {
                format!("failed to read file \"{}\": {e:?}", path.to_string_lossy())
            })?;
            let br_path = path.parent().unwrap().join(format!("{filename}.br"));
            let compressed = read(br_path.clone()).ok();
            let operation = if let Some(compressed) = compressed {
                if force {
                    Operation::Update
                } else {
                    let mut decompressed = Vec::with_capacity(uncompressed.len());
                    BrotliDecompress(&mut &*compressed, &mut decompressed).map_err(|e| {
                        format!(
                            "failed to decompress file \"{}\": {e:?}",
                            br_path.to_string_lossy()
                        )
                    })?;
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
                BrotliCompress(&mut &*uncompressed, &mut compressed, &params).map_err(|e| {
                    format!(
                        "failed to compress file \"{}\": {e:?}",
                        path.to_string_lossy()
                    )
                })?;
                if dry_run {
                    copy(&mut compressed.as_slice(), &mut sink()).map_err(|e| {
                        format!(
                            "failed to write file \"{}\": {e:?}",
                            br_path.to_string_lossy()
                        )
                    })?;
                } else {
                    write(&br_path, compressed).map_err(|e| {
                        format!(
                            "failed to write file \"{}\": {e:?}",
                            br_path.to_string_lossy()
                        )
                    })?;
                };
            }
            callback(&br_path, operation);
            Ok(())
        })
        .collect()
}
