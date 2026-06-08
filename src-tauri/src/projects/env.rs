use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default)]
pub struct EnvOverlay {
    pub vars: HashMap<String, String>,
    pub notice: Option<String>,
}

pub fn load_project_env(cwd: &Path) -> EnvOverlay {
    let path = cwd.join(".env");
    if !path.exists() {
        return EnvOverlay::default();
    }

    let mut overlay = EnvOverlay::default();
    match dotenvy::from_path_iter(&path) {
        Ok(iter) => {
            let mut malformed = 0usize;
            for item in iter {
                match item {
                    Ok((key, value)) => {
                        overlay.vars.insert(key, value);
                    }
                    Err(_) => malformed += 1,
                }
            }
            if malformed > 0 {
                overlay.notice = Some(format!(
                    ".env: ignored {malformed} malformed line{}",
                    if malformed == 1 { "" } else { "s" }
                ));
            }
        }
        Err(error) => {
            overlay.notice = Some(format!(".env: could not load ({error})"));
        }
    }

    overlay
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_env_file_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();

        let overlay = load_project_env(dir.path());

        assert!(overlay.vars.is_empty());
        assert!(overlay.notice.is_none());
    }

    #[test]
    fn env_file_values_are_loaded_for_overlay() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "PORT=4217\nNAME=portus\n").unwrap();

        let overlay = load_project_env(dir.path());

        assert_eq!(overlay.vars.get("PORT"), Some(&"4217".to_string()));
        assert_eq!(overlay.vars.get("NAME"), Some(&"portus".to_string()));
    }

    #[test]
    fn malformed_env_file_keeps_valid_pairs_and_returns_notice() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "GOOD=yes\nnot valid dotenv\n").unwrap();

        let overlay = load_project_env(dir.path());

        assert_eq!(overlay.vars.get("GOOD"), Some(&"yes".to_string()));
        assert!(overlay.notice.unwrap().contains(".env"));
    }
}
