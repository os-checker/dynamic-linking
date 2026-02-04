pub use tokio;

pub use dynify;
pub type Fut<T = ()> = ::dynify::Fn!(=> dyn Send + Future<Output = T>);
