use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

pub mod api;
mod wheels;

pub struct State {
    pub normalized_project_names: HashMap<String, String>,
    pub projects: HashMap<String, Project>,
}

pub struct Project {
    pub files: Vec<std::path::PathBuf>,
}

pub struct Config {
    pub base: std::path::PathBuf,
}

#[derive(Debug)]
pub struct UpdateTrigger {
    pub source: std::borrow::Cow<'static, str>,
}

pub fn update(
    config: Config,
    state: Arc<RwLock<State>>,
    mut trigger: tokio::sync::mpsc::UnboundedReceiver<UpdateTrigger>,
) {
    loop {
        let trigger = match trigger.blocking_recv() {
            Some(t) => t,
            None => {
                tracing::info!("Triggers have been closed");
                return;
            }
        };

        let _entered = tracing::info_span!("triggered", ?trigger).entered();

        tracing::info!("Starting update");

        match update_inner(&config.base) {
            Ok(files) => {
                let mut guard = state.blocking_write();

                // Clear everything for now
                guard.projects.clear();

                for (file, wheel) in files.iter() {
                    tracing::info!("{:?} => {:?}", file, wheel);

                    let project = guard
                        .projects
                        .entry(wheel.meta.name.clone())
                        .or_insert_with(|| Project { files: Vec::new() });
                    if !project.files.contains(&file) {
                        project.files.push(file.clone());
                    }

                    let normalized_name = wheel.meta.normalized_name();

                    guard
                        .normalized_project_names
                        .insert(normalized_name, wheel.meta.name.clone());
                }
            }
            Err(e) => {
                tracing::error!("Update Inner: {:?}", e);
            }
        };

        tracing::info!("Done updating");
    }
}

#[tracing::instrument()]
fn update_inner(base: &std::path::Path) -> Result<Vec<(std::path::PathBuf, wheels::Wheel)>, ()> {
    let read_dir = std::fs::read_dir(base).map_err(|e| ())?;

    let mut results = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                continue;
            }
        };

        match entry.file_type() {
            Ok(ty) if ty.is_file() => {}
            _ => continue,
        };

        let path = entry.path();
        if path.extension().map(|ext| ext != "whl").unwrap_or(true) {
            tracing::warn!("Skipping file");
            continue;
        }

        let file_reader = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
        let archive = zip::read::ZipArchive::new(file_reader).unwrap();

        match wheels::parse_wheel(archive) {
            Ok(wheel) => {
                results.push((path, wheel));
            }
            Err(e) => {
                tracing::error!("Parsing wheel: {:?}", e);
            }
        };
    }

    Ok(results)
}
