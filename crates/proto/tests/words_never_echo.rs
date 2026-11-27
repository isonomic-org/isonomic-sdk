// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! A frame that does not decode is refused without its content: no error
//! message quotes what was sent, however the frame is broken, so the
//! person's recovery words can never come back in one.
//!
//! The words are the word-list standard's public all-zero test vector.

use isonomic_proto::identity::RecoveryWords;
use isonomic_proto::lenient::{decode_client, FromClient};

const WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

fn message(frame: &str) -> String {
    match decode_client(frame.as_bytes()) {
        FromClient::Rejected { error, .. } | FromClient::Garbage(error) => error.message,
        FromClient::Message(message) => panic!("decoded: {message:?}"),
    }
}

fn quoted() -> String {
    let list: Vec<String> = WORDS.split(' ').map(|w| format!("\"{w}\"")).collect();
    format!("[{}]", list.join(","))
}

#[test]
fn no_malformed_restore_echoes_the_words() {
    let list = quoted();
    let frames = [
        // `words` as one string, the reviewer's case.
        format!(r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":"{{}}","words":"{WORDS}","replace_unused":false}}}}"#),
        // `params` as one string.
        format!(r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":"{WORDS}"}}"#),
        // A word that is not a string, beside real ones.
        r#"{"type":"request","id":7,"method":"identity.kit_restore","params":{"kit":"{}","words":["abandon",5,"art"],"replace_unused":false}}"#.to_string(),
        // `kit` the wrong type, words in the right place.
        format!(r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":5,"words":{list},"replace_unused":false}}}}"#),
        // The words typed into `kit`, and `replace_unused` the wrong type.
        format!(r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":"{WORDS}","words":{list},"replace_unused":"{WORDS}"}}}}"#),
        // No id, so not answerable by id.
        format!(r#"{{"type":"request","method":"identity.kit_restore","params":{{"kit":"{{}}","words":"{WORDS}","replace_unused":false}}}}"#),
        // Words where an export's flag belongs.
        format!(r#"{{"type":"request","id":7,"method":"identity.kit_export","params":{{"person_asked":"{WORDS}"}}}}"#),
        // A nested list.
        format!(r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":"{{}}","words":[{list}],"replace_unused":false}}}}"#),
    ];
    for frame in &frames {
        let message = message(frame);
        for word in ["abandon", "art"] {
            assert!(!message.contains(word), "{message}");
        }
    }
}

/// The words field alone is safe, even through the strict decoder and the
/// tagged enums' buffering: the reviewer's case quotes nothing.
#[test]
fn a_string_valued_words_is_refused_in_fixed_text_by_the_type() {
    let frame = format!(
        r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":"{{}}","words":"{WORDS}","replace_unused":false}}}}"#
    );
    let error = serde_json::from_str::<isonomic_proto::ClientMessage>(&frame)
        .unwrap_err()
        .to_string();
    assert!(
        !error.contains("abandon") && !error.contains("art"),
        "{error}"
    );
    assert!(error.contains("recovery words must be a list"), "{error}");
}

#[test]
fn the_words_type_alone_refuses_in_fixed_text() {
    for json in [
        format!("\"{WORDS}\""),
        "[\"abandon\",{\"art\":1}]".to_string(),
        "{\"abandon\":\"art\"}".into(),
    ] {
        let error = serde_json::from_str::<RecoveryWords>(&json)
            .unwrap_err()
            .to_string();
        assert!(
            !error.contains("abandon") && !error.contains("art"),
            "{error}"
        );
    }
    let words: RecoveryWords = serde_json::from_str(&quoted()).unwrap();
    assert_eq!(words.len(), 24);
}

/// The second review's cases: no frame, however it is broken, has its
/// content quoted back, kit method or not.
#[test]
fn no_undecodable_frame_echoes_what_it_holds() {
    let list = quoted();
    let frames = [
        // Two `method`s: the typed decoder reads the first, `Value` the last.
        format!(
            r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":"{{}}","words":{list},"replace_unused":"{WORDS}"}},"method":"identity.get"}}"#
        ),
        format!(
            r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":"{{}}","words":{list},"replace_unused":"{WORDS}"}},"method":5}}"#
        ),
        // The whole frame as one string of words, and as the words array.
        format!(r#""{WORDS}""#),
        list.clone(),
        // The words as the method name, with and without an id.
        format!(r#"{{"type":"request","id":7,"method":"{WORDS}","params":{{}}}}"#),
        format!(r#"{{"type":"request","method":"{WORDS}","params":{{}}}}"#),
        // The words in requests that are not the kit's.
        format!(r#"{{"type":"request","id":7,"method":"identity.get","params":"{WORDS}"}}"#),
        format!(
            r#"{{"type":"request","id":7,"method":"system.unsubscribe","params":{{"subscription":"{WORDS}"}}}}"#
        ),
        // The words in `hello`, and as a message type.
        format!(r#"{{"type":"hello","versions":"{WORDS}","client":"x"}}"#),
        format!(r#"{{"type":"hello","versions":[0],"client":{list}}}"#),
        format!(r#"{{"type":"{WORDS}"}}"#),
        // Not JSON at all, with the words in it.
        format!("{WORDS} {{"),
    ];
    for frame in &frames {
        let message = message(frame);
        for word in ["abandon", "art"] {
            assert!(!message.contains(word), "{frame}\n=> {message}");
        }
    }
}

/// An unknown method is never named back, not even in a method's form:
/// words joined by dots have that form.
#[test]
fn an_unknown_method_is_never_named() {
    let dotted = "abandon.abandon.abandon.abandon.abandon.abandon.art";
    for method in [
        "system.reboot",
        dotted,
        "System.Reboot",
        "a b",
        "x\"y",
        &"a".repeat(65),
    ] {
        let frame = format!(
            r#"{{"type":"request","id":7,"method":{},"params":{{}}}}"#,
            serde_json::to_string(method).unwrap()
        );
        assert_eq!(
            message(&frame),
            "this engine has no such method",
            "{method}"
        );
    }
}

/// The strict decoder says category and place only, in Display and Debug.
#[test]
fn the_strict_decoder_says_only_where() {
    let frame =
        format!(r#"{{"type":"request","id":7,"method":"identity.get","params":"{WORDS}"}}"#);
    let error = isonomic_proto::frame::decode::<isonomic_proto::ClientMessage>(frame.as_bytes())
        .unwrap_err();
    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("abandon") && !text.contains("art"), "{text}");
        assert!(text.contains("data error"), "{text}");
    }
}

/// A restore that decodes prints neither its words nor its kit in `Debug`:
/// a person can pick the wrong file, and the kit is then the words.
#[test]
fn a_restore_with_the_words_as_its_kit_debugs_without_them() {
    let frame = format!(
        r#"{{"type":"request","id":7,"method":"identity.kit_restore","params":{{"kit":"{WORDS}","words":{},"replace_unused":false}}}}"#,
        quoted()
    );
    let FromClient::Message(decoded) = decode_client(frame.as_bytes()) else {
        panic!("a well-formed restore should decode");
    };
    let text = format!("{decoded:?}");
    assert!(!text.contains("abandon") && !text.contains("art"), "{text}");
    assert!(text.contains("bytes redacted"), "{text}");
}

/// A tool server's pasted secret never comes back either, however the
/// `connections.add` frame around it is broken.
#[test]
fn no_malformed_add_echoes_the_secret() {
    const SECRET: &str = "test-secret-9c41";
    let frames = [
        // The secret in a list, where a string belongs.
        format!(
            r#"{{"type":"request","id":7,"method":"connections.add","params":{{"name":"x","server":{{"kind":"remote","url":"u"}},"secret":["{SECRET}"]}}}}"#
        ),
        // The secret where the server belongs.
        format!(
            r#"{{"type":"request","id":7,"method":"connections.add","params":{{"name":"x","server":"{SECRET}","secret":"{SECRET}"}}}}"#
        ),
        // The secret as the name, beside a server that is not one.
        format!(
            r#"{{"type":"request","id":7,"method":"connections.add","params":{{"name":"{SECRET}","server":5}}}}"#
        ),
        // `params` as the secret.
        format!(r#"{{"type":"request","id":7,"method":"connections.add","params":"{SECRET}"}}"#),
    ];
    for frame in &frames {
        let message = message(frame);
        assert!(!message.contains("9c41"), "{message}");
    }
}
