//! Specifies the chat store (#97): messages come back in order with what answers included,
//! they are sealed on disk, old ones are trimmed, and deleting a meeting deletes its chat.

use std::sync::Arc;

use chrono::{TimeZone, Utc};

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        chat::{ChatMessage, ChatRole, ChatScope, MAX_CHAT_MESSAGES},
        models::{DeviceId, MeetingId},
    },
    ports::{chat::ChatHistoryRepository, storage::NoteRepository},
};

fn repository(path: &std::path::Path) -> SqliteNoteRepository {
    SqliteNoteRepository::open(
        path,
        DeviceId::new("device"),
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
    )
    .and_then(|repository| repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec)))
    .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn messages_come_back_in_order_sealed_on_disk() {
    let folder = std::env::temp_dir().join(format!("chat-store-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let path = folder.join("db.sqlite3");
    let store = repository(&path);
    let at = Utc
        .with_ymd_and_hms(2026, 11, 9, 12, 0, 0)
        .single()
        .unwrap_or_default();
    let scope = ChatScope::All;
    let reply = ChatMessage {
        role: ChatRole::Assistant,
        text: String::from("The budget was approved."),
        created_at: at,
        included: vec![String::from("Budget review")],
        omitted: vec![String::from("Old meeting")],
        connection_label: Some(String::from("Office")),
        sources: vec![crate::domain::chat::ChatSource {
            number: 1,
            label: String::from("Budget review"),
            target: Some(crate::domain::chat::ChatTarget::Meeting {
                meeting_id: String::from("m-1"),
            }),
            at_ms: Some(61_000),
        }],
    };
    store
        .append_chat_messages(
            &scope,
            &[
                ChatMessage::question("Was the budget approved?", at),
                reply.clone(),
            ],
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let history = store
        .chat_history(&scope)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].text, "Was the budget approved?");
    assert_eq!(history[1], reply);
    assert!(
        store
            .chat_history(&ChatScope::Meeting(MeetingId::new("m")))
            .unwrap_or_default()
            .is_empty()
    );
    drop(store);
    let bytes = std::fs::read(&path).unwrap_or_default();
    assert!(
        !bytes.windows(6).any(|window| window == b"budget"),
        "the text is sealed on disk"
    );
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn old_messages_are_trimmed_and_chats_are_cleared() {
    let store = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let at = Utc::now();
    let scope = ChatScope::All;
    let many: Vec<ChatMessage> = (0..MAX_CHAT_MESSAGES + 5)
        .map(|index| ChatMessage::question(&format!("q{index}"), at))
        .collect();
    store
        .append_chat_messages(&scope, &many)
        .unwrap_or_else(|error| panic!("{error}"));
    let history = store
        .chat_history(&scope)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(history.len(), MAX_CHAT_MESSAGES);
    assert_eq!(history[0].text, "q5");
    assert!(store.clear_chat(&scope).unwrap_or_default());
    assert!(store.chat_history(&scope).unwrap_or_default().is_empty());
}

#[test]
fn deleting_a_meeting_deletes_its_chat() {
    let store = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .and_then(|repository| repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec)))
    .unwrap_or_else(|error| panic!("{error}"));
    let meeting_id = MeetingId::new("m-1");
    MeetingCrdtService::new(&store)
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("{error}"));
    let scope = ChatScope::Meeting(meeting_id.clone());
    store
        .append_chat_messages(&scope, &[ChatMessage::question("Hi?", Utc::now())])
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(store.delete_meeting(&meeting_id).unwrap_or_default());
    assert!(store.chat_history(&scope).unwrap_or_default().is_empty());
}
