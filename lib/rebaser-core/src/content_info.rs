use core::fmt;
use std::{borrow::Cow, num::ParseIntError, str::FromStr};

use si_data_nats::{HeaderMap, HeaderValue};
use thiserror::Error;

use crate::ApiWrapper;

// X-CONTENT-TYPE: application/json
const NATS_HEADER_CONTENT_TYPE_NAME: &str = "X-CONTENT-TYPE";
// X-MESSAGE-TYPE: EnqueueRequest
const NATS_HEADER_MESSAGE_TYPE_NAME: &str = "X-MESSAGE-TYPE";
// X-MESSAGE-VERSION: 1
const NATS_HEADER_MESSAGE_VERSION_NAME: &str = "X-MESSAGE-VERSION";

#[derive(Debug, Error)]
pub enum HeaderMapParseMessageInfoError {
    #[error("missing nats header: {0}")]
    MissingHeader(&'static str),
    #[error("error parsing message version header: {0}")]
    ParseVersion(#[source] ParseIntError),
}

#[derive(Clone, Debug)]
pub struct ContentInfo<'a> {
    pub content_type: ContentType<'a>,
    pub message_type: MessageType,
    pub message_version: MessageVersion,
}

impl<'a> ContentInfo<'a> {
    pub fn inject_into_headers(&self, headers: &mut HeaderMap) {
        headers.insert(NATS_HEADER_CONTENT_TYPE_NAME, self.content_type.as_str());
        headers.insert(NATS_HEADER_MESSAGE_TYPE_NAME, self.message_type.as_str());
        headers.insert(
            NATS_HEADER_MESSAGE_VERSION_NAME,
            self.message_version.to_string(),
        );
    }
}

impl TryFrom<&HeaderMap> for ContentInfo<'_> {
    type Error = HeaderMapParseMessageInfoError;

    fn try_from(map: &HeaderMap) -> Result<Self, Self::Error> {
        let content_type = ContentType::from(map.get(NATS_HEADER_CONTENT_TYPE_NAME).ok_or(
            HeaderMapParseMessageInfoError::MissingHeader(NATS_HEADER_CONTENT_TYPE_NAME),
        )?);
        let message_type = MessageType::from(map.get(NATS_HEADER_MESSAGE_TYPE_NAME).ok_or(
            HeaderMapParseMessageInfoError::MissingHeader(NATS_HEADER_MESSAGE_TYPE_NAME),
        )?);
        let message_version =
            MessageVersion::try_from(map.get(NATS_HEADER_MESSAGE_VERSION_NAME).ok_or(
                HeaderMapParseMessageInfoError::MissingHeader(NATS_HEADER_MESSAGE_VERSION_NAME),
            )?)
            .map_err(HeaderMapParseMessageInfoError::ParseVersion)?;

        Ok(Self {
            content_type,
            message_type,
            message_version,
        })
    }
}

impl<T> From<&T> for ContentInfo<'static>
where
    T: ApiWrapper,
{
    fn from(_value: &T) -> Self {
        Self {
            content_type: T::default_content_type().into(),
            message_type: T::message_type().into(),
            message_version: T::message_version().into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentType<'a>(Cow<'a, str>);

impl<'a> ContentType<'a> {
    pub const JSON: ContentType<'static> = ContentType(Cow::Borrowed(Self::JSON_STR));
    pub const JSON_STR: &'static str = "application/json";

    pub fn into_inner(self) -> Cow<'a, str> {
        self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<String> for ContentType<'_> {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

impl<'a> From<&'a str> for ContentType<'a> {
    fn from(value: &'a str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<&HeaderValue> for ContentType<'_> {
    fn from(value: &HeaderValue) -> Self {
        Self(Cow::Owned(value.as_str().to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageType(String);

impl MessageType {
    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for MessageType {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for MessageType {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&HeaderValue> for MessageType {
    fn from(value: &HeaderValue) -> Self {
        Self::from(value.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageVersion(u64);

impl MessageVersion {
    pub fn into_inner(self) -> u64 {
        self.0
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for MessageVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<u64> for MessageVersion {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl FromStr for MessageVersion {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str(s).map(Self)
    }
}

impl TryFrom<&HeaderValue> for MessageVersion {
    type Error = ParseIntError;

    fn try_from(value: &HeaderValue) -> Result<Self, Self::Error> {
        Self::from_str(value.as_str())
    }
}