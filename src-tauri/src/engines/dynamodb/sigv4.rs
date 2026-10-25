//! AWS Signature Version 4 for one request (HMAC-SHA256 chain, no SDK).

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// The access key pair. The secret only ever feeds the HMAC chain.
pub struct Credentials {
    pub access_key: String,
    pub secret_key: String,
}

/// What gets signed: lower-case header names; `query` already canonical (sorted, encoded).
pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    m.update(data.as_bytes());
    m.finalize().into_bytes().to_vec()
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Trims a header value and collapses runs of spaces, as SigV4 requires.
fn canonical_value(v: &str) -> String {
    v.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn canonical_request(r: &Request<'_>) -> (String, String) {
    let mut headers: Vec<(String, String)> = r
        .headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), canonical_value(v)))
        .collect();
    headers.sort();
    let lines: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed = headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical = format!(
        "{}\n{}\n{}\n{lines}\n{signed}\n{}",
        r.method,
        r.path,
        r.query,
        sha256_hex(r.body)
    );
    (canonical, signed)
}

/// The derived key for one day, region and service.
pub fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date);
    let k_region = hmac(&k_date, region);
    let k_service = hmac(&k_region, service);
    hmac(&k_service, "aws4_request")
}

/// The `Authorization` header value; `amz_date` is `YYYYMMDDTHHMMSSZ` and must also be
/// among the signed headers as `x-amz-date`.
pub fn authorization(
    c: &Credentials,
    region: &str,
    service: &str,
    amz_date: &str,
    r: &Request<'_>,
) -> String {
    let date = &amz_date[..8.min(amz_date.len())];
    let scope = format!("{date}/{region}/{service}/aws4_request");
    let (canonical, signed) = canonical_request(r);
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    let key = signing_key(&c.secret_key, date, region, service);
    let signature = hex::encode(hmac(&key, &to_sign));
    format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={signature}",
        c.access_key
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The AWS SigV4 test suite credentials and date.
    const DATE: &str = "20150830T123600Z";

    fn creds() -> Credentials {
        Credentials {
            access_key: "AKIDEXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
        }
    }

    fn vanilla(method: &str) -> String {
        let headers = [("Host", "example.amazonaws.com"), ("X-Amz-Date", DATE)];
        let r = Request {
            method,
            path: "/",
            query: "",
            headers: &headers,
            body: b"",
        };
        authorization(&creds(), "us-east-1", "service", DATE, &r)
    }

    #[test]
    fn aws_suite_get_and_post_vanilla() {
        let prefix = "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/\
                      aws4_request, SignedHeaders=host;x-amz-date, Signature=";
        assert_eq!(
            vanilla("GET"),
            format!("{prefix}5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31")
        );
        assert_eq!(
            vanilla("POST"),
            format!("{prefix}5da7c1a2acd57cee7505fc6676e4e544621c30862966e37dddb68e92efbe5d6b")
        );
    }

    #[test]
    fn aws_docs_iam_example() {
        let key = signing_key(
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "20150830",
            "us-east-1",
            "iam",
        );
        assert_eq!(
            hex::encode(&key),
            "c4afb1cc5771d871763a393e44b703571b55cc28424d1a5e86da6ed3c154a4b9"
        );
        let headers = [
            (
                "Content-Type",
                "application/x-www-form-urlencoded; charset=utf-8",
            ),
            ("Host", "iam.amazonaws.com"),
            ("X-Amz-Date", DATE),
        ];
        let r = Request {
            method: "GET",
            path: "/",
            query: "Action=ListUsers&Version=2010-05-08",
            headers: &headers,
            body: b"",
        };
        let (canonical, _) = canonical_request(&r);
        assert_eq!(
            sha256_hex(canonical.as_bytes()),
            "f536975d06c0309214f805bb90ccff089219ecd68b2577efef23edd43b7e1a59"
        );
        assert!(
            authorization(&creds(), "us-east-1", "iam", DATE, &r).ends_with(
                "SignedHeaders=content-type;host;x-amz-date, \
             Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7"
            )
        );
    }

    #[test]
    fn header_values_are_trimmed() {
        assert_eq!(canonical_value("  a   b  c "), "a b c");
    }
}
