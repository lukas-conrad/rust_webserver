use crate::plugin::plugin_manager::PluginError;
use crate::plugin_communication::models::{HttpRequest, HttpResponse};
use futures::future::BoxFuture;
use hyper::service::Service;
use hyper::StatusCode;
use strum::Display;

#[derive(Display, Debug)]
pub enum ServerError {
    RequestProcessingError(StatusCode, String),
}

pub type CallbackFn = Box<
    dyn Fn(HttpRequest) -> BoxFuture<'static, Result<HttpResponse, PluginError>>
        + Send
        + Sync
        + Unpin,
>;

pub trait Webserver {
    fn set_listener(&self, listener: CallbackFn);
}

