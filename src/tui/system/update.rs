//! Self-update version check.

/// The latest published version from GitHub releases (stripped of the `v`), or
/// `None` if offline / the lookup fails. Short timeout so startup never hangs.
pub(crate) async fn check_latest_version() -> Option<String> {
    crate::update::fetch_latest_async().await
}
