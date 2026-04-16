/// Task 10 gap closure — SearchProviderConfig tests (always-on, no model files required).
use std::path::PathBuf;

use ail_search::SearchProviderConfig;

#[test]
fn t114_provider_config_none_constructed() {
    let cfg = SearchProviderConfig::None;
    assert_eq!(cfg, SearchProviderConfig::None);
}

#[test]
fn t114_provider_config_local_onnx_stores_path() {
    let dir = PathBuf::from("/home/user/.ail/models/all-MiniLM-L6-v2");
    let cfg = SearchProviderConfig::LocalOnnx {
        model_dir: dir.clone(),
    };
    let SearchProviderConfig::LocalOnnx { model_dir } = cfg else {
        panic!("expected LocalOnnx");
    };
    assert_eq!(model_dir, dir);
}

#[test]
fn t114_provider_config_openai_stores_key() {
    let cfg = SearchProviderConfig::OpenAi {
        api_key: "sk-test-key".to_string(),
    };
    let SearchProviderConfig::OpenAi { api_key } = cfg else {
        panic!("expected OpenAi");
    };
    assert_eq!(api_key, "sk-test-key");
}

#[test]
fn t114_provider_config_debug_does_not_panic() {
    let none_str = format!("{:?}", SearchProviderConfig::None);
    assert!(none_str.contains("None"));

    let onnx_str = format!(
        "{:?}",
        SearchProviderConfig::LocalOnnx {
            model_dir: PathBuf::from("~/.ail/models")
        }
    );
    assert!(onnx_str.contains("LocalOnnx"));

    let openai_str = format!(
        "{:?}",
        SearchProviderConfig::OpenAi {
            api_key: "sk-real-secret-key-12345".to_string()
        }
    );
    assert!(openai_str.contains("OpenAi"));
    assert!(openai_str.contains("REDACTED"), "must redact the key");
    assert!(
        !openai_str.contains("sk-real-secret-key-12345"),
        "must NOT contain the actual key"
    );
}
