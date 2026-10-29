//! The Database adapter's row-schema write guard re-reads `/version`
//! instead of trusting a cached answer forever.
//!
//! A scripted loopback server stands in for the library server so a test
//! can change its answer between writes and drop a connection.

#![cfg(feature = "database")]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::database::DatabaseAdapter;
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved, ROW_SCHEMA};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::lifecycle::LifecycleState;
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

enum Reply {
    Json(u16, String),
    /// Close the connection without answering.
    Drop,
}

/// Answers each request line (`"GET /version"`) from `script`, given how
/// many times that line was seen before (0-based).
struct Fake {
    url: String,
    seen: Arc<Mutex<HashMap<String, usize>>>,
}

impl Fake {
    fn start(script: impl Fn(&str, usize) -> Reply + Send + 'static) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen: Arc<Mutex<HashMap<String, usize>>> = Arc::default();
        let seen_in = Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let key = match request_line(&stream) {
                    Some(k) => k,
                    None => continue,
                };
                let n = {
                    let mut m = seen_in.lock().unwrap();
                    let c = m.entry(key.clone()).or_insert(0);
                    *c += 1;
                    *c - 1
                };
                respond(stream, script(&key, n));
            }
        });
        Fake { url, seen }
    }

    fn hits(&self, line: &str) -> usize {
        self.seen.lock().unwrap().get(line).copied().unwrap_or(0)
    }
}

/// `"METHOD /path"` (query stripped); the body is read and discarded.
fn request_line(mut stream: &TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).ok()?;
        buf.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let length = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().ok())
        })
        .flatten()
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).ok()?;
    let mut parts = head.lines().next()?.split(' ');
    let method = parts.next()?;
    let path = parts.next()?.split('?').next()?;
    Some(format!("{method} {path}"))
}

fn respond(mut stream: TcpStream, reply: Reply) {
    if let Reply::Json(status, body) = reply {
        let text = format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(text.as_bytes()).ok();
    }
}

fn row() -> ComponentRow {
    let lib = Uuid::now_v7();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R1"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url(""),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "R1"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
        server_revision: Some(1),
    }
}

fn version_body(schema: u32) -> String {
    format!(r#"{{"name":"signex-library-server","row_schema":{schema}}}"#)
}

fn stored(row: &ComponentRow) -> String {
    let mut r = row.clone();
    r.server_revision = Some(2);
    serde_json::to_string(&r).unwrap()
}

fn adapter(fake: &Fake) -> DatabaseAdapter {
    DatabaseAdapter::with_token(fake.url.clone(), "t", "holder").unwrap()
}

/// A server that reported an old schema is asked again on the next write,
/// so an upgrade unblocks writes without reopening the library.
#[test]
fn a_low_row_schema_is_not_cached_and_an_upgrade_unblocks_writes() {
    let row = row();
    let body = stored(&row);
    let fake = Fake::start(move |line, n| match line {
        "GET /version" if n == 0 => Reply::Json(200, version_body(ROW_SCHEMA - 1)),
        "GET /version" => Reply::Json(200, version_body(ROW_SCHEMA)),
        _ => Reply::Json(200, body.clone()),
    });
    let adapter = adapter(&fake);

    let err = adapter
        .update_row("resistors", row.clone(), "m")
        .unwrap_err();
    assert!(err.to_string().contains("too old"), "{err}");
    let put = format!("PUT /tables/resistors/rows/{}", row.row_id);
    assert_eq!(fake.hits(&put), 0, "a refused write sends nothing");

    adapter.update_row("resistors", row, "m").unwrap();
    assert_eq!(fake.hits("GET /version"), 2);
}

/// A good answer is cached, but a transport error on a write drops it so
/// the next write re-checks `/version`.
#[test]
fn a_cached_good_row_schema_is_dropped_after_a_transport_error() {
    let row = row();
    let body = stored(&row);
    let put = format!("PUT /tables/resistors/rows/{}", row.row_id);
    let put_in = put.clone();
    let fake = Fake::start(move |line, n| {
        if line == "GET /version" {
            Reply::Json(200, version_body(ROW_SCHEMA))
        } else if line == put_in && n == 1 {
            Reply::Drop
        } else {
            Reply::Json(200, body.clone())
        }
    });
    let adapter = adapter(&fake);

    adapter.update_row("resistors", row.clone(), "m").unwrap();
    adapter
        .update_row("resistors", row.clone(), "m")
        .unwrap_err();
    assert_eq!(fake.hits("GET /version"), 1, "a cached answer was reused");
    adapter.update_row("resistors", row, "m").unwrap();
    assert_eq!(
        fake.hits("GET /version"),
        2,
        "the transport error forced a re-check"
    );
}

/// A server answering a write with the schema-refusal code drops the cached
/// answer too, and the error tells the user to upgrade.
#[test]
fn a_schema_refusal_drops_the_cache_and_asks_for_an_upgrade() {
    let row = row();
    let put = format!("PUT /tables/resistors/rows/{}", row.row_id);
    let get = format!("GET /tables/resistors/rows/{}", row.row_id);
    let body = stored(&row);
    let fake = Fake::start(move |line, _| {
        if line == "GET /version" {
            Reply::Json(200, version_body(ROW_SCHEMA))
        } else if line == get {
            Reply::Json(200, body.clone())
        } else if line == put {
            Reply::Json(
                400,
                r#"{"error":"needs newer","code":"row_schema_too_old"}"#.into(),
            )
        } else {
            Reply::Json(200, "{}".into())
        }
    });
    let adapter = adapter(&fake);

    let err = adapter
        .update_row("resistors", row.clone(), "m")
        .unwrap_err();
    assert!(err.to_string().contains("upgrade signex"), "{err}");
    adapter.update_row("resistors", row, "m").unwrap_err();
    assert_eq!(fake.hits("GET /version"), 2);
}
