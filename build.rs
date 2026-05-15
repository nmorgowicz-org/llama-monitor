// build.rs — Auto-generate static asset registration.
// Scans static/ directory and generates:
//   src/gen/static_assets.rs  — include_str! constants for each file
//   src/gen/routes.rs         — warp route filters for each file
//
// This eliminates the error-prone manual 2-step registration process.
// Regenerated automatically on every `cargo build` when static/ changes.

use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let static_dir = "static";
    let out_dir = "src/gen";

    // Ensure output directory exists
    fs::create_dir_all(out_dir).expect("Failed to create src/gen/ directory");

    // Collect all static files (skip .DS_Store and directories)
    let mut files: Vec<(String, String, String)> = Vec::new();
    collect_files(Path::new(static_dir), static_dir, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate static_assets.rs
    generate_static_assets(&files, &format!("{}/static_assets.rs", out_dir));

    // Generate routes.rs
    generate_routes(&files, &format!("{}/routes.rs", out_dir));

    // Tell Cargo to rerun when any file in static/ changes
    mark_rerun(static_dir);
    println!("cargo:rerun-if-changed=build.rs");
}

fn mark_rerun(dir: &str) {
    for e in fs::read_dir(dir)
        .unwrap_or_else(|_| panic!("Failed to read: {}", dir))
        .flatten()
    {
        let path = e.path();
        if path.is_dir() {
            mark_rerun(path.to_string_lossy().as_ref());
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn collect_files(base: &Path, prefix: &str, files: &mut Vec<(String, String, String)>) {
    let entries =
        fs::read_dir(prefix).unwrap_or_else(|_| panic!("Failed to read directory: {}", prefix));
    for e in entries.flatten() {
        let path = e.path();
        if path.is_file() {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            // Skip macOS metadata and hidden files
            if filename.starts_with(".DS_Store") || filename.starts_with('.') {
                continue;
            }
            // Get relative path from base directory (normalize to forward slashes)
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .to_string()
                .replace('\\', "/");
            // Generate a Rust-compatible constant name
            let const_name = path_to_const(&relative);
            // Determine content type category (only js/css/html have dedicated reply functions)
            let category = if relative.ends_with(".js") {
                "js"
            } else if relative.ends_with(".css") {
                "css"
            } else if relative.ends_with(".html") {
                "html"
            } else {
                "other"
            };
            files.push((relative, const_name, category.to_string()));
        } else if path.is_dir() {
            // Skip hidden directories
            let dirname = path.file_name().unwrap().to_string_lossy().to_string();
            if !dirname.starts_with('.') {
                let sub_prefix = path.to_string_lossy().to_string();
                collect_files(base, &sub_prefix, files);
            }
        }
    }
}

/// Convert a file path to a Rust constant name matching existing naming convention.
/// e.g., "js/bootstrap.js" → "BOOTSTRAP_JS"
///      "js/features/nav.js" → "FEATURES_NAV_JS"
///      "css/tokens.css" → "CSS_TOKENS"
///      "css/cards-inference.css" → "CSS_CARDS_INFERENCE"
///      "js/compat/globals.js" → "COMPAT_GLOBALS_JS"
///      "index.html" → "INDEX_HTML"
///      "compact.html" → "COMPACT_HTML"
///      "manifest.json" → "MANIFEST_JSON"
///      "icon.svg" → "ICON_SVG"
fn path_to_const(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();

    // CSS files: CSS_ + filename stem (no extension)
    if path.starts_with("css/") {
        let filename = parts[parts.len() - 1];
        let stem = filename.strip_suffix(".css").unwrap();
        return format!("CSS_{}", stem.replace('-', "_").to_uppercase());
    }

    // JS files: skip "js" prefix, join remaining parts with underscore, replace dot and hyphen
    if path.starts_with("js/") {
        let js_parts: Vec<&str> = parts.iter().skip(1).cloned().collect();
        let joined = js_parts.join("_");
        let result = joined.replace(['.', '-'], "_");
        return result.to_uppercase();
    }

    // Root-level files: filename with dot and hyphen replaced by underscore
    let filename = parts[parts.len() - 1];
    let result = filename.replace(['.', '-'], "_");
    result.to_uppercase()
}

/// Convert a file path to a URL path (for warp routes).
/// e.g., "js/features/nav.js" → "js/features/nav.js"
///      "index.html" → ""
fn path_to_url(path: &str) -> String {
    if path == "index.html" {
        return "".to_string();
    }
    path.to_string()
}

/// Generate src/gen/static_assets.rs
fn generate_static_assets(files: &[(String, String, String)], output: &str) {
    let mut f = fs::File::create(output).expect("Failed to create static_assets.rs");

    writeln!(f, "// AUTO-GENERATED by build.rs — do not edit manually").unwrap();
    writeln!(f, "// Regenerate with: cargo build\n").unwrap();
    writeln!(
        f,
        "//! Re-exported static asset constants for embedding at compile time.\n"
    )
    .unwrap();

    for (relative, const_name, _category) in files {
        writeln!(
            f,
            "pub const {}: &str = include_str!(\"../../static/{}\");",
            const_name, relative
        )
        .unwrap();
    }

    println!("Generated {} with {} constants", output, files.len());
}

/// Generate src/gen/routes.rs
fn generate_routes(files: &[(String, String, String)], output: &str) {
    let mut f = fs::File::create(output).expect("Failed to create routes.rs");

    writeln!(f, "// AUTO-GENERATED by build.rs — do not edit manually").unwrap();
    writeln!(f, "// Regenerate with: cargo build\n").unwrap();
    writeln!(f, "//! Warp route filters for all static assets.\n").unwrap();
    writeln!(f, "use super::static_assets;").unwrap();
    writeln!(f, "use warp::Filter;").unwrap();
    writeln!(f).unwrap();
    writeln!(
        f,
        "/// Returns a warp Filter that serves all static assets."
    )
    .unwrap();
    writeln!(f, "pub fn static_routes() -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {{").unwrap();
    writeln!(
        f,
        "    // Helper: serve static JS with no-cache (force browser to reload on every request)"
    )
    .unwrap();
    writeln!(
        f,
        "    fn js_reply(content: &'static str) -> impl warp::Reply {{"
    )
    .unwrap();
    writeln!(f, "        warp::reply::with_header(").unwrap();
    writeln!(f, "            warp::reply::with_header(content, \"content-type\", \"application/javascript\"),").unwrap();
    writeln!(f, "            \"cache-control\",").unwrap();
    writeln!(f, "            \"no-cache, no-store, must-revalidate\",").unwrap();
    writeln!(f, "        )").unwrap();
    writeln!(f, "    }}\n").unwrap();
    writeln!(f, "    // Helper: serve static CSS with cache headers").unwrap();
    writeln!(
        f,
        "    fn css_reply(content: &'static str) -> impl warp::Reply {{"
    )
    .unwrap();
    writeln!(f, "        warp::reply::with_header(").unwrap();
    writeln!(
        f,
        "            warp::reply::with_header(content, \"content-type\", \"text/css\"),"
    )
    .unwrap();
    writeln!(f, "            \"cache-control\",").unwrap();
    writeln!(f, "            \"max-age=3600\",").unwrap();
    writeln!(f, "        )").unwrap();
    writeln!(f, "    }}\n").unwrap();
    writeln!(f, "    // Helper: serve HTML").unwrap();
    writeln!(
        f,
        "    fn html_reply(content: &'static str) -> impl warp::Reply {{"
    )
    .unwrap();
    writeln!(f, "        warp::reply::with_header(").unwrap();
    writeln!(
        f,
        "            warp::reply::with_header(content, \"content-type\", \"text/html\"),"
    )
    .unwrap();
    writeln!(f, "            \"cache-control\",").unwrap();
    writeln!(f, "            \"no-cache, no-store, must-revalidate\",").unwrap();
    writeln!(f, "        )").unwrap();
    writeln!(f, "    }}\n").unwrap();
    writeln!(f, "    // Helper: serve other content types").unwrap();
    writeln!(
        f,
        "    fn other_reply(content: &'static str, content_type: &str) -> impl warp::Reply {{"
    )
    .unwrap();
    writeln!(
        f,
        "        warp::reply::with_header(content, \"content-type\", content_type)"
    )
    .unwrap();
    writeln!(f, "    }}\n").unwrap();

    // Generate individual route variables (skip index.html - handled specially in mod.rs)
    let route_files: Vec<_> = files.iter().filter(|(r, _, _)| r != "index.html").collect();
    for (i, (relative, const_name, category)) in route_files.iter().enumerate() {
        let url_path = path_to_url(relative);
        let var_name = format!("route_{}", i);

        // Build warp::path chain from URL segments
        let segments: Vec<&str> = url_path.split('/').collect();
        let reply_fn = match category.as_str() {
            "js" => "js_reply",
            "css" => "css_reply",
            "html" => "html_reply",
            _ => "other_reply",
        };

        writeln!(f, "    let {} = warp::path({:?})", var_name, segments[0]).unwrap();
        for segment in &segments[1..] {
            writeln!(f, "        .and(warp::path({:?}))", segment).unwrap();
        }
        writeln!(f, "        .and(warp::get())").unwrap();

        if category == "other" {
            let content_type = content_type_for(relative);
            writeln!(
                f,
                "        .map(|| other_reply(static_assets::{}, {:?}));",
                const_name, content_type
            )
            .unwrap();
        } else {
            writeln!(
                f,
                "        .map(|| {}(static_assets::{}));",
                reply_fn, const_name
            )
            .unwrap();
        }
    }

    // Chain all routes with .or() using a balanced tree structure to avoid compiler overflow
    writeln!(f).unwrap();
    writeln!(f, "    // Chain all routes using balanced tree structure").unwrap();

    // Group routes into chunks and chain them hierarchically
    let total_routes = route_files.len();
    if total_routes == 0 {
        writeln!(
            f,
            "    warp::path({:?}).and(warp::get()).map(|| panic!(\"No routes\"))",
            "placeholder"
        )
        .unwrap();
    } else if total_routes == 1 {
        writeln!(f, "    route_0").unwrap();
    } else {
        // Create a balanced binary tree of .or() chains
        let mut current_level: Vec<String> = route_files
            .iter()
            .enumerate()
            .map(|(i, _)| format!("route_{}", i))
            .collect();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                if chunk.len() == 1 {
                    next_level.push(chunk[0].clone());
                } else {
                    next_level.push(format!("{} .or({})", chunk[0], chunk[1]));
                }
            }
            current_level = next_level;
        }
        writeln!(f, "    {}", current_level[0]).unwrap();
    }
    writeln!(f, "}}").unwrap();

    println!("Generated {} with {} routes", output, files.len());
}

fn content_type_for(path: &str) -> &'static str {
    if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}
