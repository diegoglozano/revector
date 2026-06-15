//! Scaffolding for `revector new` and `revector init`.

use std::path::{Path, PathBuf};

use time::OffsetDateTime;

use crate::chain::Chain;
use crate::error::Result;
use crate::migration::discover;

/// Slugify a free-text migration name into a filename-safe fragment.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('_');
            prev_dash = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Create the migrations directory and a starter `revector.toml` if missing.
/// Returns the list of paths that were created.
pub fn init(migrations_dir: &Path, config_path: &Path) -> Result<Vec<PathBuf>> {
    let mut created = Vec::new();
    if !migrations_dir.exists() {
        std::fs::create_dir_all(migrations_dir)?;
        created.push(migrations_dir.to_path_buf());
    }
    if !config_path.exists() {
        let body = format!(
            "# revector configuration\n\
             url = \"http://localhost:6334\"\n\
             migrations_dir = \"{}\"\n\
             # api_key = \"...\"            # or set REVECTOR_API_KEY\n\
             # tracking_collection = \"_revector_migrations\"\n",
            migrations_dir.display()
        );
        std::fs::write(config_path, body)?;
        created.push(config_path.to_path_buf());
    }
    Ok(created)
}

/// Create a new, empty migration file chained onto the current head.
///
/// The revision id is `<timestamp>_<slug>`; `down_revision` is set to the
/// existing chain head so the new file extends the chain.
pub fn new_migration(migrations_dir: &Path, name: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(migrations_dir)?;

    let chain = Chain::resolve(discover(migrations_dir)?)?;
    let down_revision = chain.head().map(str::to_string);

    let ts = OffsetDateTime::now_utc().unix_timestamp();
    let slug = slugify(name);
    let revision = format!("{ts}_{slug}");
    let filename = format!("{revision}.yaml");
    let path = migrations_dir.join(&filename);

    let down_line = match &down_revision {
        Some(d) => format!("down_revision: {d}"),
        None => "down_revision: null".to_string(),
    };

    let template = format!(
        "revision: {revision}\n\
         {down_line}\n\
         description: {name}\n\
         \n\
         # Operations applied on `revector up`.\n\
         up:\n\
         #  - op: create_collection\n\
         #    name: my_collection\n\
         #    spec:\n\
         #      vectors:\n\
         #        \"\":\n\
         #          size: 768\n\
         #          distance: Cosine\n\
         \n\
         # Optional. If omitted, revector auto-inverts the `up` ops on\n\
         # `revector down` (and refuses if any step is irreversible).\n\
         # down:\n\
         #  - op: delete_collection\n\
         #    name: my_collection\n"
    );

    std::fs::write(&path, template)?;
    Ok(path)
}
