use std::env;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;

use crate::errors::SearchError;
use crate::provider::EmbeddingProvider;

/// Embedding dimensions for `all-MiniLM-L6-v2`.
pub const DIMENSION: usize = 384;

/// Provider name used in `EmbeddingProvider::name()` and for index model tracking.
///
/// Exported so callers (e.g., `ail status`, `SqliteGraph::check_embedding_model`)
/// can compare against the stored model name without importing the ONNX provider.
pub const DEFAULT_MODEL_NAME: &str = "onnx/all-MiniLM-L6-v2";

/// Model subdirectory name under `~/.ail/models/`.
const MODEL_DIR_NAME: &str = "all-MiniLM-L6-v2";

/// Expected filenames inside the model directory.
const MODEL_FILE: &str = "model.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";

/// Local ONNX-backed embedding provider using `all-MiniLM-L6-v2`.
///
/// ## Model files
///
/// Place both files from the HuggingFace repo
/// `sentence-transformers/all-MiniLM-L6-v2` at:
/// ```text
/// ~/.ail/models/all-MiniLM-L6-v2/model.onnx
/// ~/.ail/models/all-MiniLM-L6-v2/tokenizer.json
/// ```
///
/// The `tokenizer.json` **must** be from the same HuggingFace repo as the
/// ONNX export — mixing tokenizer files from other sources produces wrong
/// embeddings (review issue 10.1-C).
///
/// ## Fallback
///
/// If the model is absent, [`OnnxEmbeddingProvider::ensure_model`] returns
/// [`SearchError::ModelNotFound`] which callers may catch to fall back to
/// BM25-only search.
pub struct OnnxEmbeddingProvider {
    /// Session is wrapped in `Mutex` because `Session::run` takes `&mut self`.
    session: Mutex<Session>,
    tokenizer: tokenizers::Tokenizer,
}

impl OnnxEmbeddingProvider {
    /// Load the ONNX session and tokenizer from a model directory.
    ///
    /// `model_dir` must contain both `model.onnx` and `tokenizer.json`.
    /// Call [`ensure_model`] first to verify the files exist.
    pub fn new(model_dir: &Path) -> Result<Self, SearchError> {
        let model_path = model_dir.join(MODEL_FILE);
        let tokenizer_path = model_dir.join(TOKENIZER_FILE);

        if !model_path.exists() {
            return Err(SearchError::ModelNotFound {
                path: model_path,
                hint: model_not_found_hint(),
            });
        }
        if !tokenizer_path.exists() {
            return Err(SearchError::ModelNotFound {
                path: tokenizer_path,
                hint: model_not_found_hint(),
            });
        }

        let session = Session::builder()
            .map_err(|e| SearchError::ModelLoadFailed(e.to_string()))?
            .commit_from_file(&model_path)
            .map_err(|e| SearchError::ModelLoadFailed(e.to_string()))?;

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| SearchError::ModelLoadFailed(e.to_string()))?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
        })
    }

    /// Return the default model directory and verify both model files are present.
    ///
    /// Default path: `~/.ail/models/all-MiniLM-L6-v2/`
    ///
    /// Returns [`SearchError::ModelNotFound`] with an actionable hint when
    /// either `model.onnx` or `tokenizer.json` is absent. Does **not**
    /// download the model — automatic download is handled by the CLI
    /// `ail search --setup` command (future task).
    pub fn ensure_model() -> Result<PathBuf, SearchError> {
        let dir = default_model_dir();
        let model_path = dir.join(MODEL_FILE);
        let tokenizer_path = dir.join(TOKENIZER_FILE);

        if !model_path.exists() {
            return Err(SearchError::ModelNotFound {
                path: model_path,
                hint: model_not_found_hint(),
            });
        }
        if !tokenizer_path.exists() {
            return Err(SearchError::ModelNotFound {
                path: tokenizer_path,
                hint: model_not_found_hint(),
            });
        }

        Ok(dir)
    }
}

/// Return `~/.ail/models/all-MiniLM-L6-v2/`.
///
/// Checks `HOME` (Unix) then `USERPROFILE` (Windows); falls back to `"."`.
fn default_model_dir() -> PathBuf {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".ail")
        .join("models")
        .join(MODEL_DIR_NAME)
}

fn model_not_found_hint() -> String {
    let dir = default_model_dir();
    format!(
        "Place model.onnx and tokenizer.json from \
         https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2 \
         at '{}'. Run `ail search --setup` to automate the download.",
        dir.display()
    )
}

impl EmbeddingProvider for OnnxEmbeddingProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError> {
        let encoding = self
            .tokenizer
            .encode(text, /*add_special_tokens=*/ true)
            .map_err(|e| SearchError::TokenizationFailed(e.to_string()))?;

        let seq_len = encoding.get_ids().len();

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();

        let input_ids = Tensor::<i64>::from_array(([1usize, seq_len], ids))
            .map_err(|e| SearchError::InferenceFailed(e.to_string()))?;
        let attn_mask = Tensor::<i64>::from_array(([1usize, seq_len], mask))
            .map_err(|e| SearchError::InferenceFailed(e.to_string()))?;
        let token_type_ids = Tensor::<i64>::from_array(([1usize, seq_len], type_ids))
            .map_err(|e| SearchError::InferenceFailed(e.to_string()))?;

        // Run inference; hold the mutex guard alive until outputs are consumed.
        let flat: Vec<f32> = {
            let inputs = ort::inputs![
                "input_ids"      => input_ids,
                "attention_mask" => attn_mask,
                "token_type_ids" => token_type_ids,
            ];
            let mut session = self
                .session
                .lock()
                .map_err(|e| SearchError::InferenceFailed(format!("session lock: {e}")))?;
            let outputs = session
                .run(inputs)
                .map_err(|e| SearchError::InferenceFailed(e.to_string()))?;

            // Output 0 is last_hidden_state [1, seq_len, DIMENSION].
            // try_extract_tensor returns (&Shape, &[f32]); copy while outputs is in scope.
            let (_shape, data) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| SearchError::InferenceFailed(e.to_string()))?;
            data.to_vec()
            // data, outputs, then session (mutex guard) drop here in reverse order.
        };

        mean_pool_and_normalize(&flat, seq_len)
    }

    /// Override the default `embed_batch` for explicit per-text inference.
    ///
    /// True batching (padding + single forward pass) is a future optimisation;
    /// correctness takes priority at this stage.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    fn dimension(&self) -> usize {
        DIMENSION
    }

    fn name(&self) -> &str {
        DEFAULT_MODEL_NAME
    }
}

/// Mean-pool the last hidden state and L2-normalise the result.
///
/// `flat` is the raw output tensor in row-major layout `[1, seq_len, DIMENSION]`.
/// We average across the `seq_len` dimension, then L2-normalise so that
/// cosine similarity equals the dot product.
fn mean_pool_and_normalize(flat: &[f32], seq_len: usize) -> Result<Vec<f32>, SearchError> {
    if flat.len() != seq_len * DIMENSION {
        return Err(SearchError::InferenceFailed(format!(
            "unexpected output size {} (expected seq_len={} × dim={}={})",
            flat.len(),
            seq_len,
            DIMENSION,
            seq_len * DIMENSION,
        )));
    }

    let mut pooled = vec![0.0f32; DIMENSION];
    for token_idx in 0..seq_len {
        let offset = token_idx * DIMENSION;
        for dim_idx in 0..DIMENSION {
            pooled[dim_idx] += flat[offset + dim_idx];
        }
    }
    let denom = seq_len as f32;
    for x in &mut pooled {
        *x /= denom;
    }

    // L2 normalise — after this, cosine similarity == dot product.
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut pooled {
            *x /= norm;
        }
    }

    Ok(pooled)
}
