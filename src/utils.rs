use std::{convert::Infallible, fmt::Display, sync::Arc};
use warp::{reject::Reject, Filter, Rejection};

pub fn inject<'a, T: Send + Sync>(
    val: &'a Arc<T>,
) -> impl Filter<Extract = (Arc<T>,), Error = Infallible> + Clone + 'a {
    warp::any().map(move || val.clone())
}

#[derive(Debug)]
struct InternalServerError;
impl Reject for InternalServerError {}

pub trait TryExt<T> {
    fn or_internal_server_error(self) -> Result<T, Rejection>;
    fn or_not_found(self) -> Result<T, Rejection>;
}

impl<T, E: Display> TryExt<T> for Result<T, E> {
    fn or_internal_server_error(self) -> Result<T, Rejection> {
        self.map_err(|e| {
            log::error!("{}", e);
            warp::reject::custom(InternalServerError)
        })
    }
    fn or_not_found(self) -> Result<T, Rejection> {
        self.map_err(|e| {
            log::error!("{}", e);
            warp::reject::not_found()
        })
    }
}

impl<T> TryExt<T> for Option<T> {
    fn or_internal_server_error(self) -> Result<T, Rejection> {
        self.ok_or_else(|| warp::reject::custom(InternalServerError))
    }
    fn or_not_found(self) -> Result<T, Rejection> {
        self.ok_or_else(warp::reject::not_found)
    }
}
