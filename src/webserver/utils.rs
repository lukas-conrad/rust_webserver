use crate::plugin_communication::models::{HttpHeader, HttpRequest, HttpResponse};
use crate::webserver::webserver::ServerError;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};

pub async fn build_http_request(req: Request<Incoming>) -> HttpRequest {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    let host = req
        .headers()
        .get("host")
        .map(|h| h.to_str().unwrap_or(""))
        .unwrap_or("")
        .to_string();
    let headers: Vec<HttpHeader> = req
        .headers()
        .iter()
        .map(|(name, value)| HttpHeader {
            key: name.to_string(),
            value: value.to_str().unwrap_or("").to_string(),
        })
        .collect();
    let (_, body) = req.into_parts();
    let body_bytes = body.collect().await.unwrap_or_default().to_bytes();

    HttpRequest {
        request_method: method,
        path,
        host,
        headers,
        body: BASE64_STANDARD.encode(body_bytes),
    }
}

pub fn build_http_response(
    response: HttpResponse,
) -> Result<Response<Full<Bytes>>, ServerError> {
    let body_bytes = BASE64_STANDARD.decode(&response.body).map_err(|err| {
        log::error!("Failed to decode base64 response body: {}", err);
        ServerError::RequestProcessingError(
            StatusCode::BAD_GATEWAY,
            "Invalid response from plugin: failed to decode body".to_string(),
        )
    })?;

    let status_code = StatusCode::from_u16(response.status_code).map_err(|err| {
        log::error!("Invalid status code {} from plugin: {}", response.status_code, err);
        ServerError::RequestProcessingError(
            StatusCode::BAD_GATEWAY,
            format!("Invalid status code from plugin: {}", response.status_code),
        )
    })?;

    let mut response_builder = Response::builder().status(status_code);
    for header in response.headers {
        response_builder = response_builder.header(header.key, header.value);
    }

    let client_response = response_builder
        .body(Full::new(Bytes::from(body_bytes)))
        .map_err(|err| {
            log::error!("Failed to build response: {}", err);
            ServerError::RequestProcessingError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build response".to_string(),
            )
        })?;

    Ok(client_response)
}

