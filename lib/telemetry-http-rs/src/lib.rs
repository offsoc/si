//! Telemetry support for running HTTP-related services.

#![warn(
    clippy::unwrap_in_result,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::panic_in_result_fn,
    missing_docs
)]

use hyper::header::USER_AGENT;
use telemetry::prelude::*;
use tower_http::trace::MakeSpan;

/// An implementation of [`MakeSpan`] to generate [`Span`]s from incoming HTTP requests.
#[derive(Clone, Debug)]
pub struct HttpMakeSpan {
    level: Level,

    // See: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#common-attributes
    network_protocol_name: &'static str,
    network_transport: NetworkTransport,
}

impl HttpMakeSpan {
    /// Creates a new `HttpMakeSpan`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the [`Level`] used for the tracing [`Span`].
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Sets the network [protocol name] to be used in span metadata.
    ///
    /// Defaults to `"http"`.
    ///
    /// [protocol name]: https://opentelemetry.io/docs/specs/semconv/attributes-registry/network/
    pub fn network_protocol_name(mut self, s: &'static str) -> Self {
        self.network_protocol_name = s;
        self
    }

    /// Sets the network [protocol version] to be used in span metadata.
    ///
    /// Defaults to `tcp`.
    ///
    /// [protocol version]: https://opentelemetry.io/docs/specs/semconv/attributes-registry/network/
    pub fn network_transport(mut self, nt: NetworkTransport) -> Self {
        self.network_transport = nt;
        self
    }

    fn span_from_request<B>(&mut self, request: &hyper::Request<B>) -> Span {
        #[derive(Clone, Copy, Debug)]
        enum InnerMethod {
            Options,
            Get,
            Post,
            Put,
            Delete,
            Head,
            Trace,
            Connect,
            Patch,
            Other,
        }
        impl From<&str> for InnerMethod {
            fn from(value: &str) -> Self {
                match value {
                    "OPTIONS" => Self::Options,
                    "GET" => Self::Get,
                    "POST" => Self::Post,
                    "PUT" => Self::Put,
                    "DELETE" => Self::Delete,
                    "HEAD" => Self::Head,
                    "TRACE" => Self::Trace,
                    "CONNECT" => Self::Connect,
                    "PATCH" => Self::Patch,
                    _ => Self::Other,
                }
            }
        }
        impl InnerMethod {
            fn as_str(&self) -> &'static str {
                match self {
                    Self::Options => "OPTIONS",
                    Self::Get => "GET",
                    Self::Post => "POST",
                    Self::Put => "PUT",
                    Self::Delete => "DELETE",
                    Self::Head => "HEAD",
                    Self::Trace => "TRACE",
                    Self::Connect => "CONNECT",
                    Self::Patch => "PATCH",
                    // > If the HTTP request method is not known to instrumentation, it MUST set
                    // > the http.request.method attribute to _OTHER.
                    //
                    // See: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#common-attributes
                    Self::Other => "_OTHER",
                }
            }
        }

        enum InnerLevel {
            Error,
            Warn,
            Info,
            Debug,
            Trace,
        }
        impl From<Level> for InnerLevel {
            fn from(value: Level) -> Self {
                match value {
                    Level::ERROR => InnerLevel::Error,
                    Level::WARN => InnerLevel::Warn,
                    Level::INFO => InnerLevel::Info,
                    Level::DEBUG => InnerLevel::Debug,
                    _ => InnerLevel::Trace,
                }
            }
        }

        let uri = request.uri();

        let http_request_method = InnerMethod::from(request.method().as_str());
        let network_protocol_version = HttpVersion::from(request.version());

        // This ugly macro is needed, unfortunately, because `tracing::span!` required the level
        // argument to be static. Meaning we can't just pass `self.level` and a dynamic name.
        macro_rules! inner {
            ($level:expr, $name:expr) => {
                ::telemetry::tracing::span!(
                    $level,
                    $name,

                    // Common HTTP attributes
                    //
                    // See: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#common-attributes

                    http.request.method = http_request_method.as_str(),
                    // http.response.header.<key>
                    http.response.status_code = Empty,
                    // network.peer.address = Empty,
                    // network.peer.port = Empty,
                    network.protocol.name = self.network_protocol_name,
                    network.protocol.version = network_protocol_version.as_str(),
                    network.transport = self.network_transport.as_str(),

                    // HTTP Server semantic conventions
                    //
                    // See: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#http-server

                    // client.address = Empty,
                    // client.port = Empty,
                    // http.request.header.<key>
                    // network.local.address = Empty,
                    // network.local.port = Empty,
                    // server.address = Empty,
                    // server.port = Empty,
                    url.path = uri.path(),
                    url.query = uri.query(),
                    url.scheme = Empty,
                    user_agent.original = Empty,

                    // Set special `otel.*` fields which tracing-opentelemetry will use when
                    // transmitting traces via OpenTelemetry protocol
                    //
                    // See:
                    // https://docs.rs/tracing-opentelemetry/0.22.0/tracing_opentelemetry/#special-fields
                    //

                    otel.kind = SpanKind::Server.as_str(),
                    // TODO(fnichol): would love this to be "{method} {route}" but should limit
                    // detail in route to preserve low cardinality.
                    //
                    // See: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#method-placeholder
                    // See: https://opentelemetry.io/docs/specs/semconv/http/http-spans/#name
                    otel.name = $name,
                    // Default for OpenTelemetry status is `Unset` which should map to an empty/unset
                    // tracing value.
                    //
                    // See: https://docs.rs/opentelemetry/0.21.0/opentelemetry/trace/enum.Status.html
                    otel.status_code = Empty,
                    // Only set if status_code == Error
                    otel.status_message = Empty,
                )
            };
        }

        let span = match (InnerLevel::from(self.level), http_request_method) {
            (InnerLevel::Error, InnerMethod::Options) => inner!(Level::ERROR, "OPTIONS"),
            (InnerLevel::Error, InnerMethod::Get) => inner!(Level::ERROR, "GET"),
            (InnerLevel::Error, InnerMethod::Post) => inner!(Level::ERROR, "POST"),
            (InnerLevel::Error, InnerMethod::Put) => inner!(Level::ERROR, "PUT"),
            (InnerLevel::Error, InnerMethod::Delete) => inner!(Level::ERROR, "DELETE"),
            (InnerLevel::Error, InnerMethod::Head) => inner!(Level::ERROR, "HEAD"),
            (InnerLevel::Error, InnerMethod::Trace) => inner!(Level::ERROR, "TRACE"),
            (InnerLevel::Error, InnerMethod::Connect) => inner!(Level::ERROR, "CONNECT"),
            (InnerLevel::Error, InnerMethod::Patch) => inner!(Level::ERROR, "PATCH"),
            (InnerLevel::Error, InnerMethod::Other) => inner!(Level::ERROR, "HTTP"),
            (InnerLevel::Warn, InnerMethod::Options) => inner!(Level::WARN, "OPTIONS"),
            (InnerLevel::Warn, InnerMethod::Get) => inner!(Level::WARN, "GET"),
            (InnerLevel::Warn, InnerMethod::Post) => inner!(Level::WARN, "POST"),
            (InnerLevel::Warn, InnerMethod::Put) => inner!(Level::WARN, "PUT"),
            (InnerLevel::Warn, InnerMethod::Delete) => inner!(Level::WARN, "DELETE"),
            (InnerLevel::Warn, InnerMethod::Head) => inner!(Level::WARN, "HEAD"),
            (InnerLevel::Warn, InnerMethod::Trace) => inner!(Level::WARN, "TRACE"),
            (InnerLevel::Warn, InnerMethod::Connect) => inner!(Level::WARN, "CONNECT"),
            (InnerLevel::Warn, InnerMethod::Patch) => inner!(Level::WARN, "PATCH"),
            (InnerLevel::Warn, InnerMethod::Other) => inner!(Level::WARN, "HTTP"),
            (InnerLevel::Info, InnerMethod::Options) => inner!(Level::INFO, "OPTIONS"),
            (InnerLevel::Info, InnerMethod::Get) => inner!(Level::INFO, "GET"),
            (InnerLevel::Info, InnerMethod::Post) => inner!(Level::INFO, "POST"),
            (InnerLevel::Info, InnerMethod::Put) => inner!(Level::INFO, "PUT"),
            (InnerLevel::Info, InnerMethod::Delete) => inner!(Level::INFO, "DELETE"),
            (InnerLevel::Info, InnerMethod::Head) => inner!(Level::INFO, "HEAD"),
            (InnerLevel::Info, InnerMethod::Trace) => inner!(Level::INFO, "TRACE"),
            (InnerLevel::Info, InnerMethod::Connect) => inner!(Level::INFO, "CONNECT"),
            (InnerLevel::Info, InnerMethod::Patch) => inner!(Level::INFO, "PATCH"),
            (InnerLevel::Info, InnerMethod::Other) => inner!(Level::INFO, "HTTP"),
            (InnerLevel::Debug, InnerMethod::Options) => inner!(Level::DEBUG, "OPTIONS"),
            (InnerLevel::Debug, InnerMethod::Get) => inner!(Level::DEBUG, "GET"),
            (InnerLevel::Debug, InnerMethod::Post) => inner!(Level::DEBUG, "POST"),
            (InnerLevel::Debug, InnerMethod::Put) => inner!(Level::DEBUG, "PUT"),
            (InnerLevel::Debug, InnerMethod::Delete) => inner!(Level::DEBUG, "DELETE"),
            (InnerLevel::Debug, InnerMethod::Head) => inner!(Level::DEBUG, "HEAD"),
            (InnerLevel::Debug, InnerMethod::Trace) => inner!(Level::DEBUG, "TRACE"),
            (InnerLevel::Debug, InnerMethod::Connect) => inner!(Level::DEBUG, "CONNECT"),
            (InnerLevel::Debug, InnerMethod::Patch) => inner!(Level::DEBUG, "PATCH"),
            (InnerLevel::Debug, InnerMethod::Other) => inner!(Level::DEBUG, "HTTP"),
            (InnerLevel::Trace, InnerMethod::Options) => inner!(Level::TRACE, "OPTIONS"),
            (InnerLevel::Trace, InnerMethod::Get) => inner!(Level::TRACE, "GET"),
            (InnerLevel::Trace, InnerMethod::Post) => inner!(Level::TRACE, "POST"),
            (InnerLevel::Trace, InnerMethod::Put) => inner!(Level::TRACE, "PUT"),
            (InnerLevel::Trace, InnerMethod::Delete) => inner!(Level::TRACE, "DELETE"),
            (InnerLevel::Trace, InnerMethod::Head) => inner!(Level::TRACE, "HEAD"),
            (InnerLevel::Trace, InnerMethod::Trace) => inner!(Level::TRACE, "TRACE"),
            (InnerLevel::Trace, InnerMethod::Connect) => inner!(Level::TRACE, "CONNECT"),
            (InnerLevel::Trace, InnerMethod::Patch) => inner!(Level::TRACE, "PATCH"),
            (InnerLevel::Trace, InnerMethod::Other) => inner!(Level::TRACE, "HTTP"),
        };

        if let Some(url_scheme) = uri.scheme() {
            span.record("url.scheme", url_scheme.as_str());
        }
        if let Some(user_agent_original) = request.headers().get(USER_AGENT) {
            span.record(
                "user_agent.original",
                user_agent_original.to_str().unwrap_or("invalid-ascii"),
            );
        }

        span
    }
}

impl Default for HttpMakeSpan {
    #[inline]
    fn default() -> Self {
        Self {
            level: Level::INFO,
            network_protocol_name: "http",
            network_transport: NetworkTransport::default(),
        }
    }
}

impl<B> MakeSpan<B> for HttpMakeSpan {
    fn make_span(&mut self, request: &hyper::Request<B>) -> Span {
        self.span_from_request(request)
    }
}

/// Represents the [OSI transport layer] as described in the OpenTelemetry [network] specification.
///
/// [network]: https://opentelemetry.io/docs/specs/semconv/attributes-registry/network/
/// [OSI transport layer]: https://osi-model.com/transport-layer/
#[remain::sorted]
#[derive(Clone, Copy, Debug)]
pub enum NetworkTransport {
    /// Named or anonymous pipe
    Pipe,
    /// TCP
    Tcp,
    /// UDP
    Udp,
    /// Unix domain socket
    Unix,
}

impl NetworkTransport {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Pipe => "pipe",
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Unix => "unix",
        }
    }
}

impl Default for NetworkTransport {
    #[inline]
    fn default() -> Self {
        Self::Tcp
    }
}

#[derive(Clone, Copy, Debug)]
enum HttpVersion {
    Http09,
    Http10,
    Http11,
    Http2,
    Http3,
    Unknown,
}

impl Default for HttpVersion {
    #[inline]
    fn default() -> Self {
        Self::Http11
    }
}

impl From<hyper::Version> for HttpVersion {
    fn from(value: hyper::Version) -> Self {
        match value {
            hyper::Version::HTTP_09 => Self::Http09,
            hyper::Version::HTTP_10 => Self::Http10,
            hyper::Version::HTTP_11 => Self::Http11,
            hyper::Version::HTTP_2 => Self::Http2,
            hyper::Version::HTTP_3 => Self::Http3,
            _ => Self::Unknown,
        }
    }
}

impl HttpVersion {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Http09 => "0.9",
            Self::Http10 => "1.0",
            Self::Http11 => "1.1",
            Self::Http2 => "2",
            Self::Http3 => "3",
            Self::Unknown => "_UNKNOWN",
        }
    }
}