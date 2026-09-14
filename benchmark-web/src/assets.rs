//! Templates and static assets compiled into the binary.
//!
//! The release archives ship binaries only — no `templates/` or `static/` directories —
//! so these assets have to travel inside the executable. Embedding them also means
//! `llm-benchmark web` works from any working directory, which is what the documented
//! "download the binary and run it" path assumes.
//!
//! `TEMPLATES_DIR` and `STATIC_DIR` still point at on-disk copies. That is what lets
//! template and CSS edits show up without a rebuild during development.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

/// Tera templates, registered under their bare file names (`dashboard.tera`).
///
/// Those are the names `Tera::new("templates/**/*.tera")` used to produce, and the
/// templates reference each other by them (`{% extends "layout.tera" %}`).
#[derive(Embed)]
#[folder = "templates"]
pub struct Templates;

/// Static files, served at the site root: `static/css/style.css` is `/css/style.css`.
#[derive(Embed)]
#[folder = "static"]
pub struct StaticAssets;

/// Serve an embedded static file, or 404.
///
/// Used as the router fallback, so an unmatched path is an asset lookup — the same
/// shape the server had when this was `ServeDir`.
pub async fn serve_static(uri: Uri) -> Response {
    // `Uri::path` always begins with `/`; embedded keys never do.
    let Some(file) = StaticAssets::get(uri.path().trim_start_matches('/')) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut response = Body::from(file.data.into_owned()).into_response();
    // `insert` rather than a header tuple: a bare byte body already carries
    // `application/octet-stream`, and a second Content-Type header is ambiguous.
    let content_type = HeaderValue::from_str(file.metadata.mimetype())
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::TemplateEngine;

    /// The set the server renders. Kept explicit so a template added to `templates/`
    /// but not embedded — or a typo in a name — fails here rather than at request time.
    const EXPECTED_TEMPLATES: [&str; 10] = [
        "compare.tera",
        "dashboard.tera",
        "exercise-detail.tera",
        "layout.tera",
        "result-detail.tera",
        "results.tera",
        "run.tera",
        "scoring.tera",
        "test.tera",
        "view_benchmark.tera",
    ];

    #[test]
    fn every_template_is_embedded_under_its_bare_name() {
        let embedded: Vec<String> = Templates::iter().map(|name| name.into_owned()).collect();

        for expected in EXPECTED_TEMPLATES {
            assert!(
                embedded.iter().any(|name| name == expected),
                "{expected} is not embedded; embedded: {embedded:?}"
            );
        }
        // `Tera::new("templates/**/*.tera")` named templates relative to `templates/`,
        // and the templates include each other by those names.
        assert!(
            embedded.iter().all(|name| !name.contains('/')),
            "template names must stay flat: {embedded:?}"
        );
    }

    /// Regression: the server used to load templates from disk, so on a machine with
    /// only the binary installed every page rendered "Template Rendering Error".
    #[test]
    fn the_embedded_templates_compile() {
        TemplateEngine::from_embedded().expect("the embedded template set must compile");
    }

    /// Regression: `static/css/style.css` must be servable with no `static/` on disk.
    #[tokio::test]
    async fn the_style_sheet_is_embedded_and_served_as_css() {
        let response = serve_static(Uri::from_static("/css/style.css")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .expect("a content type must be set"),
            "text/css"
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("reading the embedded stylesheet");
        assert!(!body.is_empty(), "the stylesheet must not be empty");
    }

    #[tokio::test]
    async fn an_unknown_static_path_is_a_404() {
        let response = serve_static(Uri::from_static("/css/does-not-exist.css")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
