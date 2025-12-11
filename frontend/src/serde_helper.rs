use js_sys::wasm_bindgen::JsValue;
use serde::{Serialize, de::DeserializeOwned};

/// Error type for serialization/deserialization operations
#[derive(Debug)]
pub enum Error {
    SerdeWasmBindgen(serde_wasm_bindgen::Error),
    JsSys(JsValue),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SerdeWasmBindgen(e) => write!(f, "Serde WASM Bindgen Error: {}", e),
            Error::JsSys(v) => write!(f, "JS Sys Error: {:?}", v),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_wasm_bindgen::Error> for Error {
    fn from(e: serde_wasm_bindgen::Error) -> Self {
        Error::SerdeWasmBindgen(e)
    }
}

/// Serialize a Rust data structure into a JsValue
pub fn to_value<T: Serialize>(value: &T) -> Result<JsValue, Error> {
    // Configure serializer to handle large numbers as JS numbers (fixes BigInt issues with JSON.stringify)
    let serializer =
        serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(false);
    value.serialize(&serializer).map_err(Error::from)
}

/// Deserialize a JsValue into a Rust data structure
pub fn from_value<T: DeserializeOwned>(value: JsValue) -> Result<T, Error> {
    serde_wasm_bindgen::from_value(value).map_err(Error::from)
}

/// Convert a Rust data structure to a JSON string (via JsValue and JSON.stringify)
/// This is useful when you specifically need a JSON string in JS, or for certain APIs.
pub fn to_json_string<T: Serialize>(value: &T) -> Result<String, Error> {
    let js_val = to_value(value)?;
    let json_str = js_sys::JSON::stringify(&js_val)
        .map_err(Error::JsSys)?
        .as_string()
        .ok_or_else(|| Error::JsSys(JsValue::from_str("JSON.stringify returned non-string")))?;
    Ok(json_str)
}

/// Parse a JSON string into a Rust data structure (via JSON.parse and JsValue)
pub fn from_json_string<T: DeserializeOwned>(s: &str) -> Result<T, Error> {
    let js_val = js_sys::JSON::parse(s).map_err(Error::JsSys)?;
    from_value(js_val)
}
