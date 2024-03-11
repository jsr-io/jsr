// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;

use hyper::Body;
use hyper::Response;
use routerify::RequestInfo;
use serde::Deserialize;
use serde::Serialize;
use tracing::Span;

use crate::api::ApiError;

#[derive(Serialize, Deserialize)]
pub struct ApiErrorStruct {
  pub code: Cow<'static, str>,
  pub message: Cow<'static, str>,
  #[serde(flatten)]
  pub data: serde_json::Value,
}

/// This macro builds an error enum that can be rendered to a `Response`. Every
/// error variant has a "code" (e.g `publishPayloadTooLarge`), "message" (a
/// human readable description of the error), and a "status code" (the HTTP
/// status code associated with the error). All three of these fields are
/// publicly visible to users, and thus should not contain any sensitive
/// information.
///
/// The variant name (e.g `NotFound`) will be used as both the variant name, and
/// after being converted to `camelCase` will be used as the error code.
///
/// Error variants can contain fields, which can be used when formatting the
/// error message. The fields are declared in the macro using the
/// `fields: { ... }` syntax. The fields are then available in the error message
/// using the `({ <field>* }) => <format string>` syntax.
///
/// By default, fields are not visible to users. If a field should be serialized
/// into the JSON response, the identifier should be added to the `data_fields`
/// list. The field will then be serialized into the `data` field of the JSON
/// response.
///
/// ### Example
///
/// ```rs
/// errors!(
///   NotFound {
///     status: NOT_FOUND,
///     "The requested resource was not found.",
///   },
///   DeploymentFailed {
///     status: BAD_REQUEST,
///     fields: { msg: Cow<'static, str>, deployment: Deployment },
///     data_fields: { deployment },
///     ({ msg }) => "The deployment failed: {msg}.",
///   },
/// );
/// ```
#[macro_export]
macro_rules! errors {
  ($($name:ident { status: $status:ident $(, fields: $fields:tt $(, data_fields: { $($data_field:ident),*$(,)? })?)?  $(, headers: $({ $($headers_pattern:ident),*$(,)? } => )? [ $(($header_name:expr, $header_value:expr)),*$(,)? ])? $(, $msg_lit:literal)? $(, ($pattern:tt) => $msg_expr:tt)? $(,)? }),*$(,)?) => {
    #[derive(Debug, Clone)]
    pub enum ApiError {
      $($name $($fields)?),*
    }

    impl ApiError {
      pub fn status_code(&self) -> hyper::StatusCode {
        match self {
          $(Self::$name { .. } => hyper::StatusCode::$status),*
        }
      }

      pub fn code(&self) -> &'static str {
        match self {
          $(Self::$name { .. } => const_format::map_ascii_case!(const_format::Case::Camel, stringify!($name))),*
        }
      }

      pub fn message(&self) -> std::borrow::Cow<'static, str> {
        match self {
          $(Self::$name $({..} => std::borrow::Cow::Borrowed($msg_lit))? $($pattern => std::borrow::Cow::Owned(format!($msg_expr)))?),*
        }
      }


      fn data(&self) -> serde_json::Value {
        match self {
          $(Self::$name { $($($($data_field),*,)?)? .. } => {
            serde_json::json!({
              $($($(
                const_format::map_ascii_case!(const_format::Case::Camel, stringify!($data_field)): $data_field,
              )*)?)?
            })
          })*
        }
      }
    }


    impl ApiError {
      pub fn json(&self) -> String {
        let err = $crate::errors_internal::ApiErrorStruct {
          code: Cow::Borrowed(self.code()),
          message: self.message(),
          data: self.data()
        };
        serde_json::to_string_pretty(&err).unwrap()
      }


      pub fn json_response(&self) -> Response<Body> {
        Response::builder()
          .status(self.status_code())
          .header("Content-Type", "application/json")
          .body(Body::from(self.json()))
          .unwrap()
      }
    }

    impl std::fmt::Display for ApiError {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
      }
    }

    impl std::error::Error for ApiError {}
  };
}

pub async fn error_handler(
  err: routerify::RouteError,
  _: RequestInfo,
) -> Response<Body> {
  // Because `routerify::RouteError` is a boxed error, it must be downcast
  // first. Unwrap for simplicity.
  let api_err = err.downcast::<ApiError>().unwrap();
  let span = Span::current();
  span.record("otel.status_code", "error");
  api_err.json_response()
}
