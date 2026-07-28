use std::error::Error;

use a3s_boot::{BootApplication, BootError, BootRequest, BootResponse, HttpMethod};
use axum::body::{to_bytes, Body, HttpBody};
use axum::extract::{Request, State};
use axum::http::{
    header::{ALLOW, CONTENT_TYPE},
    response::Builder as ResponseBuilder,
    HeaderName, HeaderValue, Method, StatusCode,
};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use futures::StreamExt;

use super::API_PREFIX;

#[derive(Clone)]
pub(super) struct ApiGateway {
    app: BootApplication,
    body_limit: usize,
}

impl ApiGateway {
    pub(super) fn new(app: BootApplication, body_limit: usize) -> Self {
        Self { app, body_limit }
    }

    pub(super) fn router(self) -> Router {
        let api_catch_all = format!("{API_PREFIX}/{{*path}}");

        Router::new()
            .route(API_PREFIX, any(dispatch_api))
            .route(&api_catch_all, any(dispatch_api))
            .with_state(self)
    }

    async fn dispatch(&self, request: Request) -> Response {
        let path = request.uri().path().to_string();
        let is_head = request.method() == Method::HEAD;
        let boot_request = match to_boot_request(request, self.body_limit).await {
            Ok(request) => request,
            Err(error) => {
                return finalize_response(&self.app, &path, boot_error_response(error), is_head)
            }
        };

        let response = to_axum_response(self.app.handle(boot_request).await);
        finalize_response(&self.app, &path, response, is_head)
    }
}

async fn dispatch_api(State(gateway): State<ApiGateway>, request: Request) -> Response {
    gateway.dispatch(request).await
}

async fn to_boot_request(
    axum_request: Request,
    body_limit: usize,
) -> a3s_boot::Result<BootRequest> {
    let path = axum_request.uri().path().to_string();
    let method = axum_request
        .method()
        .as_str()
        .parse::<HttpMethod>()
        .map_err(|error| match error {
            BootError::MethodNotAllowed(method) => {
                BootError::MethodNotAllowed(format!("{method} {path}"))
            }
            error => error,
        })?;
    let query_string = axum_request.uri().query().map(str::to_string);
    let mut headers = Vec::new();
    for (name, value) in axum_request.headers() {
        let value = value.to_str().map_err(|error| {
            BootError::BadRequest(format!("invalid request header value for {name}: {error}"))
        })?;
        headers.push((name.as_str().to_string(), value.to_string()));
    }

    let mut boot_request = BootRequest::new(method, path);
    if let Some(query_string) = query_string {
        boot_request = boot_request.with_query_string(query_string);
    }
    for (name, value) in headers {
        boot_request = if boot_request.header(&name).is_some() {
            boot_request.append_header(name, value)
        } else {
            boot_request.with_header(name, value)
        };
    }

    boot_request.validate_headers()?;
    boot_request.validate_body_limit(body_limit)?;

    let body = axum_request.into_body();
    if body.size_hint().lower() > body_limit as u64 {
        return Err(BootError::PayloadTooLarge(format!(
            "request body exceeds {body_limit} bytes"
        )));
    }
    let body = to_bytes(body, body_limit)
        .await
        .map_err(|error| map_body_error(error, body_limit))?
        .to_vec();

    let boot_request = boot_request.with_body(body);
    boot_request.validate_with_body_limit(body_limit)?;
    Ok(boot_request)
}

fn map_body_error(error: axum::Error, body_limit: usize) -> BootError {
    if error
        .source()
        .is_some_and(|source| source.is::<http_body_util::LengthLimitError>())
    {
        BootError::PayloadTooLarge(format!("request body exceeds {body_limit} bytes"))
    } else {
        BootError::Adapter(error.to_string())
    }
}

fn to_axum_response(response: BootResponse) -> Response {
    if let Err(error) = response.validate() {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, internal_message(error));
    }

    let is_streaming = response.is_streaming();
    let status = match StatusCode::from_u16(response.status()) {
        Ok(status) => status,
        Err(error) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("invalid response status {}: {error}", response.status()),
            )
        }
    };

    let mut builder = Response::builder().status(status);
    builder = match with_response_headers(builder, response.header_entries()) {
        Ok(builder) => builder,
        Err(message) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, message),
    };

    let body = if is_streaming && response.is_file_stream() {
        let Some(stream) = response.into_body_stream() else {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "streaming response body has already been consumed".to_string(),
            );
        };
        Body::from_stream(stream)
    } else if is_streaming {
        let Some(stream) = response.into_sse_stream() else {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "streaming response body has already been consumed".to_string(),
            );
        };
        Body::from_stream(stream.map(|event| event.map(|event| event.encode())))
    } else {
        Body::from(response.into_body())
    };

    builder.body(body).unwrap_or_else(|error| {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    })
}

fn with_response_headers<I, N, V>(
    mut builder: ResponseBuilder,
    headers: I,
) -> std::result::Result<ResponseBuilder, String>
where
    I: IntoIterator<Item = (N, V)>,
    N: AsRef<str>,
    V: AsRef<str>,
{
    for (name, value) in headers {
        let name = name.as_ref();
        let value = value.as_ref();
        let header_name = HeaderName::try_from(name)
            .map_err(|error| format!("invalid response header name {name:?}: {error}"))?;
        let header_value = HeaderValue::try_from(value)
            .map_err(|error| format!("invalid response header value for {name:?}: {error}"))?;
        builder = builder.header(header_name, header_value);
    }

    Ok(builder)
}

fn finalize_response(
    app: &BootApplication,
    path: &str,
    response: Response,
    is_head: bool,
) -> Response {
    strip_head_body(is_head, with_allow_header(app, path, response))
}

fn with_allow_header(app: &BootApplication, path: &str, mut response: Response) -> Response {
    if response.status() != StatusCode::METHOD_NOT_ALLOWED {
        return response;
    }

    let Some(allow) = app.allowed_methods_header(path) else {
        return response;
    };

    match HeaderValue::try_from(allow) {
        Ok(value) => {
            response.headers_mut().insert(ALLOW, value);
            response
        }
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("invalid allow header value: {error}"),
        ),
    }
}

fn strip_head_body(is_head: bool, response: Response) -> Response {
    if !is_head {
        return response;
    }

    let (parts, _) = response.into_parts();
    Response::from_parts(parts, Body::empty())
}

fn boot_error_response(error: BootError) -> Response {
    to_axum_response(BootResponse::from_error(&error))
}

fn internal_message(error: BootError) -> String {
    match error {
        BootError::Internal(message) => message,
        error => error.to_string(),
    }
}

fn error_response(status: StatusCode, message: String) -> Response {
    let mut response = Response::new(Body::from(message));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use a3s_boot::StreamableFile;

    use super::*;

    #[tokio::test]
    async fn streams_file_responses_without_treating_them_as_sse() {
        let response = to_axum_response(BootResponse::streamable_file(
            StreamableFile::stream(futures::stream::iter([
                Ok(Vec::from("hello ")),
                Ok(Vec::from("pdf")),
            ]))
            .with_content_type("application/pdf")
            .with_content_length(9),
        ));

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/pdf"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body.as_ref(), b"hello pdf");
    }
}
