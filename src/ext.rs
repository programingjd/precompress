use std::collections::BTreeSet;
use std::sync::LazyLock;

pub static EXTENSIONS: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    BTreeSet::from_iter([
        "html", "htm", "css", "js", "mjs", "cjs", "map", "json", "xml", "ldjson", "txt", "csv",
        "tsv", "md", "adoc", "wasm", "ico", "svg", "pdf", "gpx", "atom", "kml", "geojson", "pbf",
        "gltf", "glb", "bin", "ts", "xsd", "xslt", "dtd", "manifest",
    ])
});
