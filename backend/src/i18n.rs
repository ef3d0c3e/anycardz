//! Language of card and collection texts: `lang` cookie, else Accept-Language.

use std::future::{Ready, ready};

use actix_web::{FromRequest, HttpRequest, dev::Payload};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Fr,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Lang> {
        match s.get(..2)?.to_ascii_lowercase().as_str() {
            "en" => Some(Lang::En),
            "fr" => Some(Lang::Fr),
            _ => None,
        }
    }

    pub fn from_request(req: &HttpRequest) -> Lang {
        if let Some(lang) = req.cookie("lang").and_then(|c| Lang::parse(c.value())) {
            return lang;
        }
        req.headers()
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').find_map(|part| Lang::parse(part.trim())))
            .unwrap_or(Lang::En)
    }
}

impl FromRequest for Lang {
    type Error = actix_web::Error;
    type Future = Ready<Result<Lang, actix_web::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(Ok(Lang::from_request(req)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn cookie_wins_over_header() {
        let req = TestRequest::default()
            .cookie(actix_web::cookie::Cookie::new("lang", "fr"))
            .insert_header(("accept-language", "en-US,en;q=0.9"))
            .to_http_request();
        assert_eq!(Lang::from_request(&req), Lang::Fr);
    }

    #[test]
    fn header_picks_first_supported() {
        let req =
            TestRequest::default().insert_header(("accept-language", "de-DE,fr;q=0.8,en;q=0.5")).to_http_request();
        assert_eq!(Lang::from_request(&req), Lang::Fr);
        assert_eq!(Lang::from_request(&TestRequest::default().to_http_request()), Lang::En);
    }
}
