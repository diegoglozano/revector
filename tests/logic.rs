//! Unit-level tests for the pure logic that doesn't need a live Qdrant:
//! chain resolution, reversibility, checksums, and spec parsing.

use revector::chain::Chain;
use revector::migration::{checksum_bytes, Migration, MigrationFile};
use revector::ops::{Operation, Reversibility};

/// Build a `Migration` directly from YAML, bypassing the filesystem.
fn mig(yaml: &str) -> Migration {
    let file: MigrationFile = serde_yaml::from_str(yaml).expect("valid migration yaml");
    Migration {
        file,
        path: std::path::PathBuf::from("<test>"),
        checksum: checksum_bytes(yaml.as_bytes()),
    }
}

const M1: &str = r#"
revision: "0001"
down_revision: null
description: create products
up:
  - op: create_collection
    name: products
    spec:
      vectors:
        "":
          size: 768
          distance: Cosine
"#;

const M2: &str = r#"
revision: "0002"
down_revision: "0001"
description: index category
up:
  - op: create_payload_index
    collection: products
    field_name: category
    schema: keyword
"#;

const M3: &str = r#"
revision: "0003"
down_revision: "0002"
up:
  - op: create_vector
    collection: products
    name: image
    spec:
      size: 512
      distance: Dot
"#;

#[test]
fn resolves_linear_chain_in_order() {
    // Intentionally out of file order; resolver must order by the chain links.
    let chain = Chain::resolve(vec![mig(M3), mig(M1), mig(M2)]).unwrap();
    let order: Vec<&str> = chain.migrations().iter().map(|m| m.revision()).collect();
    assert_eq!(order, vec!["0001", "0002", "0003"]);
    assert_eq!(chain.head(), Some("0003"));
    assert_eq!(chain.position("0002"), Some(1));
}

#[test]
fn empty_set_is_valid_empty_chain() {
    let chain = Chain::resolve(vec![]).unwrap();
    assert!(chain.is_empty());
    assert_eq!(chain.head(), None);
}

#[test]
fn rejects_duplicate_revision() {
    let err = Chain::resolve(vec![mig(M1), mig(M1)]).unwrap_err();
    assert!(err.to_string().contains("duplicate"), "{err}");
}

#[test]
fn rejects_missing_parent() {
    let orphan = r#"
revision: "0009"
down_revision: "does_not_exist"
up: []
"#;
    let err = Chain::resolve(vec![mig(M1), mig(orphan)]).unwrap_err();
    assert!(err.to_string().contains("unknown down_revision"), "{err}");
}

#[test]
fn rejects_multiple_bases() {
    let other_base = r#"
revision: "base2"
down_revision: null
up: []
"#;
    let err = Chain::resolve(vec![mig(M1), mig(other_base)]).unwrap_err();
    assert!(err.to_string().contains("multiple base"), "{err}");
}

#[test]
fn rejects_branch() {
    // Two migrations both claim 0001 as parent.
    let branch = r#"
revision: "0002b"
down_revision: "0001"
up: []
"#;
    let err = Chain::resolve(vec![mig(M1), mig(M2), mig(branch)]).unwrap_err();
    assert!(err.to_string().contains("multiple children"), "{err}");
}

#[test]
fn rejects_cycle() {
    // Every node has a parent → no base → reported as cycle/no-base.
    let a = r#"
revision: "a"
down_revision: "b"
up: []
"#;
    let b = r#"
revision: "b"
down_revision: "a"
up: []
"#;
    let err = Chain::resolve(vec![mig(a), mig(b)]).unwrap_err();
    assert!(
        err.to_string().contains("no base") || err.to_string().contains("cycle"),
        "{err}"
    );
}

#[test]
fn auto_inverts_reversible_ops() {
    let m = mig(M2); // create_payload_index → delete_payload_index
    let down = m.downgrade_ops().unwrap();
    assert_eq!(down.len(), 1);
    match &down[0] {
        Operation::DeletePayloadIndex {
            collection,
            field_name,
            schema,
        } => {
            assert_eq!(collection, "products");
            assert_eq!(field_name, "category");
            assert!(schema.is_some());
        }
        other => panic!("expected delete_payload_index, got {other:?}"),
    }
    assert!(m.is_reversible());
}

#[test]
fn create_collection_inverts_to_delete() {
    let m = mig(M1);
    let down = m.downgrade_ops().unwrap();
    assert!(
        matches!(down.as_slice(), [Operation::DeleteCollection { name }] if name == "products")
    );
}

#[test]
fn delete_collection_is_irreversible_without_explicit_down() {
    let m = mig(r#"
revision: "x"
down_revision: null
up:
  - op: delete_collection
    name: gone
"#);
    assert!(!m.is_reversible());
    let err = m.downgrade_ops().unwrap_err();
    assert!(err.to_string().contains("irreversible"), "{err}");
}

#[test]
fn explicit_down_overrides_auto_inversion() {
    let m = mig(r#"
revision: "x"
down_revision: null
up:
  - op: delete_collection
    name: gone
down:
  - op: create_collection
    name: gone
    spec:
      vectors:
        "":
          size: 8
          distance: Euclid
"#);
    assert!(m.is_reversible());
    let down = m.downgrade_ops().unwrap();
    assert!(
        matches!(down.as_slice(), [Operation::CreateCollection { name, .. }] if name == "gone")
    );
}

#[test]
fn exec_op_is_irreversible_without_explicit_down() {
    let op = Operation::Exec(revector::ops::ExecOp {
        command: "echo hi".to_string(),
        name: None,
        workdir: None,
    });
    assert!(matches!(op.auto_inverse(), Reversibility::Irreversible(_)));
}

#[test]
fn multi_op_down_is_reverse_order() {
    let m = mig(M3); // single create_vector → delete_vector is irreversible
                     // create_vector auto-inverts to delete_vector (reversible).
    let down = m.downgrade_ops().unwrap();
    assert!(matches!(down.as_slice(), [Operation::DeleteVector { name, .. }] if name == "image"));
}

#[test]
fn checksum_is_stable_and_sensitive() {
    assert_eq!(checksum_bytes(b"abc"), checksum_bytes(b"abc"));
    assert_ne!(checksum_bytes(b"abc"), checksum_bytes(b"abd"));
}
