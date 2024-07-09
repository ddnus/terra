use std::fmt::Debug;

pub trait State: Debug + Clone + Send + Sync + 'static {
}