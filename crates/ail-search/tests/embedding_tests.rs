/// Task 10.1 — Embedding Provider Interface tests.
///
/// Tests that require the actual ONNX model (`~/.ail/models/all-MiniLM-L6-v2/`)
/// are marked `#[ignore]` with a clear reason.  Running
/// `cargo test -p ail-search -- --ignored` after placing the model files will
/// exercise the full suite.
///
/// The one always-on test (`t101_provider_trait_works_with_dyn`) verifies that
/// the trait itself is object-safe and that the default `embed_batch` impl
/// works correctly against a mock provider.
use ail_search::{EmbeddingProvider, SearchError};

// ─── Mock provider (used by always-on test) ─────────────────────────────────

/// Trivial fixed-vector provider for trait-level tests.
struct ConstantProvider {
    dim: usize,
    value: f32,
    provider_name: &'static str,
}

impl EmbeddingProvider for ConstantProvider {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, SearchError> {
        Ok(vec![self.value; self.dim])
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn name(&self) -> &str {
        self.provider_name
    }
}

// ─── Always-on ───────────────────────────────────────────────────────────────

/// Verify that `EmbeddingProvider` is object-safe (can be held as
/// `Box<dyn EmbeddingProvider>`) and that the default `embed_batch`
/// implementation delegates to `embed` correctly.
#[test]
fn t101_provider_trait_works_with_dyn() {
    let provider: Box<dyn EmbeddingProvider> = Box::new(ConstantProvider {
        dim: 4,
        value: 0.5,
        provider_name: "test/constant",
    });

    // Single embed
    let v = provider.embed("anything").unwrap();
    assert_eq!(v.len(), 4);
    assert!(v.iter().all(|&x| (x - 0.5).abs() < 1e-6));

    // Default embed_batch
    let batch = provider.embed_batch(&["a", "b", "c"]).unwrap();
    assert_eq!(batch.len(), 3);
    for row in &batch {
        assert_eq!(row.len(), 4);
    }

    // dimension / name accessors
    assert_eq!(provider.dimension(), 4);
    assert_eq!(provider.name(), "test/constant");
}

// ─── Ignored (require model files at ~/.ail/models/all-MiniLM-L6-v2/) ───────

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires model.onnx and tokenizer.json at ~/.ail/models/all-MiniLM-L6-v2/"]
fn t101_local_provider_loads_model() {
    use ail_search::OnnxEmbeddingProvider;

    let model_dir = OnnxEmbeddingProvider::ensure_model()
        .expect("ensure_model should succeed when model files are present");
    let _provider =
        OnnxEmbeddingProvider::new(&model_dir).expect("new() should succeed with valid model dir");
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires model.onnx and tokenizer.json at ~/.ail/models/all-MiniLM-L6-v2/"]
fn t101_local_provider_embeds_single_text() {
    use ail_search::OnnxEmbeddingProvider;

    let model_dir = OnnxEmbeddingProvider::ensure_model().unwrap();
    let provider = OnnxEmbeddingProvider::new(&model_dir).unwrap();

    let vec = provider.embed("transfer money safely").unwrap();
    assert_eq!(vec.len(), 384, "embedding must have 384 dimensions");
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires model.onnx and tokenizer.json at ~/.ail/models/all-MiniLM-L6-v2/"]
fn t101_local_provider_embeds_batch() {
    use ail_search::OnnxEmbeddingProvider;

    let model_dir = OnnxEmbeddingProvider::ensure_model().unwrap();
    let provider = OnnxEmbeddingProvider::new(&model_dir).unwrap();

    let texts = ["transfer money", "validate input"];
    let batch = provider.embed_batch(&texts).unwrap();

    assert_eq!(batch.len(), 2, "one embedding per input text");
    for row in &batch {
        assert_eq!(row.len(), 384);
    }
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires model.onnx and tokenizer.json at ~/.ail/models/all-MiniLM-L6-v2/"]
fn t101_local_provider_dimension_correct() {
    use ail_search::OnnxEmbeddingProvider;

    let model_dir = OnnxEmbeddingProvider::ensure_model().unwrap();
    let provider = OnnxEmbeddingProvider::new(&model_dir).unwrap();

    assert_eq!(provider.dimension(), 384);
    assert_eq!(provider.name(), "onnx/all-MiniLM-L6-v2");
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires model.onnx and tokenizer.json at ~/.ail/models/all-MiniLM-L6-v2/"]
fn t101_local_provider_similar_texts_high_cosine() {
    use ail_search::OnnxEmbeddingProvider;

    let model_dir = OnnxEmbeddingProvider::ensure_model().unwrap();
    let provider = OnnxEmbeddingProvider::new(&model_dir).unwrap();

    let a = provider.embed("transfer money safely").unwrap();
    let b = provider.embed("send funds to recipient").unwrap();
    let sim = cosine_similarity(&a, &b);

    assert!(
        sim > 0.7,
        "semantically similar texts should have cosine > 0.7, got {sim:.3}"
    );
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires model.onnx and tokenizer.json at ~/.ail/models/all-MiniLM-L6-v2/"]
fn t101_local_provider_different_texts_low_cosine() {
    use ail_search::OnnxEmbeddingProvider;

    let model_dir = OnnxEmbeddingProvider::ensure_model().unwrap();
    let provider = OnnxEmbeddingProvider::new(&model_dir).unwrap();

    let a = provider.embed("transfer money safely").unwrap();
    let b = provider.embed("compile GLSL shader program").unwrap();
    let sim = cosine_similarity(&a, &b);

    assert!(
        sim < 0.3,
        "semantically dissimilar texts should have cosine < 0.3, got {sim:.3}"
    );
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "integration — requires OPENAI_API_KEY; OpenAI provider deferred to hardening task"]
fn t101_cloud_provider_embeds_text() {
    todo!("OpenAI provider deferred to hardening task after HTTP-client choice is settled")
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires network + write access to ~/.ail/models/; automatic download not in 10.1"]
fn t101_model_auto_download_on_first_use() {
    use ail_search::OnnxEmbeddingProvider;
    use std::env;

    // Override HOME to a temp dir that has no model files.
    // ensure_model() should return ModelNotFound with a hint.
    let tmp = std::env::temp_dir().join("ail_test_no_model");
    env::set_var("HOME", &tmp);
    env::set_var("USERPROFILE", &tmp);

    let err = OnnxEmbeddingProvider::ensure_model()
        .expect_err("should fail when model is absent from temp dir");

    match err {
        SearchError::ModelNotFound { hint, .. } => {
            assert!(
                hint.contains("ail search --setup"),
                "hint should mention `ail search --setup`, got: {hint}"
            );
        }
        other => panic!("expected ModelNotFound, got: {other}"),
    }
}

#[test]
#[cfg(feature = "embeddings")]
#[ignore = "requires model.onnx and tokenizer.json at ~/.ail/models/all-MiniLM-L6-v2/"]
fn t101_model_reuses_cached_model() {
    use ail_search::OnnxEmbeddingProvider;

    // Two successive ensure_model() calls should return the same directory.
    let path1 = OnnxEmbeddingProvider::ensure_model().unwrap();
    let path2 = OnnxEmbeddingProvider::ensure_model().unwrap();
    assert_eq!(path1, path2, "ensure_model() must be idempotent");
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Cosine similarity between two L2-normalised vectors.
#[cfg(feature = "embeddings")]
///
/// Both vectors are expected to already be L2-normalised (as produced by
/// `OnnxEmbeddingProvider`), so this is equivalent to their dot product.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "vector dimensions must match");
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
