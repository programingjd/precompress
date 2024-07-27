# precompress_static &nbsp;[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![crates.io Version](https://img.shields.io/crates/v/precompress_static.svg)](https://crates.io/crates/precompress_static) [![Documentation](https://docs.rs/precompress_static/badge.svg)](https://docs.rs/precompress_static) ![Rust 1.80](https://img.shields.io/badge/rustc-1.80-ab6000.svg)

Precompress all static web content from a directory with brotli at max compression (original files are kept).

`sync` and `async` (tokio) versions, both as a lib and as executables.

See [ext.rs](src/ext.rs) for the list of file extensions that are compressed.

