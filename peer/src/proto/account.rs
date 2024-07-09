#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountRegisterReq {
    #[prost(string, tag = "1")]
    pub account: ::prost::alloc::string::String,
}
