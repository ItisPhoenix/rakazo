//! Secure development-link transport primitives for Charlie WatchOS.
//!
//! The Phase 3 development link has two layers:
//!
//! * the **link layer** (this module): framing, authentication, correlation and
//!   bounded session state; and
//! * the **protocol layer** (`watchos_protocol`): the versioned v3 [`Envelope`]
//!   that carries watch messages.
//!
//! `watchos_protocol` is deliberately left untouched. The link only ever moves
//! protocol envelopes; it never rewrites, reorders or reinterprets them.
//!
//! # Wire format
//!
//! Every frame is a four-byte big-endian payload length followed by the payload:
//!
//! ```text
//! +--------------------+---------------------------+
//! | u32 length (BE)    | JSON link message          |
//! +--------------------+---------------------------+
//! ```
//!
//! The whole frame, length prefix included, may not exceed
//! [`MAX_FRAME_BYTES`] (4096) bytes, so a payload is at most
//! [`MAX_PAYLOAD_BYTES`] (4092) bytes. The limit is checked against the
//! declared length *before* any payload buffer is allocated, exactly as
//! `docs/PROTOCOL.md` requires of an adapter.
//!
//! A link message is a JSON object with a `link` version, a `kind`, optional
//! correlation ids, an optional revision, an optional session token and an
//! optional protocol [`Envelope`]:
//!
//! ```json
//! {"link":1,"kind":"handshake","request_id":1,"token":"…"}
//! {"link":1,"kind":"message","request_id":7,"revision":4,"envelope":{"version":3,"id":7,"message":{…}}}
//! {"link":1,"kind":"ack","request_id":8,"reply_to":7,"revision":5,"envelope":{"version":3,"id":8,"message":{…}}}
//! {"link":1,"kind":"error","request_id":9,"reply_to":7,"code":"conflict","revision":5,"envelope":{…}}
//! ```
//!
//! The revision belongs to the link layer. It records which committed state a
//! request was based on, so a stale request can be refused instead of
//! silently overwriting a newer change. Keeping it here leaves the protocol
//! envelope exactly as `watchos_protocol` defines it.
//!
//! # Scope
//!
//! This module opens no sockets, starts no threads and touches no application
//! state. It is compiled and unit tested today; a later step wires it into the
//! simulator runtime. Nothing here may be reached in a normal simulator run.

pub mod server;

use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use watchos_protocol::Envelope;

/// Version carried by every link-layer message.
pub const LINK_VERSION: u16 = 1;

/// Bytes used by the frame length prefix.
pub const FRAME_PREFIX_BYTES: usize = 4;

/// Maximum bytes a single frame may occupy on the wire, length prefix included.
/// Mirrors `watchos_protocol::MAX_FRAME_BYTES`.
pub const MAX_FRAME_BYTES: usize = watchos_protocol::MAX_FRAME_BYTES;

/// Maximum JSON payload bytes that fit inside one frame.
pub const MAX_PAYLOAD_BYTES: usize = MAX_FRAME_BYTES - FRAME_PREFIX_BYTES;

/// Default number of in-flight requests one session tracks.
pub const DEFAULT_MAX_PENDING: usize = 32;

/// Default number of completed request replies one session remembers.
pub const DEFAULT_MAX_COMPLETED: usize = 64;

/// Every failure the link layer can report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// A frame declared more bytes than the ingress cap allows. The payload was
    /// refused before a buffer was allocated for it.
    FrameTooLarge { declared: usize, limit: usize },
    /// A frame declared zero payload bytes.
    EmptyFrame,
    /// A payload was not valid UTF-8.
    InvalidUtf8,
    /// A payload was not valid JSON, or not a valid link message.
    Malformed(String),
    /// A link message failed validation.
    Invalid(String),
    /// The presented session token did not match.
    Unauthorized,
    /// A message arrived before the handshake completed.
    NotAuthenticated,
    /// A second handshake arrived on an authenticated session.
    AlreadyAuthenticated,
    /// The peer closed the connection.
    Closed,
    /// A read or write did not finish inside its deadline.
    Timeout,
    /// A bounded table or queue is full.
    Full,
    /// A request id was already completed and its reply is no longer cached.
    AlreadyCompleted(u64),
    /// Underlying I/O failed.
    Io {
        kind: io::ErrorKind,
        message: String,
    },
}

impl LinkError {
    /// True when the failure was a deadline expiry rather than a hard error.
    pub fn is_timeout(&self) -> bool {
        matches!(self, LinkError::Timeout)
    }
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::FrameTooLarge { declared, limit } => {
                write!(
                    f,
                    "frame of {declared} bytes exceeds the {limit} byte limit"
                )
            }
            LinkError::EmptyFrame => write!(f, "frame carried no payload"),
            LinkError::InvalidUtf8 => write!(f, "frame payload was not valid UTF-8"),
            LinkError::Malformed(detail) => write!(f, "malformed link message: {detail}"),
            LinkError::Invalid(detail) => write!(f, "invalid link message: {detail}"),
            LinkError::Unauthorized => write!(f, "session token rejected"),
            LinkError::NotAuthenticated => write!(f, "handshake has not completed"),
            LinkError::AlreadyAuthenticated => write!(f, "session is already authenticated"),
            LinkError::Closed => write!(f, "peer closed the connection"),
            LinkError::Timeout => write!(f, "operation exceeded its deadline"),
            LinkError::Full => write!(f, "bounded queue is full"),
            LinkError::AlreadyCompleted(id) => write!(f, "request {id} was already completed"),
            LinkError::Io { kind, message } => write!(f, "io error ({kind:?}): {message}"),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<io::Error> for LinkError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => LinkError::Timeout,
            io::ErrorKind::UnexpectedEof => LinkError::Closed,
            kind => {
                let message = error.to_string();
                let message = if message.len() > 200 {
                    let mut truncated = message[..200].to_string();
                    truncated.push('…');
                    truncated
                } else {
                    message
                };
                LinkError::Io { kind, message }
            }
        }
    }
}

/// What a link message does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// Client to server: present a session token.
    Handshake,
    /// Client to server: ask for the server's current state.
    Hello,
    /// Either direction: carry a protocol v3 envelope.
    Message,
    /// Answer a request, optionally echoing the applied state.
    Ack,
    /// Report a failure with a machine-readable code.
    Error,
    /// Orderly close.
    Bye,
}

/// Machine-readable failure reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkCode {
    /// The session token did not match.
    Unauthorized,
    /// The message arrived before authentication.
    NotAuthenticated,
    /// The request id was already completed.
    AlreadyCompleted,
    /// The server state moved on before the request could be applied.
    Conflict,
    /// The message exceeded the frame limit.
    TooLarge,
    /// The frame was not decodable.
    BadFrame,
    /// The message was not valid JSON or not a valid link message.
    Malformed,
    /// The request timed out.
    Timeout,
    /// The requested resource does not exist.
    NotFound,
    /// The request did not state which revision it was based on.
    RevisionRequired,
    /// The watch has an open customization draft, so the request was refused.
    DraftActive,
    /// The requested change was not acceptable.
    Invalid,
    /// The change could not be written to the settings file.
    StorageFailed,
    /// An unexpected server-side failure.
    Internal,
}

/// A versioned link-layer message.
///
/// The session token is the only credential this type carries. It is never
/// rendered by `Debug`, so it cannot reach a log by accident.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkMessage {
    /// Link-layer version. Only [`LINK_VERSION`] is understood.
    pub link: u16,
    /// What this message does.
    pub kind: LinkKind,
    /// Correlation id assigned by the sender. Never zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<u64>,
    /// Correlation id of the request this message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<u64>,
    /// Session token. Only ever present on a handshake.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Failure reason. Only ever present on an error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<LinkCode>,
    /// Revision of the committed state this message is based on, or the
    /// revision it reports. Revisions live in the link layer so the protocol
    /// envelope stays exactly as `watchos_protocol` defines it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    /// Protocol v3 envelope. Present on messages, optionally on acknowledgements
    /// and on errors that report the authoritative state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope: Option<Envelope>,
}

impl fmt::Debug for LinkMessage {
    /// Renders the message without the session token and without payload text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkMessage")
            .field("link", &self.link)
            .field("kind", &self.kind)
            .field("request_id", &self.request_id)
            .field("reply_to", &self.reply_to)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("code", &self.code)
            .field("revision", &self.revision)
            .field(
                "envelope",
                &self
                    .envelope
                    .as_ref()
                    .map(|envelope| (envelope.version, envelope.id)),
            )
            .finish()
    }
}

impl LinkMessage {
    /// A handshake that presents a session token.
    pub fn handshake(token: impl Into<String>, request_id: u64) -> Self {
        Self {
            link: LINK_VERSION,
            kind: LinkKind::Handshake,
            request_id: Some(request_id),
            reply_to: None,
            token: Some(token.into()),
            code: None,
            revision: None,
            envelope: None,
        }
    }

    /// A request for the peer's current state.
    pub fn hello(request_id: u64) -> Self {
        Self {
            link: LINK_VERSION,
            kind: LinkKind::Hello,
            request_id: Some(request_id),
            reply_to: None,
            token: None,
            code: None,
            revision: None,
            envelope: None,
        }
    }

    /// A protocol v3 envelope with a correlation id.
    pub fn message(request_id: u64, envelope: Envelope) -> Self {
        Self {
            link: LINK_VERSION,
            kind: LinkKind::Message,
            request_id: Some(request_id),
            reply_to: None,
            token: None,
            code: None,
            revision: None,
            envelope: Some(envelope),
        }
    }

    /// An acknowledgement of `reply_to`.
    pub fn ack(request_id: u64, reply_to: u64, envelope: Option<Envelope>) -> Self {
        Self {
            link: LINK_VERSION,
            kind: LinkKind::Ack,
            request_id: Some(request_id),
            reply_to: Some(reply_to),
            token: None,
            code: None,
            revision: None,
            envelope,
        }
    }

    /// A failure report.
    pub fn error(request_id: Option<u64>, reply_to: Option<u64>, code: LinkCode) -> Self {
        Self {
            link: LINK_VERSION,
            kind: LinkKind::Error,
            request_id,
            reply_to,
            token: None,
            code: Some(code),
            revision: None,
            envelope: None,
        }
    }

    /// An orderly close.
    pub fn bye() -> Self {
        Self {
            link: LINK_VERSION,
            kind: LinkKind::Bye,
            request_id: None,
            reply_to: None,
            token: None,
            code: None,
            revision: None,
            envelope: None,
        }
    }

    /// State the revision this message is based on, or reports.
    pub fn with_revision(mut self, revision: u64) -> Self {
        self.revision = Some(revision);
        self
    }

    /// Attach the authoritative protocol state.
    pub fn with_envelope(mut self, envelope: Envelope) -> Self {
        self.envelope = Some(envelope);
        self
    }

    /// Check the message against the link rules.
    ///
    /// Every field combination is checked explicitly so that a malformed or
    /// hostile peer cannot smuggle a token, an envelope or a code into a kind
    /// that does not expect it.
    pub fn validate(&self) -> Result<(), LinkError> {
        if self.link != LINK_VERSION {
            return Err(LinkError::Invalid(format!(
                "unsupported link version {}",
                self.link
            )));
        }
        match self.kind {
            LinkKind::Handshake => {
                if self.token.as_deref().map(str::is_empty).unwrap_or(true) {
                    return Err(LinkError::Invalid(
                        "handshake without a session token".into(),
                    ));
                }
                self.require_request_id()?;
                if self.envelope.is_some()
                    || self.reply_to.is_some()
                    || self.code.is_some()
                    || self.revision.is_some()
                {
                    return Err(LinkError::Invalid(
                        "handshake carries unexpected fields".into(),
                    ));
                }
            }
            LinkKind::Hello => {
                self.require_request_id()?;
                if self.token.is_some()
                    || self.envelope.is_some()
                    || self.code.is_some()
                    || self.revision.is_some()
                {
                    return Err(LinkError::Invalid("hello carries unexpected fields".into()));
                }
            }
            LinkKind::Message => {
                self.require_request_id()?;
                if self.token.is_some() || self.code.is_some() {
                    return Err(LinkError::Invalid(
                        "message carries unexpected fields".into(),
                    ));
                }
                let envelope = self
                    .envelope
                    .as_ref()
                    .ok_or_else(|| LinkError::Invalid("message without an envelope".into()))?;
                if envelope.id != self.request_id.unwrap_or_default() {
                    return Err(LinkError::Invalid(
                        "envelope id does not match the request id".into(),
                    ));
                }
                envelope.validate().map_err(|error| {
                    LinkError::Invalid(format!("invalid protocol envelope: {error:?}"))
                })?;
            }
            LinkKind::Ack => {
                self.require_request_id()?;
                if self.reply_to.is_none() {
                    return Err(LinkError::Invalid(
                        "acknowledgement without a reply id".into(),
                    ));
                }
                if self.token.is_some() || self.code.is_some() {
                    return Err(LinkError::Invalid(
                        "acknowledgement carries unexpected fields".into(),
                    ));
                }
            }
            LinkKind::Error => {
                if self.code.is_none() {
                    return Err(LinkError::Invalid("error without a code".into()));
                }
                if self.token.is_some() {
                    return Err(LinkError::Invalid("error carries unexpected fields".into()));
                }
            }
            LinkKind::Bye => {
                if self.token.is_some()
                    || self.envelope.is_some()
                    || self.code.is_some()
                    || self.revision.is_some()
                {
                    return Err(LinkError::Invalid("bye carries unexpected fields".into()));
                }
            }
        }
        Ok(())
    }

    /// Encode the message as bounded JSON.
    pub fn to_json(&self) -> Result<Vec<u8>, LinkError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| LinkError::Malformed("link message could not be encoded".into()))?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(LinkError::FrameTooLarge {
                declared: bytes.len(),
                limit: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }

    /// Decode and validate a message from bounded JSON.
    pub fn from_json(bytes: &[u8]) -> Result<Self, LinkError> {
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(LinkError::FrameTooLarge {
                declared: bytes.len(),
                limit: MAX_PAYLOAD_BYTES,
            });
        }
        let message: LinkMessage = serde_json::from_slice(bytes)
            .map_err(|error| LinkError::Malformed(sanitize_json_error(&error)))?;
        message.validate()?;
        Ok(message)
    }

    fn require_request_id(&self) -> Result<(), LinkError> {
        match self.request_id {
            Some(0) | None => Err(LinkError::Invalid("missing request id".into())),
            Some(_) => Ok(()),
        }
    }
}

/// Keep JSON failure text free of payload content.
///
/// A serde error can quote the value that failed to parse, so only the position
/// of the failure is reported.
fn sanitize_json_error(error: &serde_json::Error) -> String {
    format!(
        "invalid json at line {} column {}",
        error.line(),
        error.column()
    )
}

/// Compare two byte strings without an early exit.
///
/// The loop always runs to the longer of the two lengths, so neither the
/// content nor the length of the compared values decides how long the
/// comparison takes.
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = (left.len() ^ right.len()) as u8;
    let length = left.len().max(right.len());
    for index in 0..length {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= left_byte ^ right_byte;
    }
    difference == 0
}

/// Check a presented session token against the expected one.
pub fn verify_token(presented: &str, expected: &str) -> bool {
    constant_time_eq(presented.as_bytes(), expected.as_bytes())
}

/// Write one length-prefixed frame.
///
/// An oversized payload is refused before a single byte reaches the writer, so
/// a rejected message can never leave a half-written frame on the wire.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), LinkError> {
    if payload.is_empty() {
        return Err(LinkError::EmptyFrame);
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(LinkError::FrameTooLarge {
            declared: payload.len(),
            limit: MAX_PAYLOAD_BYTES,
        });
    }
    let length = u32::try_from(payload.len()).map_err(|_| LinkError::FrameTooLarge {
        declared: payload.len(),
        limit: MAX_PAYLOAD_BYTES,
    })?;
    let mut framed = Vec::with_capacity(FRAME_PREFIX_BYTES + payload.len());
    framed.extend_from_slice(&length.to_be_bytes());
    framed.extend_from_slice(payload);
    writer.write_all(&framed).map_err(LinkError::from)
}

/// Read one length-prefixed frame.
///
/// The declared length is checked against the ingress cap before a payload
/// buffer is allocated, and the payload is then read to completion, so a frame
/// split across several reads is reassembled.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>, LinkError> {
    let mut prefix = [0u8; FRAME_PREFIX_BYTES];
    reader.read_exact(&mut prefix).map_err(LinkError::from)?;
    let declared = u32::from_be_bytes(prefix) as usize;
    if declared == 0 {
        return Err(LinkError::EmptyFrame);
    }
    if declared > MAX_PAYLOAD_BYTES {
        return Err(LinkError::FrameTooLarge {
            declared,
            limit: MAX_PAYLOAD_BYTES,
        });
    }
    let mut payload = vec![0u8; declared];
    reader.read_exact(&mut payload).map_err(LinkError::from)?;
    Ok(payload)
}

/// Write one link message as a framed payload.
pub fn write_message<W: Write>(writer: &mut W, message: &LinkMessage) -> Result<(), LinkError> {
    message.validate()?;
    let payload = message.to_json()?;
    write_frame(writer, &payload)
}

/// Read and decode one link message.
pub fn read_message<R: Read>(reader: &mut R) -> Result<LinkMessage, LinkError> {
    let payload = read_frame(reader)?;
    LinkMessage::from_json(&payload)
}

/// Outcome of one incremental read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fill {
    /// More bytes arrived.
    Progress,
    /// The stream had nothing to offer.
    Idle,
}

/// Incremental frame reader for a stream that may deliver data in pieces.
///
/// [`read_frame`] waits for a whole frame. `FrameReader` instead reports
/// `Ok(None)` when the stream had nothing to offer, so a caller can poll a
/// stream that carries a read deadline without mistaking an idle link for a
/// closed one. A deadline that expires in the middle of a frame means the
/// frame was truncated, and the session ends.
pub struct FrameReader {
    buffer: Vec<u8>,
    expected: Option<usize>,
}

impl FrameReader {
    /// A reader that has not seen any bytes yet.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            expected: None,
        }
    }

    /// Read the next frame, or report that the stream was idle.
    ///
    /// The declared length is validated before the buffer is allowed to grow,
    /// so a hostile peer cannot make the reader allocate beyond the frame cap.
    pub fn read_frame<R: Read>(&mut self, reader: &mut R) -> Result<Option<Vec<u8>>, LinkError> {
        loop {
            if self.expected.is_none() {
                if self.buffer.len() < FRAME_PREFIX_BYTES {
                    match self.fill(reader)? {
                        Fill::Progress => continue,
                        Fill::Idle => return Ok(None),
                    }
                }
                let declared = u32::from_be_bytes([
                    self.buffer[0],
                    self.buffer[1],
                    self.buffer[2],
                    self.buffer[3],
                ]) as usize;
                if declared == 0 {
                    return Err(LinkError::EmptyFrame);
                }
                if declared > MAX_PAYLOAD_BYTES {
                    return Err(LinkError::FrameTooLarge {
                        declared,
                        limit: MAX_PAYLOAD_BYTES,
                    });
                }
                self.expected = Some(declared);
            }
            let expected = self.expected.unwrap_or(0);
            let total = FRAME_PREFIX_BYTES + expected;
            if self.buffer.len() < total {
                match self.fill(reader)? {
                    Fill::Progress => continue,
                    // A deadline inside a frame means the frame was truncated.
                    Fill::Idle => return Err(LinkError::Closed),
                }
            }
            let payload = self.buffer[FRAME_PREFIX_BYTES..total].to_vec();
            self.buffer.drain(..total);
            self.expected = None;
            return Ok(Some(payload));
        }
    }

    fn fill<R: Read>(&mut self, reader: &mut R) -> Result<Fill, LinkError> {
        let mut chunk = [0u8; 512];
        match reader.read(&mut chunk) {
            Ok(0) => Err(LinkError::Closed),
            Ok(count) => {
                if self.buffer.len() + count > FRAME_PREFIX_BYTES + MAX_PAYLOAD_BYTES {
                    return Err(LinkError::FrameTooLarge {
                        declared: self.buffer.len() + count,
                        limit: FRAME_PREFIX_BYTES + MAX_PAYLOAD_BYTES,
                    });
                }
                self.buffer.extend_from_slice(&chunk[..count]);
                Ok(Fill::Progress)
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut =>
            {
                if self.buffer.is_empty() {
                    Ok(Fill::Idle)
                } else {
                    Err(LinkError::Closed)
                }
            }
            Err(error) => Err(LinkError::from(error)),
        }
    }
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

/// Read and decode deadlines used by the transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeouts {
    /// How long a single read may block.
    pub read: Duration,
    /// How long a single write may block.
    pub write: Duration,
    /// How long an unanswered request stays pending.
    pub request: Duration,
}

impl Timeouts {
    /// Deadlines that never expire.
    pub const fn none() -> Self {
        Self {
            read: Duration::ZERO,
            write: Duration::ZERO,
            request: Duration::ZERO,
        }
    }

    /// Apply the read and write deadlines to an open stream.
    ///
    /// A zero deadline leaves that direction unblocked. The caller decides when
    /// to use it; this module never opens a connection.
    pub fn apply(&self, stream: &TcpStream) -> io::Result<()> {
        if !self.read.is_zero() {
            stream.set_read_timeout(Some(self.read))?;
        }
        if !self.write.is_zero() {
            stream.set_write_timeout(Some(self.write))?;
        }
        Ok(())
    }
}

/// Lifecycle of a link session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    /// The handshake has not completed yet.
    AwaitingHandshake,
    /// The peer presented the right token.
    Authenticated,
    /// The session is finished.
    Closed,
}

/// A request that has been sent and not yet answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingRequest {
    /// Correlation id of the request.
    pub request_id: u64,
    /// When the request was registered.
    pub sent_at: Instant,
    /// How long the request may stay unanswered.
    pub deadline: Duration,
}

impl PendingRequest {
    /// True when the request has outlived its deadline.
    pub fn is_expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.sent_at) >= self.deadline
    }
}

/// A request that was answered, kept so a repeat can be answered identically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedRequest {
    /// Correlation id of the request.
    pub request_id: u64,
    /// The reply that was sent for it.
    pub reply: LinkMessage,
}

/// What the caller must do with an accepted message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// The handshake succeeded and the session is now authenticated.
    Authenticated,
    /// A new request that the caller has to apply.
    Apply {
        /// Correlation id of the request.
        request_id: u64,
        /// The protocol envelope to apply.
        envelope: Envelope,
    },
    /// A repeated request. The cached reply has to be sent again unchanged.
    Repeat {
        /// Correlation id of the request.
        request_id: u64,
        /// The reply that was sent the first time.
        reply: LinkMessage,
    },
    /// A request for the current state. The caller has to answer it.
    Hello {
        /// Correlation id of the request.
        request_id: u64,
    },
    /// The peer asked to close the session.
    Bye,
    /// Nothing to do for this message.
    Ignored,
}

/// A bounded queue of outbound link messages.
#[derive(Clone, Debug)]
pub struct OutboundQueue {
    capacity: usize,
    items: VecDeque<LinkMessage>,
}

impl OutboundQueue {
    /// A queue that holds at most `capacity` messages.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: VecDeque::new(),
        }
    }

    /// The largest number of messages the queue accepts.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of queued messages.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Queue a message, refusing it when the queue is full.
    pub fn push(&mut self, message: LinkMessage) -> Result<(), LinkError> {
        if self.items.len() >= self.capacity {
            return Err(LinkError::Full);
        }
        self.items.push_back(message);
        Ok(())
    }

    /// Take the oldest queued message.
    pub fn pop(&mut self) -> Option<LinkMessage> {
        self.items.pop_front()
    }

    /// Drop every queued message.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// One link session: authentication, correlation and bounded state.
///
/// The session holds no socket. It decides what a peer is allowed to say and
/// what the caller still has to do.
#[derive(Clone, Debug)]
pub struct Session {
    state: SessionState,
    expected_token: String,
    max_pending: usize,
    max_completed: usize,
    pending: VecDeque<PendingRequest>,
    completed: VecDeque<CompletedRequest>,
    highest_completed: u64,
    next_request_id: u64,
}

impl Session {
    /// A session that expects `expected_token` and uses the default limits.
    pub fn new(expected_token: impl Into<String>) -> Self {
        Self::with_limits(expected_token, DEFAULT_MAX_PENDING, DEFAULT_MAX_COMPLETED)
    }

    /// A session with explicit bounds on its pending and completed tables.
    pub fn with_limits(
        expected_token: impl Into<String>,
        max_pending: usize,
        max_completed: usize,
    ) -> Self {
        Self {
            state: SessionState::AwaitingHandshake,
            expected_token: expected_token.into(),
            max_pending,
            max_completed,
            pending: VecDeque::new(),
            completed: VecDeque::new(),
            highest_completed: 0,
            next_request_id: 1,
        }
    }

    /// The current lifecycle state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// True once the peer proved it knows the session token.
    pub fn is_authenticated(&self) -> bool {
        self.state == SessionState::Authenticated
    }

    /// Number of requests that are still unanswered.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Number of replies kept for repeated requests.
    pub fn completed_len(&self) -> usize {
        self.completed.len()
    }

    /// The next correlation id for a new request. Ids start at one.
    pub fn next_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1).max(1);
        id
    }

    /// Authenticate a peer against the session token.
    ///
    /// A wrong token closes the session: a failed handshake must not leave a
    /// session that a later message could use.
    pub fn authenticate(&mut self, presented: &str) -> Result<(), LinkError> {
        if self.state == SessionState::Authenticated {
            return Err(LinkError::AlreadyAuthenticated);
        }
        if !verify_token(presented, &self.expected_token) {
            self.state = SessionState::Closed;
            return Err(LinkError::Unauthorized);
        }
        self.state = SessionState::Authenticated;
        Ok(())
    }

    /// Decide what the caller must do with an incoming message.
    ///
    /// Unauthenticated messages are refused. A request id that was already
    /// answered is never applied twice: the cached reply is returned so a retry
    /// from the peer produces the same answer.
    pub fn accept(&mut self, message: &LinkMessage) -> Result<Disposition, LinkError> {
        message.validate()?;
        match message.kind {
            LinkKind::Bye => {
                self.state = SessionState::Closed;
                return Ok(Disposition::Bye);
            }
            LinkKind::Handshake => {
                if self.state == SessionState::Authenticated {
                    return Err(LinkError::AlreadyAuthenticated);
                }
                let presented = message.token.as_deref().ok_or(LinkError::Unauthorized)?;
                self.authenticate(presented)?;
                return Ok(Disposition::Authenticated);
            }
            _ => {}
        }
        if self.state != SessionState::Authenticated {
            return Err(LinkError::NotAuthenticated);
        }
        match message.kind {
            LinkKind::Hello => {
                let request_id = message
                    .request_id
                    .ok_or_else(|| LinkError::Invalid("hello without a request id".into()))?;
                Ok(Disposition::Hello { request_id })
            }
            LinkKind::Message => {
                let request_id = message
                    .request_id
                    .ok_or_else(|| LinkError::Invalid("message without a request id".into()))?;
                let envelope = message
                    .envelope
                    .clone()
                    .ok_or_else(|| LinkError::Invalid("message without an envelope".into()))?;
                if request_id <= self.highest_completed {
                    let reply = self
                        .completed
                        .iter()
                        .find(|completed| completed.request_id == request_id)
                        .map(|completed| completed.reply.clone())
                        .unwrap_or_else(|| {
                            LinkMessage::error(None, Some(request_id), LinkCode::AlreadyCompleted)
                        });
                    return Ok(Disposition::Repeat { request_id, reply });
                }
                Ok(Disposition::Apply {
                    request_id,
                    envelope,
                })
            }
            LinkKind::Ack | LinkKind::Error => Ok(Disposition::Ignored),
            LinkKind::Handshake | LinkKind::Bye => unreachable!("handled above"),
        }
    }

    /// Record the answer to a request so a repeat is answered identically.
    pub fn complete(&mut self, request_id: u64, reply: LinkMessage) {
        self.pending
            .retain(|pending| pending.request_id != request_id);
        self.highest_completed = self.highest_completed.max(request_id);
        self.completed
            .push_back(CompletedRequest { request_id, reply });
        while self.completed.len() > self.max_completed {
            self.completed.pop_front();
        }
    }

    /// Track an unanswered request, refusing it when the table is full.
    pub fn register_pending(
        &mut self,
        request_id: u64,
        deadline: Duration,
    ) -> Result<(), LinkError> {
        if self.pending.len() >= self.max_pending {
            return Err(LinkError::Full);
        }
        self.pending.push_back(PendingRequest {
            request_id,
            sent_at: Instant::now(),
            deadline,
        });
        Ok(())
    }

    /// Forget a tracked request.
    pub fn take_pending(&mut self, request_id: u64) -> Option<PendingRequest> {
        let position = self
            .pending
            .iter()
            .position(|pending| pending.request_id == request_id)?;
        self.pending.remove(position)
    }

    /// The requests that outlived their deadline, oldest first.
    pub fn expire_pending(&mut self, now: Instant) -> Vec<u64> {
        let mut expired = Vec::new();
        while let Some(front) = self.pending.front() {
            if !front.is_expired(now) {
                break;
            }
            if let Some(pending) = self.pending.pop_front() {
                expired.push(pending.request_id);
            }
        }
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use watchos_protocol::{Appearance, Customization, Face, Message};

    /// A placeholder credential used only by the unit tests.
    const TOKEN: &str = "unit-test-session-value";

    fn envelope(id: u64) -> Envelope {
        Envelope::new(
            id,
            Message::Customization(Customization {
                appearance: Appearance {
                    face: Face::Classic,
                    ..Appearance::default()
                },
                use_24h: false,
                reduce_motion: true,
            }),
        )
    }

    /// A reader that hands out at most `chunk` bytes per call so fragmented
    /// reads can be exercised.
    struct CountingReader {
        data: Vec<u8>,
        position: usize,
        chunk: usize,
    }

    impl CountingReader {
        fn new(data: Vec<u8>, chunk: usize) -> Self {
            Self {
                data,
                position: 0,
                chunk,
            }
        }

        fn consumed(&self) -> usize {
            self.position
        }
    }

    impl Read for CountingReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.position >= self.data.len() {
                return Ok(0);
            }
            let count = buffer
                .len()
                .min(self.chunk)
                .min(self.data.len() - self.position);
            buffer[..count].copy_from_slice(&self.data[self.position..self.position + count]);
            self.position += count;
            Ok(count)
        }
    }

    /// A writer that records everything it is given.
    #[derive(Default)]
    struct CountingWriter {
        written: Vec<u8>,
    }

    impl Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn frames_round_trip() {
        let message = LinkMessage::hello(1);
        let payload = message.to_json().expect("encode");
        let mut writer = CountingWriter::default();
        write_frame(&mut writer, &payload).expect("write");
        assert_eq!(writer.written.len(), FRAME_PREFIX_BYTES + payload.len());
        assert_eq!(
            u32::from_be_bytes(writer.written[..4].try_into().unwrap()),
            payload.len() as u32
        );
        let decoded = read_frame(&mut Cursor::new(writer.written)).expect("read");
        assert_eq!(decoded, payload);
        assert_eq!(LinkMessage::from_json(&decoded).expect("decode"), message);
    }

    #[test]
    fn fragmented_reads_are_reassembled() {
        let message = LinkMessage::message(7, envelope(7));
        let payload = message.to_json().expect("encode");
        let mut writer = CountingWriter::default();
        write_frame(&mut writer, &payload).expect("write");
        let mut reader = CountingReader::new(writer.written, 1);
        let decoded = read_frame(&mut reader).expect("read");
        assert_eq!(decoded, payload);
        assert_eq!(reader.consumed(), FRAME_PREFIX_BYTES + payload.len());
    }

    #[test]
    fn oversized_frame_is_refused_before_allocation() {
        let mut prefix = ((MAX_PAYLOAD_BYTES + 1) as u32).to_be_bytes().to_vec();
        prefix.extend_from_slice(b"ignored");
        let mut reader = CountingReader::new(prefix, 64);
        let error = read_frame(&mut reader).expect_err("oversized frame must fail");
        assert_eq!(
            error,
            LinkError::FrameTooLarge {
                declared: MAX_PAYLOAD_BYTES + 1,
                limit: MAX_PAYLOAD_BYTES
            }
        );
        // Only the prefix may have been consumed; the payload is never read.
        assert_eq!(reader.consumed(), FRAME_PREFIX_BYTES);
    }

    #[test]
    fn empty_frames_are_refused() {
        let mut writer = CountingWriter::default();
        assert_eq!(write_frame(&mut writer, &[]), Err(LinkError::EmptyFrame));
        assert!(writer.written.is_empty());
        let mut reader = CountingReader::new(vec![0, 0, 0, 0], 4);
        assert_eq!(read_frame(&mut reader), Err(LinkError::EmptyFrame));
    }

    #[test]
    fn oversized_payload_is_never_written() {
        let mut writer = CountingWriter::default();
        let payload = vec![b'x'; MAX_PAYLOAD_BYTES + 1];
        let error = write_frame(&mut writer, &payload).expect_err("must fail");
        assert_eq!(
            error,
            LinkError::FrameTooLarge {
                declared: MAX_PAYLOAD_BYTES + 1,
                limit: MAX_PAYLOAD_BYTES
            }
        );
        assert!(writer.written.is_empty());
    }

    #[test]
    fn malformed_json_is_refused_without_echoing_the_payload() {
        let secret = "super-secret-value";
        let payload = format!("{{\"link\":1,\"kind\":\"{secret}\"}}");
        let error = LinkMessage::from_json(payload.as_bytes()).expect_err("must fail");
        assert!(matches!(error, LinkError::Malformed(_)));
        assert!(
            !error.to_string().contains(secret),
            "error text must not quote payload content"
        );
    }

    #[test]
    fn unknown_fields_and_versions_are_refused() {
        let unknown_field = br#"{"link":1,"kind":"bye","extra":true}"#;
        assert!(matches!(
            LinkMessage::from_json(unknown_field),
            Err(LinkError::Malformed(_))
        ));
        let wrong_version = br#"{"link":9,"kind":"bye"}"#;
        assert!(matches!(
            LinkMessage::from_json(wrong_version),
            Err(LinkError::Invalid(_))
        ));
    }

    #[test]
    fn a_valid_token_authenticates_and_a_wrong_one_does_not() {
        let mut session = Session::new(TOKEN);
        assert_eq!(session.state(), SessionState::AwaitingHandshake);
        let handshake = LinkMessage::handshake(TOKEN, 1);
        assert_eq!(session.accept(&handshake), Ok(Disposition::Authenticated));
        assert!(session.is_authenticated());
        assert_eq!(
            session.accept(&handshake),
            Err(LinkError::AlreadyAuthenticated)
        );

        let mut session = Session::new(TOKEN);
        let wrong = LinkMessage::handshake("not-the-token", 1);
        assert_eq!(session.accept(&wrong), Err(LinkError::Unauthorized));
        assert_eq!(session.state(), SessionState::Closed);
    }

    #[test]
    fn unauthenticated_messages_are_refused() {
        let mut session = Session::new(TOKEN);
        let request = LinkMessage::message(3, envelope(3));
        assert_eq!(session.accept(&request), Err(LinkError::NotAuthenticated));
        assert_eq!(
            session.accept(&LinkMessage::hello(3)),
            Err(LinkError::NotAuthenticated)
        );
    }

    #[test]
    fn handshake_requires_a_token() {
        let mut session = Session::new(TOKEN);
        let mut handshake = LinkMessage::handshake(TOKEN, 1);
        handshake.token = None;
        assert!(matches!(
            session.accept(&handshake),
            Err(LinkError::Invalid(_))
        ));
        assert_eq!(session.state(), SessionState::AwaitingHandshake);
    }

    #[test]
    fn a_repeated_request_is_answered_from_the_cache() {
        let mut session = Session::new(TOKEN);
        session
            .accept(&LinkMessage::handshake(TOKEN, 1))
            .expect("handshake");
        let request = LinkMessage::message(7, envelope(7));
        assert!(matches!(
            session.accept(&request),
            Ok(Disposition::Apply { request_id: 7, .. })
        ));
        let reply = LinkMessage::ack(8, 7, Some(envelope(7)));
        session.complete(7, reply.clone());
        let repeated = session.accept(&request).expect("repeat");
        match repeated {
            Disposition::Repeat {
                request_id,
                reply: cached,
            } => {
                assert_eq!(request_id, 7);
                assert_eq!(cached, reply);
            }
            other => panic!("expected a repeat, got {other:?}"),
        }
    }

    #[test]
    fn an_old_request_id_is_never_applied_again() {
        let mut session = Session::with_limits(TOKEN, 8, 1);
        session
            .accept(&LinkMessage::handshake(TOKEN, 1))
            .expect("handshake");
        let first = LinkMessage::message(2, envelope(2));
        session.accept(&first).expect("first");
        session.complete(2, LinkMessage::ack(3, 2, None));
        // The cache holds one reply, so request 2 is evicted.
        let second = LinkMessage::message(4, envelope(4));
        session.accept(&second).expect("second");
        session.complete(4, LinkMessage::ack(5, 4, None));
        match session.accept(&first).expect("replay") {
            Disposition::Repeat { reply, .. } => {
                assert_eq!(reply.code, Some(LinkCode::AlreadyCompleted));
            }
            other => panic!("expected a repeat, got {other:?}"),
        }
    }

    #[test]
    fn pending_requests_are_bounded() {
        let mut session = Session::with_limits(TOKEN, 2, 4);
        session
            .register_pending(1, Duration::from_secs(1))
            .expect("first");
        session
            .register_pending(2, Duration::from_secs(1))
            .expect("second");
        assert_eq!(
            session.register_pending(3, Duration::from_secs(1)),
            Err(LinkError::Full)
        );
        assert_eq!(session.pending_len(), 2);
    }

    #[test]
    fn completed_replies_are_bounded() {
        let mut session = Session::with_limits(TOKEN, 8, 2);
        for id in 1..=4 {
            session.complete(id, LinkMessage::ack(id + 100, id, None));
        }
        assert_eq!(session.completed_len(), 2);
    }

    #[test]
    fn pending_requests_expire_on_their_deadline() {
        let mut session = Session::new(TOKEN);
        session
            .register_pending(1, Duration::from_millis(1))
            .expect("register");
        session
            .register_pending(2, Duration::from_secs(600))
            .expect("register");
        let expired = session.expire_pending(Instant::now() + Duration::from_secs(1));
        assert_eq!(expired, vec![1]);
        assert_eq!(session.pending_len(), 1);
    }

    #[test]
    fn a_pending_request_can_be_forgotten() {
        let mut session = Session::new(TOKEN);
        session
            .register_pending(9, Duration::from_secs(60))
            .expect("register");
        assert!(session.take_pending(9).is_some());
        assert!(session.take_pending(9).is_none());
        assert_eq!(session.pending_len(), 0);
    }

    #[test]
    fn request_ids_are_monotonic_and_never_zero() {
        let mut session = Session::new(TOKEN);
        assert_eq!(session.next_request_id(), 1);
        assert_eq!(session.next_request_id(), 2);
        assert_eq!(session.next_request_id(), 3);
    }

    #[test]
    fn the_token_never_reaches_a_log() {
        let handshake = LinkMessage::handshake(TOKEN, 1);
        let rendered = format!("{handshake:?}");
        assert!(!rendered.contains(TOKEN));
        assert!(rendered.contains("<redacted>"));
        let encoded = handshake.to_json().expect("encode");
        let decoded = LinkMessage::from_json(&encoded).expect("decode");
        assert_eq!(decoded.token.as_deref(), Some(TOKEN));
        assert!(!format!("{decoded:?}").contains(TOKEN));
    }

    #[test]
    fn an_invalid_protocol_envelope_is_refused() {
        let mut broken = envelope(5);
        if let Message::Customization(customization) = &mut broken.message {
            customization.appearance.glow = 200;
        }
        let message = LinkMessage::message(5, broken);
        assert!(matches!(message.validate(), Err(LinkError::Invalid(_))));
    }

    #[test]
    fn an_envelope_id_must_match_its_request_id() {
        let message = LinkMessage::message(6, envelope(7));
        assert!(matches!(message.validate(), Err(LinkError::Invalid(_))));
    }

    #[test]
    fn a_message_may_not_carry_a_token() {
        let mut message = LinkMessage::message(6, envelope(6));
        message.token = Some(TOKEN.to_string());
        assert!(matches!(message.validate(), Err(LinkError::Invalid(_))));
    }

    #[test]
    fn a_bye_closes_the_session() {
        let mut session = Session::new(TOKEN);
        session
            .accept(&LinkMessage::handshake(TOKEN, 1))
            .expect("handshake");
        assert_eq!(session.accept(&LinkMessage::bye()), Ok(Disposition::Bye));
        assert_eq!(session.state(), SessionState::Closed);
    }

    #[test]
    fn the_outbound_queue_is_bounded() {
        let mut queue = OutboundQueue::new(2);
        assert!(queue.is_empty());
        queue.push(LinkMessage::hello(1)).expect("first");
        queue.push(LinkMessage::hello(2)).expect("second");
        assert_eq!(queue.push(LinkMessage::hello(3)), Err(LinkError::Full));
        assert_eq!(queue.len(), 2);
        assert!(queue.pop().is_some());
        assert!(queue.pop().is_some());
        assert!(queue.pop().is_none());
    }

    #[test]
    fn token_comparison_does_not_short_circuit() {
        assert!(constant_time_eq(b"token", b"token"));
        assert!(!constant_time_eq(b"token", b"tokeN"));
        assert!(!constant_time_eq(b"token", b"token-longer"));
        assert!(!constant_time_eq(b"", b"token"));
        assert!(constant_time_eq(b"", b""));
        assert!(verify_token(TOKEN, TOKEN));
        assert!(!verify_token(TOKEN, "other"));
    }

    #[test]
    fn io_failures_map_to_link_errors() {
        let would_block = io::Error::from(io::ErrorKind::WouldBlock);
        assert_eq!(LinkError::from(would_block), LinkError::Timeout);
        let timed_out = io::Error::from(io::ErrorKind::TimedOut);
        assert_eq!(LinkError::from(timed_out), LinkError::Timeout);
        let closed = io::Error::from(io::ErrorKind::UnexpectedEof);
        assert_eq!(LinkError::from(closed), LinkError::Closed);
        let other = io::Error::new(io::ErrorKind::BrokenPipe, "pipe broke");
        assert!(matches!(
            LinkError::from(other),
            LinkError::Io {
                kind: io::ErrorKind::BrokenPipe,
                ..
            }
        ));
    }

    #[test]
    fn a_truncated_frame_is_reported_as_closed() {
        let mut reader = CountingReader::new(vec![0, 0, 0, 8, 1, 2], 64);
        assert_eq!(read_frame(&mut reader), Err(LinkError::Closed));
    }

    #[test]
    fn timeouts_reach_an_open_stream() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("addr");
        let client = std::net::TcpStream::connect(address).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        let timeouts = Timeouts {
            read: Duration::from_millis(250),
            write: Duration::from_millis(500),
            request: Duration::from_secs(2),
        };
        timeouts.apply(&client).expect("apply");
        assert_eq!(
            client.read_timeout().expect("read timeout"),
            Some(Duration::from_millis(250))
        );
        assert_eq!(
            client.write_timeout().expect("write timeout"),
            Some(Duration::from_millis(500))
        );
        drop(server);
    }

    #[test]
    fn a_message_survives_a_frame_round_trip() {
        let message = LinkMessage::message(11, envelope(11));
        let mut writer = CountingWriter::default();
        write_message(&mut writer, &message).expect("write");
        let decoded = read_message(&mut Cursor::new(writer.written)).expect("read");
        assert_eq!(decoded, message);
    }
}
