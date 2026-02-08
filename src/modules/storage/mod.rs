use std::env;
use std::fs;
use std::path::Path;

use sqlx::{PgPool, Pool, Postgres};
use thiserror::Error;

const DEFAULT_MIGRATIONS_PATH: &str = "migrations";

pub mod repo;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("DATABASE_URL is not set")]
    MissingDatabaseUrl,
    #[error("failed to connect to database: {0}")]
    Connection(#[from] sqlx::Error),
    #[error("failed to read migrations from '{path}': {source}")]
    MigrationsRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to apply migrations: {0}")]
    Migration(sqlx::Error),
}

#[derive(Clone)]
pub struct Storage {
    pool: PgPool,
}

impl Storage {
    pub async fn connect() -> Result<Self, StorageError> {
        let database_url = env::var("DATABASE_URL").map_err(|_| StorageError::MissingDatabaseUrl)?;
        let pool = PgPool::connect(&database_url).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    pub async fn apply_migrations(&self) -> Result<(), StorageError> {
        let path = env::var("MIGRATIONS_PATH").unwrap_or_else(|_| DEFAULT_MIGRATIONS_PATH.to_string());
        self.apply_migrations_from(Path::new(&path)).await
    }

    async fn apply_migrations_from(&self, path: &Path) -> Result<(), StorageError> {
        let mut entries: Vec<_> = fs::read_dir(path)
            .map_err(|source| StorageError::MigrationsRead {
                path: path.display().to_string(),
                source,
            })?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("sql"))
                    .unwrap_or(false)
            })
            .collect();

        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            let sql = fs::read_to_string(entry.path()).map_err(|source| StorageError::MigrationsRead {
                path: entry.path().display().to_string(),
                source,
            })?;

            for statement in split_sql_statements(&sql) {
                sqlx::query(statement)
                    .execute(&self.pool)
                    .await
                    .map_err(StorageError::Migration)?;
            }
        }

        Ok(())
    }
}

fn split_sql_statements(sql: &str) -> Vec<&str> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::split_sql_statements;

    #[test]
    fn splits_multiple_statements() {
        let sql = "CREATE TABLE a(id INT); INSERT INTO a VALUES (1);";
        let parts = split_sql_statements(sql);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "CREATE TABLE a(id INT)");
        assert_eq!(parts[1], "INSERT INTO a VALUES (1)");
    }

    #[test]
    fn skips_empty_segments() {
        let sql = ";;  \n  ;SELECT 1;  ;";
        let parts = split_sql_statements(sql);
        assert_eq!(parts, vec!["SELECT 1"]);
    }
}
