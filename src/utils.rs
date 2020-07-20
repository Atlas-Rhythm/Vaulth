use std::{convert::Infallible, fmt::Display};
use warp::{reject::Reject, Filter, Rejection};

/// Injects a Copy type into a filter
pub fn with_copied<T: Copy + Send + 'static>(
    val: T,
) -> impl Filter<Extract = (T,), Error = Infallible> + Copy + 'static {
    warp::any().map(move || val)
}

/// Injects a Clone type into a filter
pub fn with_cloned<T: Clone + Send + Sync + 'static>(
    val: T,
) -> impl Filter<Extract = (T,), Error = Infallible> + Clone + 'static {
    warp::any().map(move || val.clone())
}

#[derive(Debug)]
struct InternalServerError;
impl Reject for InternalServerError {}

pub trait TryExt<T> {
    fn or_ise(self) -> Result<T, Rejection>;
    fn or_nf(self) -> Result<T, Rejection>;
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
}

impl<T> TryExt<T> for Option<T> {
    fn or_ise(self) -> Result<T, Rejection> {
        self.ok_or_else(|| warp::reject::custom(InternalServerError))
    }
    fn or_nf(self) -> Result<T, Rejection> {
        self.ok_or_else(warp::reject::not_found)
    }
}
