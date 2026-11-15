//! [`Json`]: a JSON value this crate carries but does not own, such as an
//! app's own event data.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Any JSON value. Its meaning is carried exactly; the order of object keys
/// is not (they come out sorted unless serde_json's `preserve_order` is on),
/// so compare values, not bytes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Json(pub serde_json::Value);

impl Json {
    /// Encode any value.
    pub fn new<T: Serialize + ?Sized>(value: &T) -> Result<Self, serde_json::Error> {
        serde_json::to_value(value).map(Self)
    }

    /// Decode into a typed value.
    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        T::deserialize(&self.0)
    }
}

impl From<serde_json::Value> for Json {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trips_and_decodes() {
        let text = r#"{"a":[true,null],"z":1}"#;
        let json: Json = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_string(&json).unwrap(), text);
        let value: serde_json::Value = json.decode().unwrap();
        assert_eq!(value["z"], 1);
        assert_eq!(
            serde_json::to_string(&Json::new(&[1, 2]).unwrap()).unwrap(),
            "[1,2]"
        );
    }
}
