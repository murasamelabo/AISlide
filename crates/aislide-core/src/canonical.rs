use crate::Result;
use serde::Serialize;
use serde_json::{Number, Value};

fn normalize(value: &mut Value) {
    match value {
        Value::Number(number) => {
            if let Some(parsed) = number.as_f64() {
                if parsed.fract() == 0.0 && parsed.abs() <= 9_007_199_254_740_991.0 { *number = Number::from(parsed as i64); }
            }
        }
        Value::Array(values) => { for entry in values { normalize(entry); } }
        Value::Object(object) => { for entry in object.values_mut() { normalize(entry); } }
        _ => {}
    }
}

pub(crate) fn value(input: &impl Serialize) -> Result<Value> { let mut result = serde_json::to_value(input)?; normalize(&mut result); Ok(result) }
pub(crate) fn bytes(input: &impl Serialize) -> Result<Vec<u8>> { Ok(serde_json::to_vec(&value(input)?)?) }
pub(crate) fn equal(left: &Value, right: &Value) -> bool {
    let mut left = left.clone(); let mut right = right.clone(); normalize(&mut left); normalize(&mut right); left == right
}