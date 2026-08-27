use quick_xml::de::from_str;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TwimlError {
    #[error("XML parse error: {0}")]
    ParseError(#[from] quick_xml::DeError),
    #[error("Invalid TwiML structure")]
    InvalidStructure,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Response {
    #[serde(rename = "Say", default)]
    pub say: Option<Say>,
    #[serde(rename = "Hangup", default)]
    pub hangup: Option<Hangup>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Say {
    #[serde(rename = "$value")]
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Hangup;

#[derive(Debug)]
pub struct TwimlAction {
    pub say_text: Option<String>,
    pub should_hangup: bool,
}

impl TwimlAction {
    pub fn should_say(&self) -> bool {
        self.say_text.is_some()
    }
}

pub fn parse_twiml(xml: &str) -> Result<TwimlAction, Box<dyn std::error::Error>> {
    let response: Response = from_str(xml)?;

    Ok(TwimlAction {
        say_text: response.say.and_then(|s| s.text),
        should_hangup: response.hangup.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_twiml_with_say_and_hangup() {
        let xml = r#"<Response><Say>Hello world</Say><Hangup/></Response>"#;
        let action = parse_twiml(xml).unwrap();
        assert_eq!(action.say_text, Some("Hello world".to_string()));
        assert!(action.should_hangup);
    }

    #[test]
    fn test_parse_twiml_with_only_say() {
        let xml = r#"<Response><Say>Test message</Say></Response>"#;
        let action = parse_twiml(xml).unwrap();
        assert_eq!(action.say_text, Some("Test message".to_string()));
        assert!(!action.should_hangup);
    }

    #[test]
    fn test_parse_twiml_empty_response() {
        let xml = r#"<Response></Response>"#;
        let action = parse_twiml(xml).unwrap();
        assert_eq!(action.say_text, None);
        assert!(!action.should_hangup);
    }

    #[test]
    fn test_parse_invalid_xml() {
        let xml = r#"<Invalid>not twiml"#;
        let result = parse_twiml(xml);
        assert!(result.is_err());
    }
}
