//! #416 oracle: seeded random edit sequences through the adapter, checked
//! against a model that applies the same edits the full way (edit, prune
//! every price, `LibraryFile::write`). Whatever path the adapter takes, the
//! bytes it writes and the edits it refuses must match the model exactly.

use super::helpers::*;
use super::*;
use crate::alternate_symbol::AlternateSymbol;
use crate::cascade::{CascadeKind, PrimitiveSave, apply_cascade_bump, needs_bump};
use crate::component::{DatasheetRef, PlmReserved};
use crate::distributor::PriceBreak;
use crate::footprint_binding::FootprintBinding;
use crate::identity::ComponentClass;
use crate::library_file::{FORMAT_TOKEN, LibrarySection};
use crate::lifecycle::LifecycleState;
use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use crate::manufacturer::{DistributorListing, ManufacturerPart};
use crate::param::ParamMap;
use crate::pricing::{CachedPricing, PricingUpdate};
use crate::primitive::PrimitiveRef;

const TABLE: &str = "parts";

/// xorshift64*: reproducible without a dependency.
pub(super) struct Rng(pub(super) u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }
    pub(super) fn uuid(&mut self) -> Uuid {
        Uuid::from_u64_pair(self.next(), self.next())
    }
}

const NAMES: &[&str] = &["R-1", "Ω-10k", "µF 日本", "C 0603", "L'1", "Ü2"];
const BAD: &[&str] = &["bad\u{7f}", "tab\there", "quote'''s", "nl\nx"];
const SKUS: &[&str] = &["DK-1", "DK-2", "DK-3"];

pub(super) struct World {
    pub(super) lib: Uuid,
    /// Symbol uuids rows bind to (one of them is the default).
    pub(super) symbols: Vec<Uuid>,
    pub(super) footprints: Vec<Uuid>,
}

pub(super) fn random_row(rng: &mut Rng, w: &World, row_id: Uuid) -> ComponentRow {
    let mut pn = NAMES[rng.below(NAMES.len())].to_string();
    if rng.one_in(12) {
        pn = BAD[rng.below(BAD.len())].to_string();
    }
    let symbol = w.symbols[rng.below(w.symbols.len())];
    let mut alternate_symbols = Vec::new();
    if rng.one_in(5) {
        let alt = w.symbols.iter().find(|s| **s != symbol).copied().unwrap();
        alternate_symbols.push(AlternateSymbol::new(
            PrimitiveRef::new(w.lib, alt),
            "1.0.0",
            "IEC",
        ));
    }
    let footprint = w.footprints[rng.below(w.footprints.len())];
    let mut alternate_footprints = Vec::new();
    if rng.one_in(5) {
        let alt = w
            .footprints
            .iter()
            .find(|f| **f != footprint)
            .copied()
            .unwrap();
        alternate_footprints.push(FootprintBinding::new(PrimitiveRef::new(w.lib, alt)));
    }
    let description = if rng.one_in(3) {
        "100 nF \"X7R\" µ".to_string()
    } else {
        String::new()
    };
    let at = chrono::DateTime::from_timestamp(1_700_000_000 + rng.below(1000) as i64, 0).unwrap();
    ComponentRow {
        alternate_symbols,
        row_id,
        internal_pn: InternalPn::new(pn.clone()),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(w.lib, symbol),
        footprint_ref: Some(PrimitiveRef::new(w.lib, footprint)),
        alternate_footprints,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: vec![DistributorListing::new(
            "DigiKey",
            SKUS[rng.below(SKUS.len())],
        )],
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: format!("0.0.{}", rng.below(5)),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: at,
        updated: at,
        content_hash: [0u8; 32],
        server_revision: None,
        datasheet_label: String::new(),
        description,
        more_datasheets: Vec::new(),
    }
}

/// Ids in the model's table, in row order (duplicates included).
fn ids(model: &LibraryFile) -> Vec<Uuid> {
    model.tables[TABLE]
        .rows
        .iter()
        .filter_map(|r| r.cells.get(LEGACY_ROW_ID_COL))
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect()
}

fn find<'a>(table: &'a mut LibraryTable, id: &str) -> Option<&'a mut LibraryRow> {
    table
        .rows
        .iter_mut()
        .find(|r| r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(id))
        .map(Arc::make_mut)
}

fn table_of<'a>(lf: &'a mut LibraryFile, name: &str) -> Result<&'a mut LibraryTable, LibraryError> {
    lf.tables
        .get_mut(name)
        .ok_or_else(|| LibraryError::NotFound(format!("table {name} not in library")))
}

/// The edit the model applies; mirrors the adapter's closures.
enum Edit {
    Insert(String, ComponentRow),
    Update(String, ComponentRow),
    Delete(String, Uuid, Option<u64>),
    Bind(String, Uuid, PrimitiveSave),
    Price(PricingUpdate),
}

/// Apply `edit` to `lf` the full way. `Ok(false)` = nothing changed.
fn model_apply(lf: &mut LibraryFile, edit: &Edit) -> Result<bool, LibraryError> {
    match edit {
        Edit::Insert(table, row) => {
            let lib_row = component_to_library_row(row)?;
            check_insert_unique(lf, &row.row_id.to_string(), &row.internal_pn.to_string())?;
            let entry = lf
                .tables
                .entry(table.clone())
                .or_insert_with(|| LibraryTable {
                    columns: legacy_columns(),
                    ..LibraryTable::default()
                });
            validate_legacy_header(table, &entry.columns)?;
            declare_alternate_symbols_column(&mut entry.columns, &lib_row);
            entry.rows.push(Arc::new(lib_row));
        }
        Edit::Update(table, row) => {
            let lib_row = component_to_library_row(row)?;
            let id = row.row_id.to_string();
            check_pn_change(lf, table, &id, &row.internal_pn.to_string())?;
            let entry = table_of(lf, table)?;
            validate_legacy_header(table, &entry.columns)?;
            let slot = find(entry, &id)
                .ok_or_else(|| LibraryError::NotFound(format!("row {id} in table {table}")))?;
            check_row_base(slot, row.server_revision)?;
            *slot = lib_row.clone();
            declare_alternate_symbols_column(&mut entry.columns, &lib_row);
        }
        Edit::Delete(table, id, base) => {
            let entry = table_of(lf, table)?;
            let id = id.to_string();
            if let Some(stored) = find(entry, &id) {
                check_row_base(stored, *base)?;
            }
            let before = entry.rows.len();
            entry
                .rows
                .retain(|r| r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) != Some(&id));
            if entry.rows.len() == before {
                return Err(LibraryError::NotFound(format!("row {id} in table {table}")));
            }
            entry.pricing.remove(&id);
        }
        Edit::Bind(table, id, save) => {
            let entry = table_of(lf, table)?;
            validate_legacy_header(table, &entry.columns)?;
            let id = id.to_string();
            let slot = find(entry, &id)
                .ok_or_else(|| LibraryError::NotFound(format!("row {id} in table {table}")))?;
            let mut row = library_row_to_component(slot)?;
            if !needs_bump(&row, save) {
                return Ok(false);
            }
            apply_cascade_bump(&mut row, save);
            *slot = component_to_library_row(&row)?;
        }
        Edit::Price(update) => {
            if apply_pricing_update(lf, update).is_err() {
                return Ok(false);
            }
        }
    }
    prune_stale_pricing(lf);
    Ok(true)
}

fn adapter_apply(a: &LocalGitAdapter, edit: &Edit) -> Result<(), LibraryError> {
    match edit {
        Edit::Insert(t, row) => a.insert_row(t, row.clone(), "insert"),
        Edit::Update(t, row) => a.update_row(t, row.clone(), "update").map(|_| ()),
        Edit::Delete(t, id, base) => a.delete_row(t, RowId::from_uuid(*id), *base, "delete"),
        Edit::Bind(t, id, save) => a
            .bind_row_primitive_version(t, RowId::from_uuid(*id), save, "bind")
            .map(|_| ()),
        Edit::Price(update) => a
            .store_pricing(
                vec![update.clone()],
                &std::sync::atomic::AtomicBool::new(false),
                "price",
            )
            .map(|_| ()),
    }
}

fn random_edit(rng: &mut Rng, w: &World, model: &LibraryFile) -> Edit {
    let ids = ids(model);
    let existing = |rng: &mut Rng| {
        if ids.is_empty() || rng.one_in(10) {
            rng.uuid()
        } else {
            ids[rng.below(ids.len())]
        }
    };
    let table = match rng.below(20) {
        0 => "notes".to_string(),
        1 => "missing".to_string(),
        _ => TABLE.to_string(),
    };
    let base = |rng: &mut Rng, id: Uuid| match rng.below(4) {
        0 => None,
        1 => Some(rng.next()),
        _ => model.tables[TABLE].rows.iter().find_map(|r| {
            (r.cells.get(LEGACY_ROW_ID_COL) == Some(&id.to_string())).then(|| row_base(r))
        }),
    };
    match rng.below(10) {
        0..=2 => {
            let id = if rng.one_in(8) && !ids.is_empty() {
                ids[rng.below(ids.len())]
            } else {
                rng.uuid()
            };
            Edit::Insert(table, random_row(rng, w, id))
        }
        3..=5 => {
            let id = existing(rng);
            let mut row = random_row(rng, w, id);
            row.server_revision = base(rng, id);
            Edit::Update(table, row)
        }
        6 => {
            let id = existing(rng);
            let b = base(rng, id);
            Edit::Delete(table, id, b)
        }
        7 => {
            let id = existing(rng);
            Edit::Bind(
                table,
                id,
                PrimitiveSave {
                    kind: CascadeKind::Footprint,
                    library_id: w.lib,
                    uuid: w.footprints[rng.below(w.footprints.len())],
                    version: format!("2.{}.0", rng.below(9)),
                },
            )
        }
        _ => Edit::Price(PricingUpdate {
            table,
            row_id: existing(rng),
            distributor: "DigiKey".into(),
            pricing: CachedPricing {
                sku: SKUS[rng.below(SKUS.len())].into(),
                currency: "USD".into(),
                stock: None,
                breaks: vec![PriceBreak {
                    qty: 1,
                    unit_price: rng.below(100) as f64 / 10.0,
                }],
                fetched_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            },
        }),
    }
}

/// A library seeded straight on disk: `rows` random parts (one id used
/// twice) and a `notes` table without a `row_id` column.
pub(super) fn seeded(dir: &Path, rng: &mut Rng, w: &World, rows: usize, git: bool) -> PathBuf {
    crate::git_identity::test_identity::install();
    let file = dir.join("lib").join("lib.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: w.lib,
        library: LibrarySection {
            name: "lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let opts = LibraryInitOptions {
        enable_git: git,
        use_lfs: false,
    };
    drop(LocalGitAdapter::init(&file, manifest.clone(), opts).unwrap());
    if git {
        // The commit stores the clean-filtered file; with LF text and
        // autocrlf on, that is still the bytes on disk.
        git2::Repository::open(file.parent().unwrap())
            .unwrap()
            .config()
            .unwrap()
            .set_bool("core.autocrlf", true)
            .unwrap();
    }
    let mut parts = LibraryTable {
        columns: legacy_columns(),
        ..LibraryTable::default()
    };
    let mut ids: Vec<Uuid> = (0..rows).map(|_| rng.uuid()).collect();
    if let Some(&dup) = ids.first() {
        ids.push(dup);
    }
    for id in ids {
        let mut row = random_row(rng, w, id);
        row.internal_pn = InternalPn::new("seed µ");
        row.primary_mpn = ManufacturerPart::draft("Acme", "seed");
        row.alternate_symbols.clear();
        parts
            .rows
            .push(Arc::new(component_to_library_row(&row).unwrap()));
    }
    let mut notes = LibraryTable {
        columns: vec!["name".into(), "value".into()],
        ..LibraryTable::default()
    };
    for (n, v) in [("b", "2"), ("a", "Ω")] {
        let mut row = LibraryRow::default();
        row.cells.insert("name".into(), n.into());
        row.cells.insert("value".into(), v.into());
        notes.rows.push(Arc::new(row));
    }
    let lf = LibraryFile {
        manifest,
        tables: [(TABLE.to_string(), parts), ("notes".to_string(), notes)].into(),
        layout: Default::default(),
    };
    fs::write(&file, lf.write().unwrap()).unwrap();
    file
}

/// Runs one seed; returns how many edits were written, no-ops and refused.
fn run_seed(seed: u64, edits: usize, git: bool) -> [usize; 3] {
    let mut tally = [0; 3];
    let mut rng = Rng(seed);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let rows = rng.below(30);
    let file = seeded(dir.path(), &mut rng, &w, rows, git);
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let mut model = LibraryFile::parse(&fs::read_to_string(&file).unwrap()).unwrap();
    for step in 0..edits {
        let edit = random_edit(&mut rng, &w, &model);
        let before = fs::read(&file).unwrap();
        let mut next = model.clone();
        // The full path refuses at encode time too (a control character,
        // `'''`), so the model's verdict includes its own write.
        let expected = model_apply(&mut next, &edit).and_then(|changed| {
            if changed {
                next.write()?;
            }
            Ok(changed)
        });
        let got = adapter_apply(&adapter, &edit);
        let ctx = format!("seed {seed} step {step}");
        match (expected, got) {
            (Ok(true), Ok(())) => {
                tally[0] += 1;
                if let Edit::Bind(t, id, _) = &edit {
                    // The bump stamps `updated` with the clock; take the
                    // adapter's stamp so the rest can be compared.
                    let disk =
                        LibraryFile::parse(&String::from_utf8_lossy(&fs::read(&file).unwrap()))
                            .unwrap();
                    let id = id.to_string();
                    let stamp = disk.tables[t.as_str()]
                        .rows
                        .iter()
                        .find(|r| r.cells.get(LEGACY_ROW_ID_COL) == Some(&id))
                        .unwrap()
                        .cells["updated"]
                        .clone();
                    let table = next.tables.get_mut(t).unwrap();
                    find(table, &id)
                        .unwrap()
                        .cells
                        .insert("updated".into(), stamp);
                }
                let text = next.write().unwrap();
                assert_eq!(
                    String::from_utf8(fs::read(&file).unwrap()).unwrap(),
                    text,
                    "{ctx}"
                );
                model = next;
            }
            (Ok(false), Ok(())) => {
                tally[1] += 1;
                assert_eq!(fs::read(&file).unwrap(), before, "{ctx}");
            }
            (Err(e), Err(g)) => {
                tally[2] += 1;
                assert_eq!(e.to_string(), g.to_string(), "{ctx}");
                assert_eq!(
                    fs::read(&file).unwrap(),
                    before,
                    "{ctx}: refused edit wrote"
                );
            }
            (e, g) => panic!("{ctx}: model {e:?}, adapter {g:?}"),
        }
    }
    if git {
        let repo = git2::Repository::open(dir.path().join("lib")).unwrap();
        let head = repo.head().unwrap().peel_to_tree().unwrap();
        let committed = head.get_path(Path::new("lib.snxlib")).unwrap().id();
        let staged = repo
            .index()
            .unwrap()
            .get_path(Path::new("lib.snxlib"), 0)
            .unwrap()
            .id;
        let disk = git2::Oid::hash_file(git2::ObjectType::Blob, &file).unwrap();
        assert_eq!((committed, staged), (disk, disk), "seed {seed}");
    }
    tally
}

#[test]
fn random_edit_sequences_match_the_full_write() {
    super::row_edit::ROW_WRITES.with(|n| n.set(0));
    let mut total = [0; 3];
    for seed in 1..=24u64 {
        let tally = run_seed(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), 40, false);
        for (t, n) in total.iter_mut().zip(tally) {
            *t += n;
        }
    }
    // Every outcome is exercised: written, no-op, refused.
    assert!(total.iter().all(|&n| n > 20), "{total:?}");
    // And most writes took the row write, not the full one.
    assert!(row_writes() > total[0] / 2, "{} row writes", row_writes());
}

#[test]
fn random_edit_sequences_match_the_full_write_with_git() {
    for seed in [7u64, 11] {
        run_seed(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), 25, true);
    }
}

/// #831 with shared rows (#416): an edit works on a clone that shares every
/// row, so a refused edit must leave memory holding the very same rows.
#[test]
fn a_refused_edit_leaves_every_row_shared_and_unchanged() {
    let mut rng = Rng(5);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 8, false);
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let rows = |a: &LocalGitAdapter| a.library_file.read().unwrap().tables[TABLE].rows.clone();
    let before = rows(&adapter);
    let disk = fs::read(&file).unwrap();

    let err = adapter
        .mutate_library_file(
            |lf| {
                let table = lf.tables.get_mut(TABLE).unwrap();
                Arc::make_mut(&mut table.rows[0])
                    .cells
                    .insert("internal_pn".into(), "edited".into());
                table.rows.pop();
                Err(LibraryError::Backend("refused".into()))
            },
            "edit",
            "edit",
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "backend: refused");

    let after = rows(&adapter);
    assert_eq!(after, before);
    assert!(before.iter().zip(&after).all(|(b, a)| Arc::ptr_eq(b, a)));
    assert_eq!(fs::read(&file).unwrap(), disk);
}

fn row_writes() -> usize {
    super::row_edit::ROW_WRITES.with(std::cell::Cell::get)
}

fn commits(root: &Path) -> usize {
    let repo = git2::Repository::open(root).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    walk.count()
}

/// The row write is taken only when memory holds this adapter's own last
/// write and the edit keeps the table's columns; each saved edit is one
/// commit whose blob is the file on disk and the index entry (#416).
#[test]
fn row_writes_need_an_own_write_and_the_same_columns() {
    let mut rng = Rng(9);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 6, true);
    let root = dir.path().join("lib");
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let clean = |rng: &mut Rng| {
        let id = rng.uuid();
        let mut row = random_row(rng, &w, id);
        let pn = format!("ok-{}", id.as_u128() % 100_000);
        row.internal_pn = InternalPn::new(pn.clone());
        row.primary_mpn = ManufacturerPart::draft("Acme", &pn);
        row.alternate_symbols.clear();
        row
    };
    let start = row_writes();
    let step = |expect_row_write: usize, what: &str, f: &dyn Fn()| {
        let before = commits(&root);
        f();
        assert_eq!(row_writes() - start, expect_row_write, "{what}");
        assert_eq!(commits(&root), before + 1, "{what}: one commit");
        let repo = git2::Repository::open(&root).unwrap();
        let head = repo.head().unwrap().peel_to_tree().unwrap();
        let committed = head.get_path(Path::new("lib.snxlib")).unwrap().id();
        let staged = repo
            .index()
            .unwrap()
            .get_path(Path::new("lib.snxlib"), 0)
            .unwrap()
            .id;
        let disk = git2::Oid::hash_file(git2::ObjectType::Blob, &file).unwrap();
        assert_eq!((committed, staged), (disk, disk), "{what}");
    };

    let first = clean(&mut rng);
    step(0, "first edit after open", &|| {
        adapter.insert_row(TABLE, first.clone(), "a").unwrap()
    });
    let second = clean(&mut rng);
    step(1, "insert", &|| {
        adapter.insert_row(TABLE, second.clone(), "b").unwrap()
    });
    let mut edited = adapter
        .read_row(TABLE, RowId::from_uuid(first.row_id))
        .unwrap();
    let stale = edited.clone();
    edited.version = "9.9.9".into();
    step(2, "update", &|| {
        adapter.update_row(TABLE, edited.clone(), "c").unwrap();
    });
    let refused = adapter
        .update_row(TABLE, stale.clone(), "stale")
        .unwrap_err();
    assert!(
        matches!(refused, LibraryError::RowChanged { .. }),
        "{refused}"
    );
    let save = PrimitiveSave {
        kind: CascadeKind::Footprint,
        library_id: w.lib,
        uuid: second.footprint_ref.unwrap().uuid,
        version: "7.0.0".into(),
    };
    step(3, "bind", &|| {
        assert!(
            adapter
                .bind_row_primitive_version(TABLE, RowId::from_uuid(second.row_id), &save, "d")
                .unwrap()
        );
    });
    step(4, "delete", &|| {
        adapter
            .delete_row(TABLE, RowId::from_uuid(second.row_id), None, "e")
            .unwrap()
    });
    let new_table = clean(&mut rng);
    step(4, "new table", &|| {
        adapter.insert_row("fresh", new_table.clone(), "f").unwrap()
    });
    let mut alt = clean(&mut rng);
    alt.alternate_symbols.push(AlternateSymbol::new(
        PrimitiveRef::new(
            w.lib,
            *w.symbols
                .iter()
                .find(|s| **s != alt.symbol_ref.uuid)
                .unwrap(),
        ),
        "1.0.0",
        "IEC",
    ));
    step(4, "new column", &|| {
        adapter.insert_row(TABLE, alt.clone(), "g").unwrap()
    });
    let other = clean(&mut rng);
    step(5, "after the new column", &|| {
        adapter.insert_row(TABLE, other.clone(), "h").unwrap()
    });

    // Another process writes: the next edit loads it and takes the full
    // write once.
    let elsewhere = LocalGitAdapter::open(&file).unwrap();
    elsewhere
        .insert_row(TABLE, clean(&mut rng), "elsewhere")
        .unwrap();
    let after = clean(&mut rng);
    step(5, "after an external write", &|| {
        adapter.insert_row(TABLE, after.clone(), "i").unwrap()
    });
    let last = clean(&mut rng);
    step(6, "row write again", &|| {
        adapter.insert_row(TABLE, last.clone(), "j").unwrap()
    });
}

/// A library saved with CRLF line endings is loaded, so its first edit
/// takes the full write; the row writes after it read back.
#[test]
fn a_crlf_library_goes_through_the_full_write_once() {
    let mut rng = Rng(13);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 5, false);
    let crlf = fs::read_to_string(&file).unwrap().replace('\n', "\r\n");
    fs::write(&file, crlf).unwrap();
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let start = row_writes();
    for n in 0..3 {
        let id = rng.uuid();
        let mut row = random_row(&mut rng, &w, id);
        row.internal_pn = InternalPn::new(format!("crlf-{n}"));
        row.primary_mpn = ManufacturerPart::draft("Acme", "ok");
        row.alternate_symbols.clear();
        adapter.insert_row(TABLE, row, "edit").unwrap();
    }
    assert_eq!(row_writes() - start, 2);
    let text = fs::read_to_string(&file).unwrap();
    assert!(!text.contains('\r'));
    let reread = LibraryFile::parse(&text).unwrap();
    assert_eq!(reread.write().unwrap(), text);
}

/// A panic while the row-line cache was held poisons its mutex; it is only
/// a cache, so later row writes empty it and carry on (#416).
#[test]
fn a_poisoned_row_line_cache_does_not_block_row_writes() {
    let mut rng = Rng(17);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 4, false);
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let clean = |rng: &mut Rng| {
        let id = rng.uuid();
        let mut row = random_row(rng, &w, id);
        let pn = format!("ok-{}", id.as_u128() % 100_000);
        row.internal_pn = InternalPn::new(pn.clone());
        row.primary_mpn = ManufacturerPart::draft("Acme", &pn);
        row.alternate_symbols.clear();
        row
    };
    let first = clean(&mut rng);
    adapter.insert_row(TABLE, first.clone(), "full").unwrap();

    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _held = adapter.row_lines.lock().unwrap();
        panic!("poison the row-line cache");
    }));
    assert!(poisoned.is_err());
    assert!(adapter.row_lines.is_poisoned());

    let start = row_writes();
    let mut edited = adapter
        .read_row(TABLE, RowId::from_uuid(first.row_id))
        .unwrap();
    edited.version = "3.0.0".into();
    adapter.update_row(TABLE, edited, "row write").unwrap();
    assert_eq!(row_writes(), start + 1);
    assert!(!adapter.row_lines.is_poisoned());
    let reread = LibraryFile::parse(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(reread.write().unwrap(), fs::read_to_string(&file).unwrap());
}

/// #1435 + #416: a file holding keys this build does not model keeps its
/// text, which only the full write edits in place; every edit of it takes
/// the full write and the unknown keys stay on disk byte for byte.
#[test]
fn a_file_with_unknown_keys_keeps_them_on_every_edit() {
    let mut rng = Rng(23);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 4, false);
    let text = fs::read_to_string(&file).unwrap();
    let text = format!("future_root = 1\n{text}").replacen(
        "[tables.parts]\n",
        "[tables.parts]\nfuture_table_key = \"kept\"\n",
        1,
    );
    fs::write(&file, &text).unwrap();
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let start = row_writes();
    for n in 0..2 {
        let id = rng.uuid();
        let mut row = random_row(&mut rng, &w, id);
        row.internal_pn = InternalPn::new(format!("kept-{n}"));
        row.primary_mpn = ManufacturerPart::draft("Acme", "ok");
        row.alternate_symbols.clear();
        adapter.insert_row(TABLE, row, "edit").unwrap();
        let on_disk = fs::read_to_string(&file).unwrap();
        assert!(on_disk.contains("future_root = 1\n"), "edit {n}");
        assert!(
            on_disk.contains("[tables.parts]\nfuture_table_key = \"kept\"\n"),
            "edit {n}"
        );
    }
    assert_eq!(
        row_writes(),
        start,
        "a file with kept text never takes the row write"
    );
    let reread = LibraryFile::parse(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(reread.tables[TABLE].rows.len(), 4 + 1 + 2);
}
