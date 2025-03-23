use std::sync::Arc;

use tokio::sync::RwLock;

use super::State;

/// [PEP-503](https://peps.python.org/pep-0503/#specification)
pub fn simple_index() -> axum::Router<Arc<RwLock<State>>> {
    axum::Router::new()
        .route("/", axum::routing::get(root))
        .route("/{project}/", axum::routing::get(project))
        .route("/{project}/{file}", axum::routing::get(project_file))
}

#[tracing::instrument(skip(state))]
async fn root(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<State>>>,
) -> axum::response::Html<String> {
    let parts: String = {
        let projects = state.read().await;

        let parts: String = projects
            .normalized_project_names
            .iter()
            .map(|(normalized, unnormalized)| {
                format!("<a href=\"/simple/{normalized}/\">{unnormalized}</a><br/>")
            })
            .collect();

        parts
    };

    axum::response::Html(format!(
        "<!DOCTYPE html>
<html>
  <body>
    {parts}
  </body>
</html>"
    ))
}

#[tracing::instrument(skip(state))]
async fn project(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<State>>>,
    axum::extract::Path(normalized_project_name): axum::extract::Path<String>,
) -> axum::response::Html<String> {
    let parts: String = {
        let projects = state.read().await;

        let name = match projects
            .normalized_project_names
            .get(&normalized_project_name)
        {
            Some(n) => n,
            None => {
                tracing::error!("Unknown project");
                return axum::response::Html("AHHHH".into());
            }
        };

        let project = match projects.projects.get(name) {
            Some(p) => p,
            None => {
                tracing::error!("Unknown project");
                return axum::response::Html("AHHH".into());
            }
        };

        let parts: String = project
            .files
            .iter()
            .map(|path| {
                let name = path.file_name().unwrap().to_str().unwrap();

                format!("<a href=\"/simple/{normalized_project_name}/{name}\">{name}</a><br/>")
            })
            .collect();

        parts
    };

    axum::response::Html(format!(
        "<!DOCTYPE html>
<html>
  <body>
    {parts}
  </body>
</html>"
    ))
}

#[tracing::instrument(skip(state))]
async fn project_file(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<State>>>,
    axum::extract::Path((project, file)): axum::extract::Path<(String, String)>,
) -> Result<impl axum::response::IntoResponse, ()> {
    tracing::info!("Download");

    let path = {
        let guard = state.read().await;

        let normalized_name = match guard.normalized_project_names.get(&project) {
            Some(n) => n,
            None => {
                tracing::error!("Getting normalized_project_name");
                return Err(());
            }
        };

        let project = match guard.projects.get(normalized_name) {
            Some(p) => p,
            None => {
                tracing::error!("Getting Project");
                return Err(());
            }
        };

        let path = match project
            .files
            .iter()
            .find(|path| path.file_name().unwrap().to_str().unwrap() == file)
        {
            Some(p) => p,
            None => {
                tracing::error!("Unknown File");
                return Err(());
            }
        };

        path.clone()
    };

    tracing::info!("Downloading file {:?}", path);

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|err| ())
        .unwrap();
    let stream = tokio_util::io::ReaderStream::with_capacity(file, 1024);
    let stream_body = axum::body::Body::from_stream(stream);

    Ok(axum::response::Response::builder()
        .header("Content-Type", "application/binary")
        .body(stream_body)
        .unwrap())
}
