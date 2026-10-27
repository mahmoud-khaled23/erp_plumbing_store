//! Test-only JSON-lines transport for Playwright. Never included in the desktop bundle.
use plumbing_erp::{
    error::{Error, Result},
    models::*,
    state::App,
};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

fn arg<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Result<T> {
    serde_json::from_value(v[key].clone()).map_err(Error::from)
}
fn dispatch(app: &App, command: &str, args: &Value) -> Result<Value> {
    match command {
        "erp_setup" => app.setup(arg(args, "input")?),
        "erp_login" => app.login(
            &arg::<String>(args, "username")?,
            &arg::<String>(args, "password")?,
        ),
        "erp_logout" => {
            app.logout()?;
            Ok(Value::Null)
        }
        "erp_query" => app.query(arg(args, "query")?),
        "erp_mutate" => app.mutate(arg(args, "request")?),
        "erp_change_password" => {
            app.change_password(&arg::<String>(args, "old")?, &arg::<String>(args, "new")?)?;
            Ok(Value::Null)
        }
        "erp_create_user" => app.create_user(
            &arg::<String>(args, "username")?,
            &arg::<String>(args, "name")?,
            &arg::<String>(args, "password")?,
            &arg::<Vec<Int>>(args, "roles")?,
        ),
        "erp_reset_password" => {
            app.reset_password(arg(args, "userId")?, &arg::<String>(args, "password")?)?;
            Ok(Value::Null)
        }
        _ => Err(Error::new(
            "test_transport",
            "هذه العملية تتطلب نافذة التطبيق المكتبي",
        )),
    }
}
fn main() {
    let dir = tempfile::tempdir().expect("temporary test directory");
    let app = App::open(dir.path().join("store")).expect("test store");
    let mut output = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };
        let request: Value = serde_json::from_str(&line).expect("test request JSON");
        let result = dispatch(
            &app,
            request["command"].as_str().unwrap_or(""),
            &request["args"],
        );
        let response = match result {
            Ok(value) => json!({"id":request["id"],"value":value}),
            Err(error) => json!({"id":request["id"],"error":error}),
        };
        writeln!(output, "{response}").expect("test output");
        output.flush().expect("test output flush");
    }
}
