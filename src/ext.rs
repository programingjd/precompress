use std::collections::BTreeSet;
use std::sync::LazyLock;

pub static EXTENSIONS: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    BTreeSet::from_iter([
        "html", "htm", "css", "js", "mjs", "cjs", "wasm", "json", "map", "ts", "geojson", "kml",
        "gpx", "csv", "tsv", "txt", "md", "adoc", "glsl", "xml", "xsd", "xslt", "dtd", "manifest",
        "pbf", "pdf", "svg", "ico", "jsonld", "gltf", "glb", "atom",
    ])
});
