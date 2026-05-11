use std::path::Path;

use crate::models::ExecutionResult;

/// Execute a .sql file against a PostgreSQL database.
/// Connection info is passed via `args` JSON: {"host", "port", "dbname", "user", "password", "sslmode"}
/// The SQL content is read from the file.
pub async fn execute(file_path: &Path, args: &str) -> ExecutionResult {
    let sql = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            return ExecutionResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Failed to read SQL file: {e}"),
                result: None,
                duration_ms: 0,
            };
        }
    };

    let config: PgArgs = match serde_json::from_str(args) {
        Ok(c) => c,
        Err(e) => {
            return ExecutionResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!(
                    "Invalid PostgreSQL connection args (need: host, dbname, user, password): {e}"
                ),
                result: None,
                duration_ms: 0,
            };
        }
    };

    let conn_str = format!(
        "host={} port={} dbname={} user={} password={} sslmode={}",
        config.host,
        config.port.unwrap_or(5432),
        config.dbname,
        config.user,
        config.password.as_deref().unwrap_or(""),
        config.sslmode.as_deref().unwrap_or("prefer"),
    );

    let tls_connector = match native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ExecutionResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("TLS error: {e}"),
                result: None,
                duration_ms: 0,
            };
        }
    };

    let pg_tls = postgres_native_tls::MakeTlsConnector::new(tls_connector);

    let (client, connection) = match tokio_postgres::connect(&conn_str, pg_tls).await {
        Ok(pair) => pair,
        Err(e) => {
            return ExecutionResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("PostgreSQL connection failed: {e}"),
                result: None,
                duration_ms: 0,
            };
        }
    };

    let conn_handle = tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("PG connection error: {e}");
        }
    });

    let result = match client.query(&sql as &str, &[]).await {
        Ok(rows) => {
            let mut json_rows = Vec::new();
            for row in &rows {
                let mut obj = serde_json::Map::new();
                for (i, col) in row.columns().iter().enumerate() {
                    obj.insert(col.name().to_string(), pg_value_to_json(row, i));
                }
                json_rows.push(serde_json::Value::Object(obj));
            }
            let result_json = serde_json::to_string_pretty(&json_rows).unwrap_or_default();
            ExecutionResult {
                exit_code: 0,
                stdout: format!("{} row(s) returned\n{result_json}", json_rows.len()),
                stderr: String::new(),
                result: Some(serde_json::to_string(&json_rows).unwrap_or_default()),
                duration_ms: 0,
            }
        }
        Err(e) => ExecutionResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("Query failed: {e}"),
            result: None,
            duration_ms: 0,
        },
    };

    drop(client);
    conn_handle.abort();
    result
}

#[derive(serde::Deserialize)]
struct PgArgs {
    host: String,
    port: Option<u16>,
    dbname: String,
    user: String,
    password: Option<String>,
    sslmode: Option<String>,
}

fn pg_value_to_json(row: &tokio_postgres::Row, idx: usize) -> serde_json::Value {
    if let Ok(v) = row.try_get::<_, Option<bool>>(idx) {
        return v.map_or(serde_json::Value::Null, serde_json::Value::Bool);
    }
    if let Ok(v) = row.try_get::<_, Option<i32>>(idx) {
        return v.map_or(serde_json::Value::Null, |n| serde_json::json!(n));
    }
    if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
        return v.map_or(serde_json::Value::Null, |n| serde_json::json!(n));
    }
    if let Ok(v) = row.try_get::<_, Option<f64>>(idx) {
        return v.map_or(serde_json::Value::Null, |n| serde_json::json!(n));
    }
    if let Ok(v) = row.try_get::<_, Option<String>>(idx) {
        return v.map_or(serde_json::Value::Null, serde_json::Value::String);
    }
    if let Ok(v) = row.try_get::<_, Option<serde_json::Value>>(idx) {
        return v.unwrap_or(serde_json::Value::Null);
    }
    serde_json::Value::Null
}
