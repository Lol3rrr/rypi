use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

pub mod api;

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

pub fn update(config: Config, state: Arc<RwLock<State>>) {
    loop {
        match update_inner(&config.base) {
            Ok(files) => {
                let mut guard = state.blocking_write();
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

        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}

#[tracing::instrument()]
fn update_inner(base: &std::path::Path) -> Result<Vec<(std::path::PathBuf, Wheel)>, ()> {
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
        match parse_wheel(&path) {
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

#[derive(Debug)]
struct Wheel {
    meta: Metadata,
}

#[tracing::instrument]
fn parse_wheel(path: &std::path::Path) -> Result<Wheel, ()> {
    if path.extension().map(|ext| ext != "whl").unwrap_or(true) {
        tracing::warn!("Skipping file");
        return Err(());
    }

    tracing::info!("Checking");

    let file_reader = std::io::BufReader::new(std::fs::File::open(path).unwrap());

    let mut archive = zip::read::ZipArchive::new(file_reader).unwrap();

    let dist_info_names: Vec<String> = archive
        .file_names()
        .filter(|name| name.contains(".dist-info"))
        .map(|f| f.to_owned())
        .collect();

    let mut metadata: Option<Metadata> = None;
    for name in dist_info_names.iter() {
        let _entered = tracing::info_span!("Dist-Info", ?name).entered();

        let dist_file = match archive.by_name(name) {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Getting by File: {:?}", e);
                continue;
            }
        };

        match name {
            n if n.ends_with("/METADATA") => {
                let meta = match parse_metadata(dist_file) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!("Parsing Metadata File: {:?}", e);
                        continue;
                    }
                };

                metadata = Some(meta);
            }
            _ => {}
        };
    }

    Ok(Wheel {
        meta: metadata.ok_or(())?,
    })
}

#[derive(Debug)]
struct Metadata {
    name: String,
    version: String,
}

fn parse_metadata(file: zip::read::ZipFile<'_>) -> Result<Metadata, ()> {
    use std::io::BufRead;

    let mut name: Option<String> = None;
    let mut version: Option<String> = None;

    let reader = std::io::BufReader::new(file);
    for line in reader.lines() {
        let line = line.map_err(|e| ())?;

        match line {
            line if line.starts_with("Name: ") => {
                let value = line.strip_prefix("Name: ").unwrap();
                name = Some(value.to_owned());
            }
            line if line.starts_with("Version: ") => {
                let value = line.strip_prefix("Version: ").unwrap();
                version = Some(value.to_owned());
            }
            _ => {}
        };
    }

    Ok(Metadata {
        name: name.ok_or(())?,
        version: version.ok_or(())?,
    })
}

impl Metadata {
    pub fn normalized_name(&self) -> String {
        self.name
            .chars()
            .map(|c| match c {
                s if s.is_ascii_alphanumeric() && s.is_ascii_lowercase() => s,
                l if l.is_ascii_alphanumeric() => l.to_ascii_lowercase(),
                _ => '_',
            })
            .collect()
    }
}
