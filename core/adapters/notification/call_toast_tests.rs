//! Specifies the call offer's Windows toast (#111): its XML and how its answers read.

use super::call_toast::{CallToastAnswer, call_toast_xml};

#[test]
fn the_toast_names_the_app_and_offers_record_and_not_now() {
    let xml = call_toast_xml("Teams");
    assert!(xml.contains("<text>Teams is using the microphone</text>"));
    assert!(xml.contains("<text>Record this call?</text>"));
    assert!(xml.contains(r#"<action content="Record" arguments="record""#));
    assert!(xml.contains(r#"<action content="Not now" arguments="dismiss""#));
    assert!(
        xml.contains(r#"scenario="reminder""#),
        "it stays until answered"
    );
    assert!(
        xml.contains(r#"<audio silent="true"/>"#),
        "no sound during a call"
    );
}

#[test]
fn an_app_name_cannot_change_the_xml() {
    let xml = call_toast_xml(r#"<b>"Edge" & 'co'</b>"#);
    assert!(xml.contains("&lt;b&gt;&quot;Edge&quot; &amp; &apos;co&apos;&lt;/b&gt;"));
    assert!(!xml.contains("<b>"));
}

#[test]
fn answers_read_from_the_toast() {
    assert_eq!(
        CallToastAnswer::from_arguments("record"),
        CallToastAnswer::Record
    );
    assert_eq!(
        CallToastAnswer::from_arguments("dismiss"),
        CallToastAnswer::Dismissed
    );
    // A click on the toast itself is no answer: the banner in the app still offers.
    assert_eq!(CallToastAnswer::from_arguments(""), CallToastAnswer::Open);
    assert_eq!(CallToastAnswer::Record.as_str(), "record");
    assert_eq!(CallToastAnswer::Dismissed.as_str(), "dismissed");
    assert_eq!(CallToastAnswer::Open.as_str(), "open");
    assert_eq!(CallToastAnswer::Unavailable.as_str(), "unavailable");
}
