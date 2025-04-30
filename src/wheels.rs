#[derive(Debug)]
pub struct Wheel {
    pub meta: Metadata,
}

#[tracing::instrument(skip(archive))]
pub fn parse_wheel<R>(mut archive: zip::read::ZipArchive<R>) -> Result<Wheel, ()>
where
    R: std::io::Read + std::io::Seek,
{
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
pub struct Metadata {
    pub name: String,
    pub version: String,
}

pub fn parse_metadata(file: zip::read::ZipFile<'_>) -> Result<Metadata, ()> {
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
