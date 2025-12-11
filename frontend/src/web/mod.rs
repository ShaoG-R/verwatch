//! 原生 Web API 封装模块
//!
//! 此模块提供对浏览器原生 API 的轻量级封装，替代 gloo-* 系列 crate，
//! 以减小 WASM 二进制体积。

mod http;
mod storage;
mod timer;

pub use http::HttpClient;
pub use storage::LocalStorage;
pub use timer::Interval;
