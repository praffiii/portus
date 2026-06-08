use std::path::Path;

use yaml_rust2::YamlLoader;

use super::Task;

/// Suggested tasks for a folder, read from package.json scripts and any
/// compose file. Best-effort: unreadable / malformed files contribute nothing.
pub fn tasks_from_folder(folder: &Path) -> Vec<Task> {
    let pm = detect_package_manager(folder);
    let mut out: Vec<(String, String)> = Vec::new();

    if let Ok(json) = std::fs::read_to_string(folder.join("package.json")) {
        out.extend(parse_package_scripts(&json, pm));
    }

    for name in [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yml",
        "docker-compose.yaml",
    ] {
        if let Ok(yaml) = std::fs::read_to_string(folder.join(name)) {
            for service in parse_compose_services(&yaml) {
                out.push((service.clone(), format!("docker compose up {service}")));
            }
            break;
        }
    }

    out.into_iter()
        .map(|(name, command)| Task {
            id: name.clone(),
            name,
            command,
        })
        .collect()
}

pub fn detect_package_manager(folder: &Path) -> &'static str {
    if folder.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if folder.join("yarn.lock").exists() {
        "yarn"
    } else {
        "npm"
    }
}

pub fn parse_package_scripts(json: &str, pm: &str) -> Vec<(String, String)> {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let Some(scripts) = value.get("scripts").and_then(|s| s.as_object()) else {
        return Vec::new();
    };
    scripts
        .keys()
        .map(|name| (name.clone(), format!("{pm} run {name}")))
        .collect()
}

pub fn parse_compose_services(yaml: &str) -> Vec<String> {
    let docs = match YamlLoader::load_from_str(yaml) {
        Ok(docs) => docs,
        Err(_) => return Vec::new(),
    };
    let Some(doc) = docs.first() else {
        return Vec::new();
    };
    let services = &doc["services"];
    match services.as_hash() {
        Some(hash) => hash
            .keys()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_json_scripts_into_tasks() {
        let json = r#"{ "scripts": { "dev": "vite", "build": "vite build" } }"#;
        let mut tasks = parse_package_scripts(json, "npm");
        tasks.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            tasks,
            vec![
                ("build".to_string(), "npm run build".to_string()),
                ("dev".to_string(), "npm run dev".to_string()),
            ]
        );
    }

    #[test]
    fn package_json_without_scripts_yields_nothing() {
        assert!(parse_package_scripts(r#"{ "name": "x" }"#, "npm").is_empty());
    }

    #[test]
    fn malformed_package_json_yields_nothing() {
        assert!(parse_package_scripts("{ not json", "npm").is_empty());
    }

    #[test]
    fn parses_compose_service_names_with_comments() {
        let yaml =
            "# top comment\nservices:\n  db:\n    image: postgres\n  redis:\n    image: redis\n";
        let mut services = parse_compose_services(yaml);
        services.sort();
        assert_eq!(services, vec!["db".to_string(), "redis".to_string()]);
    }

    #[test]
    fn compose_without_services_yields_nothing() {
        assert!(parse_compose_services("version: '3'\n").is_empty());
    }
}
