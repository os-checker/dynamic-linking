pub use tokio;

pub use dynify;
pub use dynify::Fn;

pub type DynFut<T = ()> = dyn Send + Future<Output = T>;

/// The return value of a function that takes no arguemnts.
pub type Fut<T = ()> = Fn!(=> DynFut<T>);
