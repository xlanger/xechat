use xechat::services::conversation_store::ConversationStore;

const TEST_LIMIT: usize = 1000;

async fn open_store() -> (ConversationStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();

    // 初始化向量存储（turns 表）
    let mut vs = xechat::services::vector_store::lancedb_store::LanceDbStore::open(path)
        .await
        .unwrap();
    vs.ensure_table().await.unwrap();
    let vector_store: std::sync::Arc<dyn xechat::services::vector_store::VectorStore> = std::sync::Arc::new(vs);

    let mut store = ConversationStore::open(path, Some(vector_store))
        .await
        .unwrap();
    store.ensure_table().await.unwrap();
    (store, dir)
}

#[tokio::test]
async fn test_create_and_load_conversation() {
    let (store, _dir) = open_store().await;

    let conv = store.create_conversation("Test Chat").await.unwrap();
    assert_eq!(conv.title, "Test Chat");
    assert!(!conv.id.is_empty());

    let loaded = store.load_conversation_by_id(&conv.id, TEST_LIMIT).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().title, "Test Chat");
}

#[tokio::test]
async fn test_add_message() {
    let (store, _dir) = open_store().await;

    let conv = store.create_conversation("Chat").await.unwrap();
    let msg = xechat::Message::new_user("Hello".into());
    store.add_message(&conv.id, &msg).await.unwrap();

    let loaded = store.load_conversation_by_id(&conv.id, TEST_LIMIT).await.unwrap().unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "Hello");
}

#[tokio::test]
async fn test_rename_conversation() {
    let (store, _dir) = open_store().await;

    let conv = store.create_conversation("Original").await.unwrap();
    store.rename_conversation(&conv.id, "Renamed").await.unwrap();

    let loaded = store.load_conversation_by_id(&conv.id, TEST_LIMIT).await.unwrap().unwrap();
    assert_eq!(loaded.title, "Renamed");
}

#[tokio::test]
async fn test_delete_conversation() {
    let (store, _dir) = open_store().await;

    let conv = store.create_conversation("To Delete").await.unwrap();
    store.delete_conversation(&conv.id).await.unwrap();

    let loaded = store.load_conversation_by_id(&conv.id, TEST_LIMIT).await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_load_conversation_list() {
    let (store, _dir) = open_store().await;

    store.create_conversation("Chat 1").await.unwrap();
    store.create_conversation("Chat 2").await.unwrap();

    let list = store.load_conversation_list(TEST_LIMIT).await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn test_update_message_content() {
    let (store, _dir) = open_store().await;

    let conv = store.create_conversation("Chat").await.unwrap();
    let msg = xechat::Message::new_assistant();
    let msg_id = msg.id.clone();
    store.add_message(&conv.id, &msg).await.unwrap();

    store.update_message_content(&conv.id, &msg_id, "Updated").await.unwrap();

    let loaded = store.load_conversation_by_id(&conv.id, TEST_LIMIT).await.unwrap().unwrap();
    assert_eq!(loaded.messages[0].content, "Updated");
}

#[tokio::test]
async fn test_conversation_exists() {
    let (store, _dir) = open_store().await;

    let conv = store.create_conversation("Chat").await.unwrap();
    assert!(store.conversation_exists(&conv.id).await);
    assert!(!store.conversation_exists("nonexistent").await);
}
