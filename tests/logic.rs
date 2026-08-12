//! Unit-level tests for the pure logic that doesn't need a live Qdrant:
//! chain resolution, reversibility, checksums, and spec parsing.

use revector::chain::Chain;
use revector::migration::{checksum_bytes, Migration, MigrationFile};
use revector::ops::{Operation, Reversibility};
use revector::spec::CollectionSpec;

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
            ..
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

/// Regression: a collection spec that declares sparse vectors must translate
/// into a non-empty sparse config on create. Previously `apply_collection_spec`
/// dropped `sparse_vectors` entirely, so collections came up dense-only.
// `on_disk` / `always_ram` are deprecated in the client since Qdrant 1.19,
// but revector still forwards them verbatim for migrations that declared
// them — so the assertions below deliberately read the legacy fields.
#[allow(deprecated)]
#[test]
fn collection_spec_carries_sparse_vectors_into_config() {
    let spec: CollectionSpec = serde_yaml::from_str(
        r#"
vectors:
  dense:
    size: 768
    distance: Cosine
sparse_vectors:
  text:
    on_disk: true
    full_scan_threshold: 5000
  keywords: {}
"#,
    )
    .expect("valid collection spec");

    let sparse = revector::convert::sparse_vectors_config(&spec)
        .expect("sparse vectors should produce a config");
    assert_eq!(sparse.map.len(), 2);

    let text = sparse.map.get("text").expect("text sparse vector present");
    let index = text
        .index
        .expect("explicit on_disk/threshold yields an index");
    assert_eq!(index.on_disk, Some(true));
    assert_eq!(index.full_scan_threshold, Some(5000));

    // A bare `{}` entry still creates the vector, just with server defaults.
    let keywords = sparse.map.get("keywords").expect("keywords present");
    assert!(keywords.index.is_none());
}

/// A spec with no sparse vectors must not emit an (empty) sparse config.
#[test]
fn dense_only_spec_has_no_sparse_config() {
    let spec: CollectionSpec = serde_yaml::from_str(
        r#"
vectors:
  "":
    size: 4
    distance: Dot
"#,
    )
    .expect("valid collection spec");
    assert!(revector::convert::sparse_vectors_config(&spec).is_none());
}

/// Pull the `CollectionSpec` out of a migration's first `create_collection` op.
fn first_create_spec(m: &Migration) -> &CollectionSpec {
    match &m.file.up[0] {
        Operation::CreateCollection { spec, .. } => spec,
        other => panic!("expected create_collection, got {other:?}"),
    }
}

/// A `turboquant` quantization spec authored in YAML parses and converts into
/// the proto oneof, mapping the `bits` keyword to the right `TurboQuantBitSize`.
// `on_disk` / `always_ram` are deprecated in the client since Qdrant 1.19,
// but revector still forwards them verbatim for migrations that declared
// them — so the assertions below deliberately read the legacy fields.
#[allow(deprecated)]
#[test]
fn turboquant_spec_converts_bits_and_always_ram() {
    use qdrant_client::qdrant::{quantization_config::Quantization, TurboQuantBitSize};
    use revector::spec::QuantizationSpec;

    let m = mig(r#"
revision: "0001"
down_revision: null
description: turboquant
up:
  - op: create_collection
    name: docs
    spec:
      vectors:
        "":
          size: 1536
          distance: Cosine
      quantization_config:
        turboquant:
          bits: "1.5"
          always_ram: true
"#);

    let q = first_create_spec(&m)
        .quantization_config
        .as_ref()
        .expect("quantization declared");
    assert!(matches!(q, QuantizationSpec::Turboquant(_)));

    match revector::convert::quantization_oneof(q).expect("turboquant yields a oneof") {
        Quantization::Turboquant(t) => {
            assert_eq!(t.always_ram, Some(true));
            assert_eq!(t.bits, Some(TurboQuantBitSize::Bits15 as i32));
        }
        other => panic!("expected turboquant, got {other:?}"),
    }
}

/// TurboQuant `bits` is optional — omitting it leaves the proto field unset so
/// Qdrant applies its own default, while `always_ram` still flows through.
// `on_disk` / `always_ram` are deprecated in the client since Qdrant 1.19,
// but revector still forwards them verbatim for migrations that declared
// them — so the assertions below deliberately read the legacy fields.
#[allow(deprecated)]
#[test]
fn turboquant_spec_without_bits_leaves_default() {
    use qdrant_client::qdrant::quantization_config::Quantization;

    let m = mig(r#"
revision: "0001"
down_revision: null
description: turboquant default bits
up:
  - op: create_collection
    name: docs
    spec:
      vectors:
        "":
          size: 8
          distance: Dot
      quantization_config:
        turboquant:
          always_ram: false
"#);

    let q = first_create_spec(&m).quantization_config.as_ref().unwrap();
    match revector::convert::quantization_oneof(q).unwrap() {
        Quantization::Turboquant(t) => {
            assert_eq!(t.bits, None);
            assert_eq!(t.always_ram, Some(false));
        }
        other => panic!("expected turboquant, got {other:?}"),
    }
}

// --- Qdrant 1.19: memory placement, Turbo4, payload index params ------------

/// `memory` placement flows into every component that accepts it, and coexists
/// with the legacy `on_disk` / `always_ram` booleans instead of replacing them.
#[allow(deprecated)] // asserts the legacy fields are still forwarded untouched
#[test]
fn memory_placement_flows_into_proto() {
    use qdrant_client::qdrant::{quantization_config::Quantization, Memory as QMemory};

    let m = mig(r#"
revision: "0001"
down_revision: null
description: memory placement
up:
  - op: create_collection
    name: docs
    spec:
      vectors:
        "":
          size: 8
          distance: Dot
          on_disk: true
          memory: cold
          hnsw_config:
            m: 16
            memory: cached
      sparse_vectors:
        text:
          memory: cold
      quantization_config:
        scalar:
          always_ram: true
          memory: pinned
      payload:
        memory: cold
"#);

    let spec = first_create_spec(&m);
    let params = revector::convert::vector_params(spec.vectors.get("").unwrap());
    assert_eq!(params.memory, Some(QMemory::Cold as i32));
    // The deprecated flag is still sent, so a 1.18 server keeps behaving.
    assert_eq!(params.on_disk, Some(true));
    assert_eq!(
        params.hnsw_config.as_ref().unwrap().memory,
        Some(QMemory::Cached as i32)
    );

    let sparse = revector::convert::sparse_vectors_config(spec).unwrap();
    let index = sparse.map.get("text").unwrap().index.as_ref().unwrap();
    assert_eq!(index.memory, Some(QMemory::Cold as i32));

    match revector::convert::quantization_oneof(spec.quantization_config.as_ref().unwrap()).unwrap()
    {
        Quantization::Scalar(s) => {
            assert_eq!(s.memory, Some(QMemory::Pinned as i32));
            assert_eq!(s.always_ram, Some(true));
        }
        other => panic!("expected scalar, got {other:?}"),
    }

    let payload = revector::convert::payload_storage_params(spec.payload.as_ref().unwrap());
    assert_eq!(payload.memory, Some(QMemory::Cold as i32));
}

/// Components with no declared placement send nothing, leaving the server free
/// to apply its own default — the same "unset means don't care" rule the rest
/// of the spec follows.
#[test]
fn omitted_memory_stays_unset() {
    let spec: CollectionSpec = serde_yaml::from_str(
        r#"
vectors:
  "":
    size: 4
    distance: Dot
"#,
    )
    .unwrap();
    let params = revector::convert::vector_params(spec.vectors.get("").unwrap());
    assert_eq!(params.memory, None);
    assert!(spec.payload.is_none());
}

/// TurboQuant 4-bit primary storage (Qdrant 1.19) is selectable as a datatype.
#[test]
fn turbo4_datatype_converts() {
    use qdrant_client::qdrant::Datatype as QDatatype;

    let spec: CollectionSpec = serde_yaml::from_str(
        r#"
vectors:
  "":
    size: 1536
    distance: Cosine
    datatype: turbo4
"#,
    )
    .unwrap();
    let params = revector::convert::vector_params(spec.vectors.get("").unwrap());
    assert_eq!(params.datatype, Some(QDatatype::Turbo4 as i32));
}

/// `update_collection` can move an existing vector and the payload store
/// between memory tiers.
#[test]
fn update_collection_carries_memory_and_payload() {
    use qdrant_client::qdrant::Memory as QMemory;

    let m = mig(r#"
revision: "0001"
down_revision: null
description: retier
up:
  - op: update_collection
    collection: docs
    vectors:
      "":
        memory: cached
    payload:
      memory: cold
"#);

    match &m.file.up[0] {
        Operation::UpdateCollection(op) => {
            let diff = revector::convert::vector_params_diff(
                op.vectors.as_ref().unwrap().get("").unwrap(),
            );
            assert_eq!(diff.memory, Some(QMemory::Cached as i32));
            let params =
                revector::convert::collection_params_diff(None, None, None, op.payload.as_ref());
            assert_eq!(
                params.payload.and_then(|p| p.memory),
                Some(QMemory::Cold as i32)
            );
        }
        other => panic!("expected update_collection, got {other:?}"),
    }
}

/// A keyword index can opt into prefix matching (Qdrant 1.19); the params ride
/// along into the auto-generated inverse so a downgrade→upgrade round trip
/// recreates the same index.
#[test]
fn keyword_prefix_params_convert_and_round_trip() {
    use qdrant_client::qdrant::{payload_index_params::IndexParams, Memory as QMemory};
    use revector::spec::PayloadSchemaType;

    let m = mig(r#"
revision: "0001"
down_revision: null
description: prefix index
up:
  - op: create_payload_index
    collection: products
    field_name: sku
    schema: keyword
    params:
      prefix: true
      is_tenant: true
      memory: cached
"#);

    let Operation::CreatePayloadIndex { params, .. } = &m.file.up[0] else {
        panic!("expected create_payload_index");
    };
    let params = params.as_ref().expect("params declared");
    match revector::convert::payload_index_params(PayloadSchemaType::Keyword, params) {
        IndexParams::KeywordIndexParams(k) => {
            assert!(k.prefix.is_some(), "prefix matching should be enabled");
            assert_eq!(k.is_tenant, Some(true));
            assert_eq!(k.memory, Some(QMemory::Cached as i32));
        }
        other => panic!("expected keyword params, got {other:?}"),
    }

    // down: drop the index, carrying schema *and* params …
    let down = m.downgrade_ops().unwrap();
    let Operation::DeletePayloadIndex { schema, params, .. } = &down[0] else {
        panic!("expected delete_payload_index");
    };
    assert_eq!(*schema, Some(PayloadSchemaType::Keyword));
    assert_eq!(params.as_ref().unwrap().prefix, Some(true));

    // … so inverting again yields the original create, params included.
    match down[0].auto_inverse() {
        Reversibility::Auto(op) => assert_eq!(*op, m.file.up[0]),
        Reversibility::Irreversible(reason) => panic!("should be reversible: {reason}"),
    }
}

/// `prefix: false` means "don't enable it" — the marker message is what turns
/// prefix matching on, so sending an empty one would silently enable it.
#[test]
fn prefix_false_does_not_enable_prefix_matching() {
    use qdrant_client::qdrant::payload_index_params::IndexParams;
    use revector::spec::{PayloadIndexParamsSpec, PayloadSchemaType};

    let params = PayloadIndexParamsSpec {
        prefix: Some(false),
        ..Default::default()
    };
    match revector::convert::payload_index_params(PayloadSchemaType::Keyword, &params) {
        IndexParams::KeywordIndexParams(k) => assert!(k.prefix.is_none()),
        other => panic!("expected keyword params, got {other:?}"),
    }
}

/// Text indexes reach the 1.19 "no stemming" marker through `stemmer: disabled`.
#[test]
fn text_index_stemmer_disabled_converts() {
    use qdrant_client::qdrant::{
        payload_index_params::IndexParams, stemming_algorithm::StemmingParams, TokenizerType,
    };
    use revector::spec::PayloadSchemaType;

    let m = mig(r#"
revision: "0001"
down_revision: null
description: text index
up:
  - op: create_payload_index
    collection: docs
    field_name: body
    schema: text
    params:
      tokenizer: word
      lowercase: true
      stemmer: disabled
      stopwords:
        languages: [english]
"#);

    let Operation::CreatePayloadIndex { params, .. } = &m.file.up[0] else {
        panic!("expected create_payload_index");
    };
    match revector::convert::payload_index_params(PayloadSchemaType::Text, params.as_ref().unwrap())
    {
        IndexParams::TextIndexParams(t) => {
            assert_eq!(t.tokenizer, TokenizerType::Word as i32);
            assert_eq!(t.lowercase, Some(true));
            assert!(matches!(
                t.stemmer.unwrap().stemming_params.unwrap(),
                StemmingParams::Disabled(_)
            ));
            assert_eq!(t.stopwords.unwrap().languages, vec!["english".to_string()]);
        }
        other => panic!("expected text params, got {other:?}"),
    }
}

/// Params that don't apply to the declared field type are a validation error,
/// not a silently dropped setting.
#[test]
fn payload_index_params_are_checked_against_the_field_type() {
    use revector::spec::{PayloadIndexParamsSpec, PayloadSchemaType};

    let prefix_only = PayloadIndexParamsSpec {
        prefix: Some(true),
        ..Default::default()
    };
    let err = prefix_only
        .validate_for(PayloadSchemaType::Integer)
        .unwrap_err()
        .to_string();
    assert!(err.contains("prefix") && err.contains("integer"), "{err}");
    assert!(prefix_only.validate_for(PayloadSchemaType::Keyword).is_ok());

    // Qdrant's text params carry the tokenizer as a required field.
    let no_tokenizer = PayloadIndexParamsSpec {
        lowercase: Some(true),
        ..Default::default()
    };
    let err = no_tokenizer
        .validate_for(PayloadSchemaType::Text)
        .unwrap_err()
        .to_string();
    assert!(err.contains("tokenizer"), "{err}");
}

/// Bad params fail when the file is parsed, so `revector validate` catches them
/// offline instead of halfway through an `up`.
#[test]
fn invalid_index_params_are_rejected_at_parse_time() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("0001.yaml");
    std::fs::write(
        &path,
        r#"
revision: "0001"
down_revision: null
description: bogus params
up:
  - op: create_payload_index
    collection: products
    field_name: price
    schema: float
    params:
      is_tenant: true
"#,
    )
    .unwrap();

    let err = Migration::from_path(&path).unwrap_err().to_string();
    assert!(err.contains("is_tenant") && err.contains("0001"), "{err}");
}

/// Unknown keys inside `params` are rejected rather than ignored — a typo in a
/// tuning knob should never look like it applied.
#[test]
fn unknown_index_params_are_rejected() {
    let yaml = r#"
revision: "0001"
down_revision: null
up:
  - op: create_payload_index
    collection: products
    field_name: sku
    schema: keyword
    params:
      prefixx: true
"#;
    let err = serde_yaml::from_str::<MigrationFile>(yaml)
        .unwrap_err()
        .to_string();
    assert!(err.contains("prefixx"), "{err}");
}

// --- Minimum server version -------------------------------------------------

/// Helper: the requirements a migration's `up` ops carry, as (version, feature).
fn up_requirements(m: &Migration) -> Vec<(String, String)> {
    m.file
        .up
        .iter()
        .flat_map(|op| op.version_requirements())
        .map(|r| (r.version.to_string(), r.feature))
        .collect()
}

/// Declaring a 1.19-only field raises the minimum server version, so the runner
/// can refuse instead of letting the field be dropped on the wire.
#[test]
fn memory_placement_requires_a_1_19_server() {
    let m = mig(r#"
revision: "0001"
down_revision: null
up:
  - op: create_collection
    name: docs
    spec:
      vectors:
        "":
          size: 8
          distance: Dot
          memory: cold
"#);
    let reqs = up_requirements(&m);
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].0, "1.19.0");
    assert!(reqs[0].1.contains("memory"), "{:?}", reqs[0]);
}

/// The TurboQuant datatype landed server-side in 1.18.2, ahead of the rest of
/// the 1.19 surface — the requirement tracks the server, not the client crate.
#[test]
fn turbo4_requires_1_18_2_not_1_19() {
    let m = mig(r#"
revision: "0001"
down_revision: null
up:
  - op: create_collection
    name: docs
    spec:
      vectors:
        "":
          size: 8
          distance: Dot
          datatype: turbo4
"#);
    assert_eq!(up_requirements(&m)[0].0, "1.18.2");
}

/// A migration that declares nothing new imposes nothing, so it still runs
/// against older servers.
#[test]
fn plain_migrations_have_no_version_floor() {
    assert!(up_requirements(&mig(M1)).is_empty());
    assert!(up_requirements(&mig(M2)).is_empty());
}

/// Only what reaches the wire counts. Qdrant's add-vector API takes neither
/// placement nor tuning — revector warns and drops them — so they must not
/// raise the floor for a `create_vector` step.
#[test]
fn create_vector_ignores_fields_the_api_cannot_accept() {
    let m = mig(r#"
revision: "0001"
down_revision: null
up:
  - op: create_vector
    collection: products
    name: image
    spec:
      size: 8
      distance: Dot
      memory: cold
      hnsw_config:
        memory: cached
"#);
    assert!(
        up_requirements(&m).is_empty(),
        "dropped fields must not gate the server version: {:?}",
        up_requirements(&m)
    );
}

/// Same rule for a payload-index delete: its `params` exist only to rebuild the
/// index on the way back up, and are never sent by the delete itself.
#[test]
fn delete_payload_index_params_do_not_gate_the_server() {
    let m = mig(r#"
revision: "0001"
down_revision: null
up:
  - op: delete_payload_index
    collection: products
    field_name: sku
    schema: keyword
    params:
      prefix: true
      memory: cached
"#);
    assert!(up_requirements(&m).is_empty());

    // But recreating it on rollback does need the newer server.
    let down = m.downgrade_ops().unwrap();
    let reqs: Vec<_> = down[0].version_requirements();
    assert_eq!(reqs.len(), 2, "{reqs:?}");
    assert!(reqs.iter().all(|r| r.version.to_string() == "1.19.0"));
}

/// `update_collection` sends its diff whole, so every 1.19 field in it counts.
#[test]
fn update_collection_requirements_cover_vectors_and_payload() {
    let m = mig(r#"
revision: "0001"
down_revision: null
up:
  - op: update_collection
    collection: docs
    vectors:
      image:
        memory: cached
    payload:
      memory: cold
"#);
    let reqs = up_requirements(&m);
    assert_eq!(reqs.len(), 2, "{reqs:?}");
    assert!(reqs.iter().any(|(_, f)| f.contains("vectors.image")));
    assert!(reqs.iter().any(|(_, f)| f.contains("payload")));
}
