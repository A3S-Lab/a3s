mod assets;
mod static_files;

pub(super) use assets::prepare_default_web_dir;
pub(super) use static_files::{api_only_fallback, serve_static};
