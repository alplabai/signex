//! Integration tests for `DistributorCache`. WS-C acceptance: cache
//! round-trip tests must run **without network**.

#![cfg(feature = "distributors-community")]

use std::time::Duration;

use chrono::Utc;
use signex_library::distributor::{DistributorPart, DistributorSource};
use signex_library::distributors::cache::{DEFAULT_TTL, DistributorCache};
use signex_library::param::ParamMap;

fn sample_part(mpn: &str, source: DistributorSource) -> DistributorPart {
    DistributorPart {
        mpn: mpn.to_string(),
        manufacturer: "Yageo".into(),
        description: "Resistor 10k 1% 0805".into(),
        datasheet_url: Some(url::Url::parse("https://example.com/ds.pdf").unwrap()),
        footprint_hint: Some("0805_2012Metric".into()),
        parameters: ParamMap::new(),
        pricing: None,
        stock: Some(50_000),
        source,
        captured_at: Utc::now(),
        extra: Default::default(),
    }
}

#[test]
fn cache_write_then_read_returns_same_part() {
    let dir = tempfile::tempdir().unwrap();
    let cache = DistributorCache::with_root(dir.path()).unwrap();

    let part = sample_part("RC0805FR-0710KL", DistributorSource::Lcsc);
    cache.put("lcsc", &part).unwrap();

    let got = cache.get("lcsc", "RC0805FR-0710KL", DEFAULT_TTL).unwrap();
    assert_eq!(got, Some(part));
}

#[test]
fn cache_miss_when_entry_absent() {
    let dir = tempfile::tempdir().unwrap();
    let cache = DistributorCache::with_root(dir.path()).unwrap();

    let got = cache.get("lcsc", "DOES-NOT-EXIST", DEFAULT_TTL).unwrap();
    assert_eq!(got, None);
}

#[test]
fn cache_treats_expired_entry_as_miss() {
    let dir = tempfile::tempdir().unwrap();
    let cache = DistributorCache::with_root(dir.path()).unwrap();

    // Backdate the captured_at by 25 hours so the default 24h TTL has elapsed.
    let mut part = sample_part("RC0805FR-0710KL", DistributorSource::Lcsc);
    part.captured_at = Utc::now() - chrono::Duration::hours(25);
    cache.put("lcsc", &part).unwrap();

    let got = cache.get("lcsc", "RC0805FR-0710KL", DEFAULT_TTL).unwrap();
    assert!(got.is_none(), "25h-old entry must be considered expired");
}

#[test]
fn cache_zero_ttl_always_misses() {
    let dir = tempfile::tempdir().unwrap();
    let cache = DistributorCache::with_root(dir.path()).unwrap();

    let part = sample_part("RC0805FR-0710KL", DistributorSource::Lcsc);
    cache.put("lcsc", &part).unwrap();

    let got = cache
        .get("lcsc", "RC0805FR-0710KL", Duration::from_secs(0))
        .unwrap();
    assert!(got.is_none(), "TTL=0 must treat any entry as expired");
}

#[test]
fn cache_partitions_by_provider() {
    // Same MPN under two providers must not collide.
    let dir = tempfile::tempdir().unwrap();
    let cache = DistributorCache::with_root(dir.path()).unwrap();

    let lcsc_part = sample_part("RC0805FR-0710KL", DistributorSource::Lcsc);
    let mut digi_part = sample_part("RC0805FR-0710KL", DistributorSource::DigiKey);
    digi_part.manufacturer = "DigiKey-rebadged".into();

    cache.put("lcsc", &lcsc_part).unwrap();
    cache.put("digikey", &digi_part).unwrap();

    let lcsc_back = cache
        .get("lcsc", "RC0805FR-0710KL", DEFAULT_TTL)
        .unwrap()
        .unwrap();
    let digi_back = cache
        .get("digikey", "RC0805FR-0710KL", DEFAULT_TTL)
        .unwrap()
        .unwrap();
    assert_eq!(lcsc_back.manufacturer, "Yageo");
    assert_eq!(digi_back.manufacturer, "DigiKey-rebadged");
}

#[test]
fn cache_path_layout_matches_spec() {
    // Per WS-C: `<root>/<provider>/<mpn>.json`.
    let dir = tempfile::tempdir().unwrap();
    let cache = DistributorCache::with_root(dir.path()).unwrap();
    let part = sample_part("RC0805FR-0710KL", DistributorSource::Lcsc);
    cache.put("lcsc", &part).unwrap();

    let expected = dir.path().join("lcsc").join("RC0805FR-0710KL.json");
    assert!(expected.exists(), "expected cache file at {expected:?}");
}
