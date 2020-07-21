use crate::providers::Params;
use std::fmt::Display;
use warp::{
    http::Uri,
    reject::{Reject, Rejection},
    Reply,
};

#[derive(Debug)]
struct InternalServerError;
impl Reject for InternalServerError {}

#[derive(Debug)]
struct Redirect(Uri);
impl Reject for Redirect {}

pub trait TryExt<T> {
    fn or_ise(self) -> Result<T, Rejection>;
    fn or_nf(self) -> Result<T, Rejection>;
    fn or_redirect<M: Display>(self, msg: M, params: &Params) -> Result<T, Rejection>;
}

impl<T, E: Display> TryExt<T> for Result<T, E> {
    fn or_ise(self) -> Result<T, Rejection> {
        self.map_err(|e| {
            tracing::error!("{}", e);
            warp::reject::custom(InternalServerError)
        })
    }

    fn or_nf(self) -> Result<T, Rejection> {
        self.map_err(|e| {
            tracing::error!("{}", e);
            warp::reject::not_found()
        })
    }

    fn or_redirect<M: Display>(self, msg: M, params: &Params) -> Result<T, Rejection> {
        self.map_err(|e| {
            tracing::error!("{}", e);

            let uri = match &params.state {
                Some(s) => format!("{}?error={}&state={}", params.redirect_uri, msg, s),
                None => format!("{}?error={}", params.redirect_uri, msg),
            };
            match Uri::from_maybe_shared(uri).or_ise() {
                Ok(uri) => warp::reject::custom(Redirect(uri)),
                Err(e) => e,
            }
        })
    }
}

impl<T> TryExt<T> for Option<T> {
    fn or_ise(self) -> Result<T, Rejection> {
        self.ok_or_else(|| warp::reject::custom(InternalServerError))
    }

    fn or_nf(self) -> Result<T, Rejection> {
        self.ok_or_else(warp::reject::not_found)
    }

    fn or_redirect<M: Display>(self, msg: M, params: &Params) -> Result<T, Rejection> {
        self.ok_or_else(|| {
            let uri = match &params.state {
                Some(s) => format!("{}?error={}&state={}", params.redirect_uri, msg, s),
                None => format!("{}?error={}", params.redirect_uri, msg),
            };
            match Uri::from_maybe_shared(uri).or_ise() {
                Ok(uri) => warp::reject::custom(Redirect(uri)),
                Err(e) => e,
            }
        })
    }
}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(Redirect(uri)) = err.find() {
        Ok(warp::redirect::temporary(uri.clone()))
    } else {
        Err(err)
    }
}
