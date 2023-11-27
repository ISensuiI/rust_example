#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StateSignal {
    #[prost(int32, tag = "1")]
    pub id: i32,
    #[prost(double, tag = "2")]
    pub current_scale: f64,
}
impl ::prost::Name for StateSignal {
    const NAME: &'static str = "StateSignal";
    const PACKAGE: &'static str = "mandelbrot";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("mandelbrot.{}", Self::NAME)
    }
}
