//! The TechEmpower fortunes test from `main_pg.rs`, reading SQLite instead of Postgres.

mod common;
mod server;

use std::{borrow::Cow, cell::RefCell, env};

use axum::{response::IntoResponse, routing::get, Router};
use dotenv::dotenv;
use rusqlite::{Connection, OpenFlags};
use yarte::Template;

use common::{get_env, utils::Utf8Html, SELECT_ALL_FORTUNES};

pub struct Fortune {
    pub id: i32,
    pub message: Cow<'static, str>,
}

#[derive(Template)]
#[template(path = "fortunes.html.hbs")]
pub struct FortunesTemplate<'a> {
    pub fortunes: &'a Vec<Fortune>,
}

thread_local! {
    // A connection is not `Sync`, and every server thread runs its own current-thread
    // runtime (`server::start_tokio`), so each thread opens its own connection.
    static DB: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

fn fetch_all_fortunes() -> rusqlite::Result<Vec<Fortune>> {
    DB.with_borrow(|db| {
        let conn = db.as_ref().expect("database not opened on this thread");
        let mut statement = conn.prepare_cached(SELECT_ALL_FORTUNES)?;
        let mut fortunes = vec![Fortune {
            id: 0,
            message: Cow::Borrowed("Additional fortune added at request time."),
        }];
        let rows = statement.query_map([], |row| {
            Ok(Fortune {
                id: row.get(0)?,
                message: Cow::Owned(row.get(1)?),
            })
        })?;
        for row in rows {
            fortunes.push(row?);
        }

        fortunes.sort_by(|it, next| it.message.cmp(&next.message));
        Ok(fortunes)
    })
}

async fn fortunes() -> impl IntoResponse {
    let fortunes = fetch_all_fortunes().expect("error loading fortunes");

    Utf8Html(
        FortunesTemplate {
            fortunes: &fortunes,
        }
        .call()
        .expect("error rendering template"),
    )
}

fn main() {
    dotenv().ok();
    server::start_tokio(serve_app)
}

async fn serve_app() {
    let path: String = get_env("SQLITE_PATH");
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("cannot open sqlite database");
    DB.set(Some(conn));

    let app = Router::new().route("/fortunes", get(fortunes));

    server::serve_hyper(app, env::var("PORT").ok().and_then(|p| p.parse().ok())).await
}
