//! Live integration tests against a real Qdrant via testcontainers.
//!
//! These are the tests that actually matter for a migration tool: they exercise
//! the apply/rollback/status/diff paths end to end against a real server. They
//! require Docker and are skipped automatically if a container can't start.

use std::path::Path;

use revector::chain::Chain;
use revector::client;
use revector::config::Config;
use revector::diff;
use revector::migration::discover;
use revector::runner::Runner;
use revector::spec::CollectionSpec;

use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

/// Qdrant server version the suite is pinned to. Kept in lockstep with the
/// `qdrant-client` crate in `Cargo.toml` and the compatibility table in the
/// README — they are bumped together. Override with `REVECTOR_QDRANT_VERSION`
/// to probe a different server without touching code; the scheduled
/// `qdrant-compat` CI job points this at `latest` to catch new releases early.
const SUPPORTED_QDRANT_VERSION: &str = "v1.18.2";

/// Boot a Qdrant container and return it alongside a ready config.
///
/// Returns `None` (and prints a skip notice) when Docker is unavailable or the
/// image can't be pulled, so the suite is a no-op in environments without
/// Docker rather than a hard failure. CI with Docker exercises the real paths.
///
/// The container handle must be kept alive for the duration of the test.
async fn boot() -> Option<(Option<testcontainers::ContainerAsync<GenericImage>>, Config)> {
    // Escape hatch: run against an already-running Qdrant (e.g. a local binary)
    // instead of spinning up a container. Useful where Docker can't pull images.
    if let Ok(url) = std::env::var("REVECTOR_TEST_URL") {
        // Each test gets an isolated tracking collection name to avoid clashes
        // when sharing one server.
        let suffix: u32 = rand_suffix();
        let config = Config {
            url,
            tracking_collection: format!("_revector_test_{suffix}"),
            ..Config::default()
        };
        return Some((None, config));
    }

    let tag = std::env::var("REVECTOR_QDRANT_VERSION")
        .unwrap_or_else(|_| SUPPORTED_QDRANT_VERSION.to_string());
    let container = match GenericImage::new("qdrant/qdrant", tag.as_str())
        .with_exposed_port(6334.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Actix runtime found"))
        .start()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipping integration test: could not start Qdrant container ({e})");
            return None;
        }
    };

    let host = container.get_host().await.expect("host");
    let port = container
        .get_host_port_ipv4(6334.tcp())
        .await
        .expect("mapped grpc port");

    let config = Config {
        url: format!("http://{host}:{port}"),
        ..Config::default()
    };

    // gRPC may take a moment past the log line; retry a cheap call until ready.
    let qdrant = client::connect(&config).expect("connect");
    for _ in 0..50 {
        if qdrant.list_collections().await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Some((Some(container), config))
}

/// Cheap pseudo-random suffix from the system clock — avoids a `rand` dep.
fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos())
        % 1_000_000
}

/// Drop collections a test relies on, so reruns against a shared server start
/// clean. Ignores "not found" errors.
async fn cleanup(qdrant: &qdrant_client::Qdrant, names: &[&str]) {
    for name in names {
        if qdrant.collection_exists(*name).await.unwrap_or(false) {
            let _ = qdrant.delete_collection(*name).await;
        }
    }
}

/// Global lock serializing the integration tests.
///
/// In testcontainers mode each test gets its own server, but in shared-server
/// mode (`REVECTOR_TEST_URL`) they'd otherwise race on the hardcoded
/// `products` collection. Serializing keeps the suite correct under any runner.
fn test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Expand `boot()` or return early (skipping the test) when Docker is absent.
/// Also takes the serialization lock for the duration of the test.
macro_rules! boot_or_skip {
    () => {{
        let _guard = test_lock().lock().await;
        match boot().await {
            Some((c, cfg)) => (c, cfg, _guard),
            None => return,
        }
    }};
}

/// Write a set of (filename, contents) migration files into a directory.
fn write_migrations(dir: &Path, files: &[(&str, &str)]) {
    std::fs::create_dir_all(dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
}

const MIG_1: &str = r#"
revision: "0001_products"
down_revision: null
description: create products collection
up:
  - op: create_collection
    name: products
    spec:
      vectors:
        "":
          size: 4
          distance: Cosine
"#;

const MIG_2: &str = r#"
revision: "0002_index"
down_revision: "0001_products"
description: index category + add image vector
up:
  - op: create_payload_index
    collection: products
    field_name: category
    schema: keyword
  - op: create_vector
    collection: products
    name: image
    spec:
      size: 8
      distance: Dot
"#;

fn resolve_chain(dir: &Path) -> Chain {
    Chain::resolve(discover(dir).unwrap()).unwrap()
}

#[tokio::test]
async fn up_down_status_roundtrip() {
    let (_c, config, _lock) = boot_or_skip!();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_migrations(dir, &[("0001.yaml", MIG_1), ("0002.yaml", MIG_2)]);

    let qdrant = client::connect(&config).unwrap();
    cleanup(&qdrant, &["products", "ratings"]).await;
    let chain = resolve_chain(dir);

    // --- up to head -------------------------------------------------------
    {
        let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
        let applied = runner.up(None).await.unwrap();
        assert_eq!(applied.revisions, vec!["0001_products", "0002_index"]);
    }
    assert!(qdrant.collection_exists("products").await.unwrap());

    // Named vector `image` should now exist alongside the default vector.
    let info = qdrant.collection_info("products").await.unwrap();
    let params = info.result.unwrap().config.unwrap().params.unwrap();
    let vectors = params.vectors_config.unwrap().config.unwrap();
    match vectors {
        qdrant_client::qdrant::vectors_config::Config::ParamsMap(m) => {
            assert!(
                m.map.contains_key("image"),
                "image vector missing: {:?}",
                m.map.keys().collect::<Vec<_>>()
            );
        }
        other => panic!("expected params map, got {other:?}"),
    }

    // --- status -----------------------------------------------------------
    {
        let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
        let report = runner.status().await.unwrap();
        assert_eq!(report.current.as_deref(), Some("0002_index"));
        assert!(report.revisions.iter().all(|r| r.applied));
        assert!(report.revisions.iter().all(|r| r.checksum_ok == Some(true)));
    }

    // --- idempotent re-up is a no-op -------------------------------------
    {
        let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
        let applied = runner.up(None).await.unwrap();
        assert!(applied.revisions.is_empty());
    }

    // --- down one step ----------------------------------------------------
    {
        let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
        let rolled = runner.down(None, 1).await.unwrap();
        assert_eq!(rolled.revisions, vec!["0002_index"]);
    }
    {
        let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
        let report = runner.status().await.unwrap();
        assert_eq!(report.current.as_deref(), Some("0001_products"));
    }

    // --- down to base removes the collection -----------------------------
    {
        let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
        runner.down(None, 1).await.unwrap();
    }
    assert!(!qdrant.collection_exists("products").await.unwrap());
}

#[tokio::test]
async fn dry_run_does_not_mutate() {
    let (_c, config, _lock) = boot_or_skip!();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_migrations(dir, &[("0001.yaml", MIG_1)]);

    let qdrant = client::connect(&config).unwrap();
    cleanup(&qdrant, &["products"]).await;
    let chain = resolve_chain(dir);

    let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir).dry_run(true);
    let applied = runner.up(None).await.unwrap();
    assert!(applied.dry_run);
    assert!(!qdrant.collection_exists("products").await.unwrap());
}

#[tokio::test]
async fn diff_detects_drift() {
    let (_c, config, _lock) = boot_or_skip!();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_migrations(dir, &[("0001.yaml", MIG_1)]);

    let qdrant = client::connect(&config).unwrap();
    cleanup(&qdrant, &["products"]).await;
    let chain = resolve_chain(dir);
    let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
    runner.up(None).await.unwrap();

    // Declared spec matches what we created → in sync.
    let matching: CollectionSpec = serde_yaml::from_str(
        r#"
vectors:
  "":
    size: 4
    distance: Cosine
"#,
    )
    .unwrap();
    let report = diff::diff_collection(&qdrant, "products", &matching)
        .await
        .unwrap();
    assert!(
        report.in_sync(),
        "expected in sync, got {:?}",
        report.differences
    );

    // Declared a different size → drift on an immutable field.
    let drifted: CollectionSpec = serde_yaml::from_str(
        r#"
vectors:
  "":
    size: 16
    distance: Cosine
"#,
    )
    .unwrap();
    let report = diff::diff_collection(&qdrant, "products", &drifted)
        .await
        .unwrap();
    assert!(!report.in_sync());
    assert!(report.differences.iter().any(|d| d.path.contains("size")));
}

#[tokio::test]
async fn stamp_marks_revisions_without_running_ops() {
    let (_c, config, _lock) = boot_or_skip!();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_migrations(dir, &[("0001.yaml", MIG_1), ("0002.yaml", MIG_2)]);

    let qdrant = client::connect(&config).unwrap();
    cleanup(&qdrant, &["products"]).await;
    let chain = resolve_chain(dir);
    let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);

    // Stamp to head: marks both revisions applied but runs no operations.
    let stamped = runner.stamp("head").await.unwrap();
    assert_eq!(stamped.marked, vec!["0001_products", "0002_index"]);
    assert!(stamped.removed.is_empty());

    // The collection was never created — stamp adopts state, it doesn't apply.
    assert!(!qdrant.collection_exists("products").await.unwrap());

    let report = runner.status().await.unwrap();
    assert_eq!(report.current.as_deref(), Some("0002_index"));

    // Re-stamping head is a no-op.
    let again = runner.stamp("head").await.unwrap();
    assert!(again.marked.is_empty() && again.removed.is_empty());

    // Stamp base clears all applied marks.
    let cleared = runner.stamp("base").await.unwrap();
    assert_eq!(cleared.removed.len(), 2);
    assert_eq!(runner.status().await.unwrap().current, None);
}

#[tokio::test]
async fn advisory_lock_blocks_concurrent_runs() {
    let (_c, config, _lock) = boot_or_skip!();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_migrations(dir, &[("0001.yaml", MIG_1)]);

    let qdrant = client::connect(&config).unwrap();
    cleanup(&qdrant, &["products"]).await;
    let chain = resolve_chain(dir);

    // Simulate another run holding the lock.
    let tracker = revector::tracking::Tracker::new(&qdrant, &config.tracking_collection);
    tracker.ensure().await.unwrap();
    tracker.acquire_lock("other-run", false).await.unwrap();

    // `up` must refuse while the lock is held, and must not mutate anything.
    let runner = Runner::new(&qdrant, &chain, &config.tracking_collection, dir);
    let err = runner.up(None).await.unwrap_err();
    assert!(matches!(err, revector::Error::Locked { .. }), "got {err:?}");
    assert!(!qdrant.collection_exists("products").await.unwrap());

    // `--force` overrides the lock and applies.
    let forced = Runner::new(&qdrant, &chain, &config.tracking_collection, dir).force(true);
    let applied = forced.up(None).await.unwrap();
    assert_eq!(applied.revisions, vec!["0001_products"]);

    // A successful run releases the lock.
    assert!(tracker.read_lock().await.unwrap().is_none());
}
