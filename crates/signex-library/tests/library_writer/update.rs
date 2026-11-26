//! `LibraryWriter::write_update`: overwrite a converter's earlier output
//! (contract §5).

use chrono::{TimeZone, Utc};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::library_file::LibraryFile;
use signex_library::lifecycle::LifecycleState;
use signex_library::writer::LibraryWriter;

use crate::read_tree;
use crate::synthetic::{self, row_id};

fn row_of(adapter: &LocalGitAdapter, n: usize) -> Option<signex_library::component::ComponentRow> {
    adapter
        .iter_rows()
        .unwrap()
        .into_iter()
        .map(|(_, r)| r)
        .find(|r| r.row_id == row_id(n))
}

#[test]
fn an_update_with_the_same_input_changes_no_byte() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    synthetic::library(120).write(&root).unwrap();
    let before = read_tree(&root);
    let summary = synthetic::library(120).write_update(&root).unwrap();
    assert_eq!(read_tree(&root), before);
    assert_eq!(summary.rows_unchanged, 120);
    assert_eq!(summary.files_written + summary.files_deleted, 0);
}

#[test]
fn an_update_bumps_changed_rows_keeps_state_and_drops_removed_items() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let snxlib = synthetic::library(100).write(&root).unwrap();
    // The library's own edit of a lifecycle state, which an update keeps.
    let text = std::fs::read_to_string(&snxlib).unwrap();
    let id = row_id(7).to_string();
    let edited: String = text
        .lines()
        .map(|l| {
            if l.starts_with(&id) {
                l.replacen("\tReleased\t", "\tObsolete\t", 1)
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&snxlib, &edited).unwrap();
    let stored_r3 = text
        .lines()
        .find(|l| l.starts_with(&row_id(3).to_string()))
        .unwrap()
        .to_string();

    // Re-convert: 90 rows and 9 footprints (10 rows and footprint 9 gone),
    // row 7 changed upstream.
    let mut w = LibraryWriter::new("Synthetic", synthetic::manifest("Synthetic"));
    let later = Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
    w.set_update_time(later);
    for n in 0..90 {
        let mut r = synthetic::row(n, 2, 10);
        if n == 7 {
            r.description = "Changed upstream".into();
        }
        w.add_row(synthetic::table_of(n), r);
    }
    w.add_symbol_file(synthetic::envelope(
        0,
        (0..2).map(synthetic::symbol).collect(),
    ));
    for n in 0..9 {
        w.add_footprint_file(synthetic::footprint(n)).unwrap();
    }
    let summary = w.write_update(&root).unwrap();
    assert_eq!(summary.rows_removed, 10);
    assert_eq!(summary.rows_changed, 1);
    assert_eq!(summary.rows_unchanged, 89);
    assert_eq!(summary.files_deleted, 1, "footprint 9 is gone");

    let adapter = LocalGitAdapter::open(&snxlib).unwrap();
    assert!(row_of(&adapter, 95).is_none());
    let r7 = row_of(&adapter, 7).unwrap();
    assert_eq!(r7.description, "Changed upstream");
    assert_eq!(r7.state, LifecycleState::Obsolete, "lifecycle state kept");
    assert_eq!(r7.version, "1.0.1");
    assert_eq!(r7.created, synthetic::time());
    assert_eq!(r7.updated, later);
    assert_eq!(
        r7.content_hash,
        signex_library::hash_row_content(&r7).unwrap()
    );
    let after = std::fs::read_to_string(&snxlib).unwrap();
    assert!(
        after.contains(&format!("\n{stored_r3}\n")),
        "an unchanged row keeps its bytes"
    );
    assert!(
        adapter
            .get_footprint(synthetic::footprint(9).footprints[0].uuid)
            .is_err()
    );
    LibraryFile::parse(&after).unwrap();
}

#[test]
fn an_update_of_another_library_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    synthetic::library(10).write(&root).unwrap();
    let before = read_tree(&root);
    let mut other = synthetic::manifest("Synthetic");
    other.library_id = uuid::Uuid::from_u128(42);
    assert!(
        LibraryWriter::new("Synthetic", other)
            .write_update(&root)
            .is_err()
    );
    assert_eq!(read_tree(&root), before);
}

/// The update holds the library's write lock: while another holder has it,
/// the update waits, then refuses and changes nothing.
#[test]
fn an_update_waits_for_the_library_write_lock() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let snxlib = synthetic::library(10).write(&root).unwrap();
    let before = read_tree(&root);
    let lock_path = signex_library::write_lock_path(&snxlib);
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .unwrap();
    lock.lock().unwrap();
    let mut changed = synthetic::library(10);
    changed.add_row("ics", synthetic::row(10, 1, 1));
    let err = changed.write_update(&root).unwrap_err().to_string();
    assert!(err.contains("another Signex process"), "{err}");
    drop(lock);
    let mut after = read_tree(&root);
    after.retain(|rel, _| !rel.contains("writelock"));
    assert_eq!(after, before);
    assert_eq!(changed.write_update(&root).unwrap().rows_added, 1);
}

#[test]
fn surviving_rows_keep_their_cached_prices_unless_the_writer_gives_its_own() {
    use signex_library::distributor::PriceBreak;
    use signex_library::pricing::CachedPricing;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let price = |sku: &str, unit_price: f64| CachedPricing {
        sku: sku.into(),
        currency: "EUR".into(),
        stock: None,
        breaks: vec![PriceBreak { qty: 1, unit_price }],
        fetched_at: synthetic::time(),
    };
    let priced = |n: usize, unit_price: f64| {
        let sku = format!("ED-{n}");
        std::collections::BTreeMap::from([(
            "ExampleDistributor".to_string(),
            std::collections::BTreeMap::from([(sku.clone(), price(&sku, unit_price))]),
        )])
    };
    let mut first = synthetic::library(10);
    // Rows 0 and 5 are in table "capacitors".
    let table = first.table_mut(synthetic::table_of(0));
    table.pricing.insert(row_id(0).to_string(), priced(0, 0.5));
    table.pricing.insert(row_id(5).to_string(), priced(5, 0.7));
    first.write(&root).unwrap();

    let mut again = synthetic::library(10);
    again
        .table_mut(synthetic::table_of(5))
        .pricing
        .insert(row_id(5).to_string(), priced(5, 0.9));
    again.write_update(&root).unwrap();
    let text = std::fs::read_to_string(root.join("Synthetic.snxlib")).unwrap();
    let file = LibraryFile::parse(&text).unwrap();
    let pricing = &file.tables[synthetic::table_of(0)].pricing;
    let unit = |n: usize| {
        pricing[&row_id(n).to_string()]["ExampleDistributor"][&format!("ED-{n}")].breaks[0]
            .unit_price
    };
    assert_eq!(unit(0), 0.5, "kept from the stored file");
    assert_eq!(unit(5), 0.9, "the writer's own price wins");
}

#[test]
fn an_update_refuses_a_library_with_an_unreadable_row_id() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let snxlib = synthetic::library(10).write(&root).unwrap();
    let id = row_id(3).to_string();
    let text = std::fs::read_to_string(&snxlib).unwrap();
    std::fs::write(&snxlib, text.replace(&id, "not-a-row-id")).unwrap();
    let before = read_tree(&root);
    let err = synthetic::library(10)
        .write_update(&root)
        .unwrap_err()
        .to_string();
    assert!(err.contains("1 row(s)"), "{err}");
    let mut after = read_tree(&root);
    after.retain(|rel, _| !rel.contains("writelock"));
    assert_eq!(after, before);
}

/// Add rows `ns` as a user would: straight into the `.snxlib`, in `table`.
fn add_user_rows(snxlib: &std::path::Path, table: &str, ns: &[usize]) {
    let text = std::fs::read_to_string(snxlib).unwrap();
    let mut file = LibraryFile::parse(&text).unwrap();
    let entry =
        file.tables
            .entry(table.to_string())
            .or_insert_with(|| signex_library::LibraryTable {
                columns: signex_library::TABLE_HEADER
                    .iter()
                    .map(|c| c.to_string())
                    .collect(),
                ..Default::default()
            });
    for &n in ns {
        let mut row = synthetic::row(n, 1, 1);
        row.internal_pn = signex_library::identity::InternalPn::new(format!("MINE-{n}"));
        row.refresh_content_hash().unwrap();
        let cells = signex_library::adapters::local_git::component_to_library_row(&row).unwrap();
        entry.rows.push(std::sync::Arc::new(cells));
    }
    std::fs::write(snxlib, file.write().unwrap()).unwrap();
}

fn line_of(snxlib: &std::path::Path, n: usize) -> String {
    let id = row_id(n).to_string();
    std::fs::read_to_string(snxlib)
        .unwrap()
        .lines()
        .find(|l| l.starts_with(&id))
        .unwrap()
        .to_string()
}

/// Rows and files a user added (not in the writer's ledger) survive an
/// update byte for byte; recorded ones the writer dropped are deleted.
#[test]
fn an_update_keeps_what_a_user_added() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let snxlib = synthetic::library(20).write(&root).unwrap();
    add_user_rows(&snxlib, "mine", &[900]);
    add_user_rows(&snxlib, synthetic::table_of(0), &[901]);
    let (mine, mine2) = (line_of(&snxlib, 900), line_of(&snxlib, 901));
    let user_fp = root.join(format!("footprints/{}.snxfpt", uuid::Uuid::from_u128(77)));
    std::fs::write(&user_fp, b"a user's footprint\n").unwrap();

    let summary = synthetic::library(10).write_update(&root).unwrap();
    assert_eq!(summary.ledger, signex_library::LedgerState::Trusted);
    assert_eq!(summary.rows_removed, 10);
    assert_eq!(summary.rows_kept, 2);
    assert_eq!(summary.files_deleted, 1, "footprint 1 of the first write");
    let after = std::fs::read_to_string(&snxlib).unwrap();
    assert!(after.contains(&format!("\n{mine}\n")));
    assert!(after.contains(&format!("\n{mine2}\n")));
    assert!(!after.contains(&row_id(15).to_string()));
    assert_eq!(std::fs::read(&user_fp).unwrap(), b"a user's footprint\n");
    let adapter = LocalGitAdapter::open(&snxlib).unwrap();
    assert!(row_of(&adapter, 900).is_some() && row_of(&adapter, 901).is_some());
}

/// A missing, damaged or foreign ledger deletes nothing.
#[test]
fn an_untrusted_ledger_deletes_nothing() {
    let ledger_file = signex_library::writer::LEDGER_FILE;
    for case in ["missing", "not json", "foreign library", "escaping name"] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Synthetic");
        synthetic::library(20).write(&root).unwrap();
        let path = root.join(ledger_file);
        let text = std::fs::read_to_string(&path).unwrap();
        match case {
            "missing" => std::fs::remove_file(&path).unwrap(),
            "not json" => std::fs::write(&path, "{").unwrap(),
            "foreign library" => {
                let id = synthetic::LIBRARY_ID.to_string();
                let other = uuid::Uuid::from_u128(9).to_string();
                std::fs::write(&path, text.replace(&id, &other)).unwrap();
            }
            _ => std::fs::write(&path, text.replacen("\"footprints/", "\"../footprints/", 1))
                .unwrap(),
        }
        let summary = synthetic::library(10).write_update(&root).unwrap();
        assert_ne!(
            summary.ledger,
            signex_library::LedgerState::Trusted,
            "{case}"
        );
        assert_eq!(summary.rows_removed, 0, "{case}");
        assert_eq!(summary.rows_kept, 10, "{case}");
        assert_eq!(summary.files_deleted, 0, "{case}");
        // The update records only what it wrote now.
        let again = synthetic::library(10).write_update(&root).unwrap();
        assert_eq!(again.ledger, signex_library::LedgerState::Trusted, "{case}");
        assert_eq!(
            again.rows_removed, 0,
            "{case}: the kept rows are not the writer's"
        );
    }
}

/// A recorded file that became a link, or a recorded folder that became a
/// link to elsewhere, is not deleted, and neither is what it points at.
#[cfg(unix)]
#[test]
fn an_update_never_deletes_through_a_link() {
    use signex_library::primitive::{SimFile, SimKind, SimModel};
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(outside.join("sims")).unwrap();
    let victim = outside.join("victim.snxfpt");
    std::fs::write(&victim, b"keep").unwrap();

    let root = dir.path().join("Synthetic");
    synthetic::library(20).write(&root).unwrap();
    // Footprint 1, which the update drops, became a link to the victim.
    let uuid = synthetic::footprint(1).footprints[0].uuid;
    let fp1 = root.join(format!("footprints/{uuid}.snxfpt"));
    std::fs::remove_file(&fp1).unwrap();
    std::os::unix::fs::symlink(&victim, &fp1).unwrap();
    let summary = synthetic::library(10).write_update(&root).unwrap();
    assert_eq!(summary.files_deleted, 0);
    assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
    assert!(
        std::fs::symlink_metadata(&fp1).is_ok(),
        "the link is left alone"
    );

    // The recorded sims folder became a link to a folder outside.
    let second = dir.path().join("Second");
    let mut w = synthetic::library(10);
    let model = SimModel {
        uuid: uuid::Uuid::from_u128(5),
        ..SimModel::empty("m", SimKind::Ngspice)
    };
    w.add_sim_file(SimFile::from_model(model)).unwrap();
    w.write(&second).unwrap();
    let name = format!("{}.snxsim", uuid::Uuid::from_u128(5));
    std::fs::rename(second.join("sims"), dir.path().join("moved")).unwrap();
    std::fs::write(outside.join("sims").join(&name), b"keep").unwrap();
    std::os::unix::fs::symlink(outside.join("sims"), second.join("sims")).unwrap();
    let summary = synthetic::library(10).write_update(&second).unwrap();
    assert_eq!(summary.files_deleted, 0);
    assert_eq!(
        std::fs::read(outside.join("sims").join(&name)).unwrap(),
        b"keep"
    );
}
