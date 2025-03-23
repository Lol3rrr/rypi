use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug, clap::Parser)]
struct CliArgs {
    package_folder: std::path::PathBuf,
}

fn main() {
    let registry =
        tracing_subscriber::Registry::default().with(tracing_subscriber::fmt::Layer::default());

    tracing::subscriber::set_global_default(registry).unwrap();

    tracing::info!("Starting up...");

    let args = <CliArgs as clap::Parser>::parse();

    tracing::info!("Parsed args: {:?}", args);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let state = std::sync::Arc::new(tokio::sync::RwLock::new(rypi::State {
        normalized_project_names: std::collections::HashMap::new(),
        projects: std::collections::HashMap::new(),
    }));

    runtime.spawn_blocking({
        let state = state.clone();
        let config = rypi::Config {
            base: std::path::PathBuf::from(args.package_folder.clone()),
        };

        move || rypi::update(config, state)
    });

    runtime.block_on(async move {
        let api_handle = tokio::spawn(async move {
            let app = axum::Router::new()
                .route("/", axum::routing::get(homepage))
                .nest("/simple/", rypi::api::simple_index())
                .with_state(state);

            let bind_addr = format!("0.0.0.0:8080");
            let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();

            axum::serve(listener, app).await.unwrap();
        });

        api_handle.await
    });
}

async fn homepage() -> axum::response::Html<&'static str> {
    axum::response::Html(
        "<!DOCTYPE html>
<html>
  <body>
    <a href=\"/simple/\">Simple</a>
  </body>
</html>",
    )
}
