//! The context an app is given, without a window.

use std::sync::Arc;

use isonomic_app::{AppContext, Attention, MemorySettings, SettingsStore};
use isonomic_proto::Token;
use isonomic_testkit::TestEngine;

#[tokio::test(flavor = "multi_thread")]
async fn attention_reaches_the_window_and_settings_stay_per_app() {
    let engine = TestEngine::new();
    let store = Arc::new(MemorySettings::default());
    let client = engine.client().scoped(Token::new("t"));
    let (notes, mut rail) = AppContext::new("com.isonomic.notes", client.clone(), store.clone());
    let (chat, _) = AppContext::new("com.isonomic.chat", client, store.clone());

    assert_eq!(notes.id(), "com.isonomic.notes");
    assert_eq!(*rail.borrow(), Attention::None);
    notes.set_attention(Attention::Count(3));
    rail.changed().await.unwrap();
    assert_eq!(*rail.borrow_and_update(), Attention::Count(3));
    assert_eq!(notes.clone().attention(), Attention::Count(3));

    notes.settings().set("width", &320u32);
    chat.settings().set("width", &640u32);
    assert_eq!(notes.settings().get::<u32>("width"), Some(320));
    assert_eq!(chat.settings().get::<u32>("width"), Some(640));
    assert_eq!(
        notes.settings().get::<String>("width"),
        None,
        "wrong type reads as absent"
    );
    assert_eq!(
        store.get("com.isonomic.notes", "width"),
        Some(serde_json::json!(320))
    );
}
