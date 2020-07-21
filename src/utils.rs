use std::convert::Infallible;
use warp::Filter;

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
