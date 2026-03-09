use std::path::{Path, PathBuf};

const DB_FILENAME: &str = "chatdb.sqlite";
const DB_PATH_ENV: &str = "CHATDB_DB_PATH";

pub fn resolve_chatdb_db_path() -> PathBuf {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let env_override = std::env::var(DB_PATH_ENV).ok();
    resolve_chatdb_db_path_from(&current_dir, env_override.as_deref())
}

fn resolve_chatdb_db_path_from(current_dir: &Path, env_override: Option<&str>) -> PathBuf {
    if let Some(path) = env_override.map(str::trim).filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }

    if current_dir
        .file_name()
        .is_some_and(|name| name == "src-tauri")
    {
        if let Some(parent) = current_dir.parent() {
            if parent.join("package.json").exists() {
                return parent.join(DB_FILENAME);
            }
        }
    }

    current_dir.join(DB_FILENAME)
}

#[cfg(test)]
mod tests {
    use super::resolve_chatdb_db_path_from;
    use std::fs;
    use std::path::PathBuf;

    fn unique_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chatdb-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn env_override_wins() {
        let cwd = PathBuf::from(r"C:\Users\Vario\Desktop\chatdb\src-tauri");
        let resolved = resolve_chatdb_db_path_from(&cwd, Some(r"D:\data\chatdb.sqlite"));
        assert_eq!(resolved, PathBuf::from(r"D:\data\chatdb.sqlite"));
    }

    #[test]
    fn tauri_dev_cwd_uses_repo_root_database() {
        let root = unique_temp_dir("runtime-paths-root");
        fs::write(root.join("package.json"), "{}").expect("package.json should be written");
        let tauri_dir = root.join("src-tauri");
        fs::create_dir_all(&tauri_dir).expect("src-tauri dir should be created");

        let resolved = resolve_chatdb_db_path_from(&tauri_dir, None);
        assert_eq!(resolved, root.join("chatdb.sqlite"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ordinary_cwd_keeps_local_database_file() {
        let cwd = PathBuf::from(r"C:\Users\Vario\Desktop\chatdb");
        let resolved = resolve_chatdb_db_path_from(&cwd, None);
        assert_eq!(resolved, cwd.join("chatdb.sqlite"));
    }
}
