use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::collections::BTreeMap;

pub fn signature(path: &str, parameters: &BTreeMap<String, String>, secret: &str) -> String {
    let mut values: Vec<String> = parameters
        .iter()
        .filter(|(k, _)| *k != "_aop_signature")
        .map(|(k, v)| format!("{k}{v}"))
        .collect();
    values.sort();
    let mut mac =
        Hmac::<Sha1>::new_from_slice(secret.as_bytes()).expect("HMAC accepts arbitrary key length");
    mac.update(path.as_bytes());
    for value in values {
        mac.update(value.as_bytes());
    }
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}
pub fn valid_signature(path: &str, parameters: &BTreeMap<String, String>, secret: &str) -> bool {
    let Some(supplied) = parameters.get("_aop_signature") else {
        return false;
    };
    let expected = signature(path, parameters, secret);
    if supplied.len() != expected.len() {
        return false;
    }
    supplied
        .bytes()
        .zip(expected.bytes())
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_signature_vector() {
        let parameters = BTreeMap::from([("b".into(), "2".into()), ("a".into(), "1".into())]);
        assert_eq!(
            signature(
                "param2/1/system/currentTime/1000000",
                &parameters,
                "test123"
            ),
            "33E54F4F7B989E3E0E912D3FBD2F1A03CA7CCE88"
        );
    }
    #[test]
    fn signature_covers_path_and_raw_values() {
        let mut input = BTreeMap::from([
            ("name".into(), "中文 + &".into()),
            ("payload".into(), "{\"a\":1}".into()),
        ]);
        let first = signature("param2/1/test/api/key", &input, "secret");
        input.insert("_aop_signature".into(), first);
        assert!(valid_signature("param2/1/test/api/key", &input, "secret"));
        assert!(!valid_signature(
            "param2/1/test/other/key",
            &input,
            "secret"
        ));
        input.insert("name".into(), "中文 +".into());
        assert!(!valid_signature("param2/1/test/api/key", &input, "secret"));
    }
}
