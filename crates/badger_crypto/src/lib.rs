//! `badger_crypto`: encodings (base64, hex, URL), hashes (SHA-2, HMAC),
//! constant-time comparison, secure random bytes, and UUIDs. Hashes and
//! encodings are pure; random bytes and new UUIDs are recorded by
//! reproduction bundles (a replay answers them from the recording).

use badger_native::{Args, Error, Handler, ModuleDef, Outcome, Value};
use base64::Engine;
use hmac::Mac;
use sha2::Digest;

fn engine(url: bool) -> base64::engine::GeneralPurpose {
    if url {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
    } else {
        base64::engine::general_purpose::STANDARD
    }
}

fn base64_encode(a: &Args) -> Outcome {
    Ok(Value::Str(engine(a.bool(1)?).encode(a.bytes(0)?)))
}

fn base64_decode(a: &Args) -> Outcome {
    let text = a.str(0)?.trim();
    let url = a.bool(1)?;
    // padding is optional either way
    let r = if url {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(text.trim_end_matches('='))
    } else {
        base64::engine::GeneralPurpose::new(
            &base64::alphabet::STANDARD,
            base64::engine::GeneralPurposeConfig::new()
                .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
        )
        .decode(text)
    };
    r.map(Value::Bytes).map_err(|e| Error::new("encoding/base64", e.to_string()))
}

fn hex_decode(a: &Args) -> Outcome {
    let t = a.str(0)?.trim();
    if t.len() % 2 != 0 {
        return Err(Error::new("encoding/hex", "an odd number of hex digits"));
    }
    (0..t.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&t[i..i + 2], 16)
                .map_err(|_| Error::new("encoding/hex", format!("`{}` is not hex", &t[i..i + 2])))
        })
        .collect::<Result<Vec<u8>, _>>()
        .map(Value::Bytes)
}

/// Unreserved characters stay; everything else is `%XX`.
const COMPONENT: &percent_encoding::AsciiSet =
    &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

fn url_encode(a: &Args) -> Outcome {
    Ok(Value::Str(percent_encoding::utf8_percent_encode(a.str(0)?, COMPONENT).to_string()))
}

fn url_decode(a: &Args) -> Outcome {
    let t = a.str(0)?.replace('+', " ");
    percent_encoding::percent_decode_str(&t)
        .decode_utf8()
        .map(|s| Value::Str(s.into_owned()))
        .map_err(|e| Error::new("encoding/url", e.to_string()))
}

fn sha256(a: &Args) -> Outcome {
    Ok(Value::Bytes(sha2::Sha256::digest(a.bytes(0)?).to_vec()))
}

fn sha512(a: &Args) -> Outcome {
    Ok(Value::Bytes(sha2::Sha512::digest(a.bytes(0)?).to_vec()))
}

fn hmac_sha256(a: &Args) -> Outcome {
    // HMAC takes keys of any length
    let mut m = hmac::Hmac::<sha2::Sha256>::new_from_slice(a.bytes(0)?).expect("any key length");
    m.update(a.bytes(1)?);
    Ok(Value::Bytes(m.finalize().into_bytes().to_vec()))
}

/// Equal bytes, in time that depends only on the lengths.
fn equal(a: &Args) -> Outcome {
    let (x, y) = (a.bytes(0)?, a.bytes(1)?);
    let diff = x.iter().zip(y).fold(0u8, |d, (p, q)| d | (p ^ q));
    Ok(Value::Bool(x.len() == y.len() && diff == 0))
}

fn random_bytes(a: &Args) -> Outcome {
    let n = a.int(0)?;
    if !(0..=1 << 20).contains(&n) {
        return Err(Error::new("crypto/argument", "from 0 to 1048576 random bytes"));
    }
    let mut b = vec![0u8; n as usize];
    getrandom::fill(&mut b).map_err(|e| Error::new("crypto/random", e.to_string()))?;
    Ok(Value::Bytes(b))
}

fn uuid_v4(_: &Args) -> Outcome {
    Ok(Value::Str(uuid::Uuid::new_v4().to_string()))
}

fn uuid_v7(_: &Args) -> Outcome {
    Ok(Value::Str(uuid::Uuid::now_v7().to_string()))
}

fn uuid_parse(a: &Args) -> Outcome {
    uuid::Uuid::parse_str(a.str(0)?.trim())
        .map(|u| Value::Str(u.hyphenated().to_string()))
        .map_err(|e| Error::new("uuid/invalid", e.to_string()))
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_crypto",
    functions: &[
        ("base64_encode", Handler::Inline(base64_encode)),
        ("base64_decode", Handler::Inline(base64_decode)),
        ("hex_decode", Handler::Inline(hex_decode)),
        ("url_encode", Handler::Inline(url_encode)),
        ("url_decode", Handler::Inline(url_decode)),
        ("sha256", Handler::Inline(sha256)),
        ("sha512", Handler::Inline(sha512)),
        ("hmac_sha256", Handler::Inline(hmac_sha256)),
        ("equal", Handler::Inline(equal)),
        ("random_bytes", Handler::Inline(random_bytes)),
        ("uuid_v4", Handler::Inline(uuid_v4)),
        ("uuid_v7", Handler::Inline(uuid_v7)),
        ("uuid_parse", Handler::Inline(uuid_parse)),
    ],
    drop_handle: None,
    cancel: None,
    recorded: &[("random_bytes", "crypto random", 1), ("uuid_v4", "uuid", 0), ("uuid_v7", "uuid", 0)],
    pure: &[
        "base64_encode",
        "base64_decode",
        "hex_decode",
        "url_encode",
        "url_decode",
        "sha256",
        "sha512",
        "hmac_sha256",
        "equal",
        "uuid_parse",
    ],
};

badger_native::export!("badger_crypto", DEF);
