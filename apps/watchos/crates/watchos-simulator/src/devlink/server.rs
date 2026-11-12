//! Simulator-side runtime for the development link.
//!
//! The runtime owns the socket side of the link and nothing else. It never
//! touches the watch application state itself: a customization request is
//! handed to the watch thread, which applies it through the existing draft,
//! apply and persistence rules and answers with the outcome.
//!
//! The socket binds to the loopback interface only. One client is served at a
//! time, so the bounded outbound queue always belongs to that client.
//!
//! A connection to this development link says nothing about Charlie's assistant
//! backend. The link carries watch customization only; it never invents
//! assistant availability, sensor readings or notifications.

use std::collections::VecDeque;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use watchos_core::{Event, Settings, State};
use watchos_platform::Storage;
use watchos_protocol::{Customization, Envelope, Message};

use super::{
    FrameReader, LinkCode, LinkError, LinkMessage, OutboundQueue, Session, Timeouts, write_message,
};

/// How many outbound messages may wait for a slow client.
pub const OUTBOUND_CAPACITY: usize = 32;

/// How many customization requests may wait for the watch thread.
pub const INBOX_CAPACITY: usize = 32;

/// How long the watch thread may take to answer a request.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Shortest session token the runtime accepts.
pub const MIN_TOKEN_LENGTH: usize = 16;

/// Tokens that are long enough but still worthless.
const WEAK_TOKENS: [&str; 8] = [
    "changeme",
    "change-me",
    "password",
    "secret",
    "token",
    "watchos",
    "charlie",
    "default",
];

/// Fragments that make a token guessable wherever they appear.
const WEAK_FRAGMENTS: [&str; 4] = ["changeme", "change-me", "password", "secret"];

/// The committed state the link reports and synchronises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Revision of the committed customization.
    pub revision: u64,
    /// The committed customization.
    pub customization: Customization,
}

/// A customization request waiting for the watch thread.
pub struct PendingApply {
    /// Correlation id of the request.
    pub request_id: u64,
    /// The revision the request was based on.
    pub base_revision: Option<u64>,
    /// The requested customization.
    pub customization: Customization,
    reply: Sender<ApplyOutcome>,
}

/// What happened to a customization request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// The change was applied and written to the settings file.
    Applied {
        /// Revision after the change.
        revision: u64,
        /// The committed customization.
        customization: Customization,
    },
    /// The request was based on an older revision and was refused.
    Conflict {
        /// The current revision.
        revision: u64,
        /// The authoritative customization.
        customization: Customization,
    },
    /// The request was refused.
    Rejected {
        /// Why it was refused.
        code: LinkCode,
    },
    /// The change could not be written to the settings file.
    StorageFailed {
        /// The unchanged revision.
        revision: u64,
        /// The unchanged customization.
        customization: Customization,
    },
}

struct Shared {
    outbound: Mutex<OutboundQueue>,
    outbound_signal: Condvar,
    inbox: Mutex<VecDeque<PendingApply>>,
    snapshot: Mutex<Snapshot>,
    next_push_id: AtomicU64,
    shutdown: AtomicBool,
}

impl Shared {
    fn new() -> Self {
        Self {
            outbound: Mutex::new(OutboundQueue::new(OUTBOUND_CAPACITY)),
            outbound_signal: Condvar::new(),
            inbox: Mutex::new(VecDeque::new()),
            snapshot: Mutex::new(Snapshot {
                revision: 1,
                customization: Customization::default(),
            }),
            next_push_id: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The development link as the simulator sees it.
///
/// The handle is safe to share: the watch thread uses it to publish committed
/// changes and to answer requests, while the socket work happens on its own
/// threads.
pub struct DevLink {
    shared: Arc<Shared>,
    listener: Mutex<Option<JoinHandle<()>>>,
    port: u16,
}

impl DevLink {
    /// Bind the loopback listener and start serving clients.
    ///
    /// The socket is bound to `127.0.0.1` only. A port of zero asks the
    /// operating system for a free port, which the caller can read back with
    /// [`DevLink::port`].
    pub fn start(port: u16, token: impl Into<String>) -> std::io::Result<Self> {
        let address = SocketAddr::from(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        let listener = TcpListener::bind(address)?;
        let bound = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let shared = Arc::new(Shared::new());
        let thread_shared = shared.clone();
        let token = token.into();
        let listener_thread = thread::Builder::new()
            .name("dev-link".into())
            .spawn(move || accept_loop(listener, token, thread_shared))?;
        Ok(Self {
            shared,
            listener: Mutex::new(Some(listener_thread)),
            port: bound,
        })
    }

    /// The port the listener is bound to.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The committed state the link currently reports.
    pub fn snapshot(&self) -> Snapshot {
        *lock(&self.shared.snapshot)
    }

    /// Record the committed customization without telling any client.
    ///
    /// The revision only moves when the customization actually changed, so a
    /// cancelled draft leaves the revision alone.
    pub fn commit(&self, customization: &Customization) -> u64 {
        self.commit_inner(customization).0
    }

    /// Record a committed change and tell the connected client about it.
    pub fn publish(&self, customization: &Customization) -> u64 {
        let (revision, changed) = self.commit_inner(customization);
        if changed {
            let push_id = self.shared.next_push_id.fetch_add(1, Ordering::SeqCst);
            let envelope = Envelope::new(push_id, Message::Customization(*customization));
            let message = LinkMessage::message(push_id, envelope).with_revision(revision);
            if let Err(error) = self.push_outbound(message) {
                eprintln!(
                    "dev link: could not publish revision {revision}: {error}; the client will re-sync"
                );
            }
        }
        revision
    }

    /// Set the committed state the link starts from.
    pub fn seed(&self, customization: &Customization) {
        let mut snapshot = lock(&self.shared.snapshot);
        snapshot.customization = *customization;
    }

    /// Apply every customization request the socket thread handed over.
    ///
    /// This runs on the watch thread: it owns the application state and the
    /// settings file. It returns how many requests were answered.
    pub fn pump(&self, state: &mut State, storage: &dyn Storage) -> usize {
        let mut handled = 0;
        while let Some(pending) = self.take_pending() {
            let outcome = apply_remote_customization(
                state,
                &pending.customization,
                pending.base_revision,
                self.snapshot().revision,
                storage,
            );
            let outcome = match outcome {
                ApplyOutcome::Applied { customization, .. } => ApplyOutcome::Applied {
                    revision: self.commit(&customization),
                    customization,
                },
                other => other,
            };
            let _ = pending.reply.send(outcome);
            handled += 1;
        }
        handled
    }

    /// Stop the listener and close the client.
    pub fn shutdown(&self) {
        self.shared.shutdown.store(true, Ordering::SeqCst);
        self.shared.outbound_signal.notify_all();
        if let Some(handle) = lock(&self.listener).take() {
            let _ = handle.join();
        }
    }

    fn commit_inner(&self, customization: &Customization) -> (u64, bool) {
        let mut snapshot = lock(&self.shared.snapshot);
        if snapshot.customization == *customization {
            return (snapshot.revision, false);
        }
        snapshot.revision += 1;
        snapshot.customization = *customization;
        (snapshot.revision, true)
    }

    fn push_outbound(&self, message: LinkMessage) -> Result<(), LinkError> {
        let result = {
            let mut queue = lock(&self.shared.outbound);
            queue.push(message)
        };
        self.shared.outbound_signal.notify_all();
        result
    }

    fn take_pending(&self) -> Option<PendingApply> {
        lock(&self.shared.inbox).pop_front()
    }
}

impl Drop for DevLink {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::SeqCst);
        self.shared.outbound_signal.notify_all();
    }
}

/// The customization the link synchronises.
///
/// Quiet mode and brightness are not part of the protocol customization, so
/// they stay local to the watch.
pub fn customization_of(settings: &Settings) -> Customization {
    Customization {
        appearance: settings.appearance,
        use_24h: settings.use_24h,
        reduce_motion: settings.reduce_motion,
    }
}

/// Apply a customization the link received.
///
/// The change goes through the same draft, apply and persistence rules the
/// watch uses for its own edits, and nothing is reported as applied until the
/// settings file was written.
pub fn apply_remote_customization(
    state: &mut State,
    customization: &Customization,
    base_revision: Option<u64>,
    current_revision: u64,
    storage: &dyn Storage,
) -> ApplyOutcome {
    // An open draft belongs to the person using the watch. A remote change is
    // refused rather than silently overwriting it.
    if state.editing() {
        return ApplyOutcome::Rejected {
            code: LinkCode::DraftActive,
        };
    }
    let Some(base) = base_revision else {
        return ApplyOutcome::Rejected {
            code: LinkCode::RevisionRequired,
        };
    };
    if base != current_revision {
        return ApplyOutcome::Conflict {
            revision: current_revision,
            customization: customization_of(&state.settings),
        };
    }
    let previous = state.settings;
    let mut updated = previous;
    updated.appearance = customization.appearance;
    updated.use_24h = customization.use_24h;
    updated.reduce_motion = customization.reduce_motion;
    if !updated.is_valid() {
        return ApplyOutcome::Rejected {
            code: LinkCode::Invalid,
        };
    }
    if updated == previous {
        return ApplyOutcome::Applied {
            revision: current_revision,
            customization: customization_of(&previous),
        };
    }
    state.dispatch(Event::BeginCustomization);
    state.settings = updated;
    state.dispatch(Event::ApplyCustomization);
    let persisted = serde_json::to_vec(&state.settings)
        .ok()
        .and_then(|bytes| storage.save(&bytes).ok())
        .is_some();
    if !persisted {
        // The settings file was not touched. Put the committed state back so
        // the watch and the link agree on what is stored.
        state.settings = previous;
        state.draft = None;
        state.preview_return = None;
        return ApplyOutcome::StorageFailed {
            revision: current_revision,
            customization: customization_of(&previous),
        };
    }
    ApplyOutcome::Applied {
        revision: current_revision,
        customization: customization_of(&state.settings),
    }
}

/// Read the session token from the environment and check that it is usable.
///
/// The token never reaches a command line, a log line or the watch face. The
/// failure messages describe what is wrong with the value without repeating
/// the value itself.
pub fn session_token_from_environment() -> Result<String, String> {
    let token = std::env::var("CHARLIE_DEV_LINK_TOKEN").map_err(|_| {
        "CHARLIE_DEV_LINK_TOKEN must be set when --dev-link-port is used".to_string()
    })?;
    check_token(&token)
}

/// Check that a session token is usable.
///
/// The token never reaches a command line, a log line or the watch face. The
/// failure messages describe what is wrong with the value without repeating
/// the value itself.
pub fn check_token(token: &str) -> Result<String, String> {
    if token.len() < MIN_TOKEN_LENGTH {
        return Err(format!(
            "CHARLIE_DEV_LINK_TOKEN must be at least {MIN_TOKEN_LENGTH} characters"
        ));
    }
    if token.trim() != token {
        return Err("CHARLIE_DEV_LINK_TOKEN must not start or end with whitespace".to_string());
    }
    if WEAK_TOKENS
        .iter()
        .any(|weak| token.eq_ignore_ascii_case(weak))
    {
        return Err("CHARLIE_DEV_LINK_TOKEN is too easy to guess".to_string());
    }
    let lowered = token.to_ascii_lowercase();
    if WEAK_FRAGMENTS
        .iter()
        .any(|fragment| lowered.contains(fragment))
    {
        return Err("CHARLIE_DEV_LINK_TOKEN is too easy to guess".to_string());
    }
    Ok(token.to_string())
}

fn accept_loop(listener: TcpListener, token: String, shared: Arc<Shared>) {
    while !shared.shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                // One client at a time: the outbound queue belongs to it.
                serve_client(stream, token.clone(), shared.clone());
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => break,
        }
    }
}

fn serve_client(stream: TcpStream, token: String, shared: Arc<Shared>) {
    let _ = stream.set_nonblocking(false);
    let timeouts = Timeouts {
        read: Duration::from_secs(1),
        write: Duration::from_secs(10),
        request: REQUEST_TIMEOUT,
    };
    let _ = timeouts.apply(&stream);
    let Ok(writer_half) = stream.try_clone() else {
        return;
    };
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let writer_shared = shared.clone();
    let writer = thread::Builder::new()
        .name("dev-link-writer".into())
        .spawn(move || writer_loop(writer_half, writer_shared, writer_stop));
    let mut session = Session::new(token);
    let mut reader = FrameReader::new();
    let mut socket = stream;
    loop {
        if shared.shutdown.load(Ordering::SeqCst) {
            break;
        }
        let payload = match reader.read_frame(&mut socket) {
            Ok(Some(payload)) => payload,
            // The link is idle: give the client a chance to speak later.
            Ok(None) => continue,
            Err(error) => {
                eprintln!("dev link: closing the connection: {error}");
                break;
            }
        };
        let message = match LinkMessage::from_json(&payload) {
            Ok(message) => message,
            Err(error) => {
                eprintln!("dev link: closing the connection: {error}");
                break;
            }
        };
        match handle_message(&mut session, &message, &shared) {
            Flow::Continue => {}
            Flow::Stop => break,
        }
    }
    stop.store(true, Ordering::SeqCst);
    shared.outbound_signal.notify_all();
    if let Ok(handle) = writer {
        let _ = handle.join();
    }
}

enum Flow {
    Continue,
    Stop,
}

fn handle_message(session: &mut Session, message: &LinkMessage, shared: &Arc<Shared>) -> Flow {
    match session.accept(message) {
        Ok(super::Disposition::Authenticated) => {
            eprintln!("dev link: client authenticated");
            let request_id = message.request_id.unwrap_or_default();
            let reply = state_reply(session, shared, request_id);
            let _ = push_outbound(shared, reply);
            Flow::Continue
        }
        Ok(super::Disposition::Hello { request_id }) => {
            let reply = state_reply(session, shared, request_id);
            let _ = push_outbound(shared, reply);
            Flow::Continue
        }
        Ok(super::Disposition::Apply {
            request_id,
            envelope,
        }) => {
            let reply = apply_reply(session, shared, message, request_id, envelope);
            session.complete(request_id, reply.clone());
            let _ = push_outbound(shared, reply);
            Flow::Continue
        }
        Ok(super::Disposition::Repeat { reply, .. }) => {
            // A repeat is answered with the reply that was already given. The
            // change is not applied a second time.
            let _ = push_outbound(shared, reply);
            Flow::Continue
        }
        Ok(super::Disposition::Bye) => Flow::Stop,
        Ok(super::Disposition::Ignored) => Flow::Continue,
        Err(error) => {
            eprintln!("dev link: closing the connection: {error}");
            Flow::Stop
        }
    }
}

fn state_reply(session: &mut Session, shared: &Arc<Shared>, reply_to: u64) -> LinkMessage {
    let snapshot = *lock(&shared.snapshot);
    let reply_id = session.next_request_id();
    let envelope = Envelope::new(reply_id, Message::Customization(snapshot.customization));
    LinkMessage::ack(reply_id, reply_to, Some(envelope)).with_revision(snapshot.revision)
}

fn apply_reply(
    session: &mut Session,
    shared: &Arc<Shared>,
    message: &LinkMessage,
    request_id: u64,
    envelope: Envelope,
) -> LinkMessage {
    let reply_id = session.next_request_id();
    let Message::Customization(customization) = envelope.message.clone() else {
        return LinkMessage::error(Some(reply_id), Some(request_id), LinkCode::Invalid);
    };
    let (reply, outcome) = channel();
    let pending = PendingApply {
        request_id,
        base_revision: message.revision,
        customization,
        reply,
    };
    {
        let mut inbox = lock(&shared.inbox);
        if inbox.len() >= INBOX_CAPACITY {
            return LinkMessage::error(Some(reply_id), Some(request_id), LinkCode::Internal);
        }
        inbox.push_back(pending);
    }
    let outcome = match outcome.recv_timeout(REQUEST_TIMEOUT) {
        Ok(outcome) => outcome,
        Err(_) => ApplyOutcome::Rejected {
            code: LinkCode::Timeout,
        },
    };
    match outcome {
        ApplyOutcome::Applied {
            revision,
            customization,
        } => {
            let envelope = Envelope::new(reply_id, Message::Customization(customization));
            LinkMessage::ack(reply_id, request_id, Some(envelope)).with_revision(revision)
        }
        ApplyOutcome::Conflict {
            revision,
            customization,
        } => {
            let envelope = Envelope::new(reply_id, Message::Customization(customization));
            LinkMessage::error(Some(reply_id), Some(request_id), LinkCode::Conflict)
                .with_revision(revision)
                .with_envelope(envelope)
        }
        ApplyOutcome::Rejected { code } => {
            LinkMessage::error(Some(reply_id), Some(request_id), code)
        }
        ApplyOutcome::StorageFailed {
            revision,
            customization,
        } => {
            let envelope = Envelope::new(reply_id, Message::Customization(customization));
            LinkMessage::error(Some(reply_id), Some(request_id), LinkCode::StorageFailed)
                .with_revision(revision)
                .with_envelope(envelope)
        }
    }
}

fn push_outbound(shared: &Arc<Shared>, message: LinkMessage) -> Result<(), LinkError> {
    let result = {
        let mut queue = lock(&shared.outbound);
        queue.push(message)
    };
    shared.outbound_signal.notify_all();
    result
}

fn writer_loop(mut stream: TcpStream, shared: Arc<Shared>, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let message = {
            let mut queue = lock(&shared.outbound);
            if queue.is_empty() {
                let (guard, _) = shared
                    .outbound_signal
                    .wait_timeout(queue, Duration::from_millis(200))
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                queue = guard;
            }
            queue.pop()
        };
        let Some(message) = message else {
            continue;
        };
        if let Err(error) = write_message(&mut stream, &message) {
            eprintln!("dev link: could not write to the client: {error}");
            stop.store(true, Ordering::SeqCst);
            break;
        }
        let _ = stream.flush();
    }
}

/// Read one message from a client, used by the tests.
#[cfg(test)]
fn read_client_message(stream: &mut TcpStream) -> Option<LinkMessage> {
    super::read_message(stream).ok()
}

/// True when the link was asked to stop.
#[cfg(test)]
fn is_shutting_down(link: &DevLink) -> bool {
    link.shared.shutdown.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::super::LinkKind;
    use super::*;
    use std::cell::{Cell, RefCell};
    use watchos_core::Settings;
    use watchos_platform::PlatformError;
    use watchos_protocol::{Appearance, Face};

    const TOKEN: &str = "unit-test-session-value-long";

    #[derive(Default)]
    struct MemoryStorage {
        bytes: RefCell<Vec<u8>>,
        fail: Cell<bool>,
    }

    impl Storage for MemoryStorage {
        fn load(&self, bytes: &mut [u8]) -> Result<usize, PlatformError> {
            let stored = self.bytes.borrow();
            if stored.len() > bytes.len() {
                return Err(PlatformError::BufferTooSmall);
            }
            bytes[..stored.len()].copy_from_slice(&stored);
            Ok(stored.len())
        }

        fn save(&self, bytes: &[u8]) -> Result<(), PlatformError> {
            if self.fail.get() {
                return Err(PlatformError::Io);
            }
            self.bytes.replace(bytes.to_vec());
            Ok(())
        }
    }

    fn state_with(face: Face) -> State {
        let mut state = State::default();
        state.settings.appearance.face = face;
        state
    }

    fn customization(face: Face) -> Customization {
        Customization {
            appearance: Appearance {
                face,
                ..Appearance::default()
            },
            use_24h: true,
            reduce_motion: false,
        }
    }

    fn start_link() -> DevLink {
        DevLink::start(0, TOKEN).expect("start the development link")
    }

    fn connect(link: &DevLink) -> TcpStream {
        let stream = TcpStream::connect(("127.0.0.1", link.port())).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        stream
    }

    /// Hand the request to the watch thread and wait until it was answered.
    fn pump_until_answered(link: &DevLink, state: &mut State, storage: &dyn Storage) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if link.pump(state, storage) > 0 {
                return true;
            }
            thread::sleep(Duration::from_millis(2));
        }
        false
    }

    fn handshake(stream: &mut TcpStream) -> LinkMessage {
        write_message(stream, &LinkMessage::handshake(TOKEN, 1)).expect("handshake");
        read_client_message(stream).expect("handshake reply")
    }

    #[test]
    fn a_handshake_returns_the_authoritative_state_and_revision() {
        let link = start_link();
        let state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        let reply = handshake(&mut stream);
        assert_eq!(reply.kind, LinkKind::Ack);
        assert_eq!(reply.reply_to, Some(1));
        assert_eq!(reply.revision, Some(1));
        let envelope = reply.envelope.expect("authoritative state");
        assert_eq!(
            envelope.message,
            Message::Customization(customization_of(&state.settings))
        );
    }

    #[test]
    fn an_unauthorized_client_is_rejected() {
        let link = start_link();
        let mut stream = connect(&link);
        write_message(
            &mut stream,
            &LinkMessage::handshake("wrong-token-entirely", 1),
        )
        .expect("write");
        // The server closes the connection without answering.
        assert!(read_client_message(&mut stream).is_none());
    }

    #[test]
    fn a_remote_apply_changes_and_persists_the_settings() {
        let link = start_link();
        let storage = MemoryStorage::default();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        let hello = handshake(&mut stream);
        let base = hello.revision.expect("revision");
        let request = LinkMessage::message(
            2,
            Envelope::new(2, Message::Customization(customization(Face::Classic))),
        )
        .with_revision(base);
        write_message(&mut stream, &request).expect("write");
        assert!(pump_until_answered(&link, &mut state, &storage));
        let reply = read_client_message(&mut stream).expect("apply reply");
        assert_eq!(reply.kind, LinkKind::Ack);
        assert_eq!(reply.revision, Some(base + 1));
        assert_eq!(state.settings.appearance.face, Face::Classic);
        let stored: Settings =
            serde_json::from_slice(&storage.bytes.borrow()).expect("stored settings");
        assert_eq!(stored.appearance.face, Face::Classic);
    }

    #[test]
    fn a_duplicate_request_is_not_applied_twice() {
        let link = start_link();
        let storage = MemoryStorage::default();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        let hello = handshake(&mut stream);
        let base = hello.revision.expect("revision");
        let request = LinkMessage::message(
            2,
            Envelope::new(2, Message::Customization(customization(Face::Dashboard))),
        )
        .with_revision(base);
        write_message(&mut stream, &request).expect("write");
        assert!(pump_until_answered(&link, &mut state, &storage));
        let first = read_client_message(&mut stream).expect("first reply");
        // The same request id arrives again.
        write_message(&mut stream, &request).expect("write");
        let second = read_client_message(&mut stream).expect("second reply");
        assert_eq!(first, second);
        assert_eq!(first.revision, Some(base + 1));
        assert_eq!(link.snapshot().revision, base + 1);
    }

    #[test]
    fn a_stale_revision_is_refused_with_the_authoritative_state() {
        let link = start_link();
        let storage = MemoryStorage::default();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        let hello = handshake(&mut stream);
        let base = hello.revision.expect("revision");
        let stale = LinkMessage::message(
            2,
            Envelope::new(2, Message::Customization(customization(Face::Classic))),
        )
        .with_revision(base - 1);
        write_message(&mut stream, &stale).expect("write");
        assert!(pump_until_answered(&link, &mut state, &storage));
        let reply = read_client_message(&mut stream).expect("conflict reply");
        assert_eq!(reply.kind, LinkKind::Error);
        assert_eq!(reply.code, Some(LinkCode::Conflict));
        assert_eq!(reply.revision, Some(base));
        assert!(
            reply.envelope.is_some(),
            "the conflict carries the authoritative state"
        );
        assert_eq!(state.settings.appearance.face, Face::Hybrid);
    }

    #[test]
    fn a_request_without_a_revision_is_refused() {
        let link = start_link();
        let storage = MemoryStorage::default();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        handshake(&mut stream);
        let request = LinkMessage::message(
            2,
            Envelope::new(2, Message::Customization(customization(Face::Classic))),
        );
        write_message(&mut stream, &request).expect("write");
        assert!(pump_until_answered(&link, &mut state, &storage));
        let reply = read_client_message(&mut stream).expect("reply");
        assert_eq!(reply.code, Some(LinkCode::RevisionRequired));
    }

    #[test]
    fn a_failed_save_is_not_acknowledged() {
        let link = start_link();
        let storage = MemoryStorage {
            bytes: RefCell::new(Vec::new()),
            fail: Cell::new(true),
        };
        let mut state = state_with(Face::Hybrid);
        let before = state.settings;
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        let hello = handshake(&mut stream);
        let base = hello.revision.expect("revision");
        let request = LinkMessage::message(
            2,
            Envelope::new(2, Message::Customization(customization(Face::Classic))),
        )
        .with_revision(base);
        write_message(&mut stream, &request).expect("write");
        assert!(pump_until_answered(&link, &mut state, &storage));
        let reply = read_client_message(&mut stream).expect("reply");
        assert_eq!(reply.kind, LinkKind::Error);
        assert_eq!(reply.code, Some(LinkCode::StorageFailed));
        assert_eq!(state.settings, before, "the committed state is restored");
        assert!(
            storage.bytes.borrow().is_empty(),
            "the settings file is untouched"
        );
    }

    #[test]
    fn an_open_draft_is_not_overwritten() {
        let link = start_link();
        let storage = MemoryStorage::default();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        let hello = handshake(&mut stream);
        let base = hello.revision.expect("revision");
        state.dispatch(Event::BeginCustomization);
        state.settings.appearance.face = Face::Minimal;
        let request = LinkMessage::message(
            2,
            Envelope::new(2, Message::Customization(customization(Face::Classic))),
        )
        .with_revision(base);
        write_message(&mut stream, &request).expect("write");
        assert!(pump_until_answered(&link, &mut state, &storage));
        let reply = read_client_message(&mut stream).expect("reply");
        assert_eq!(reply.code, Some(LinkCode::DraftActive));
        assert_eq!(
            state.settings.appearance.face,
            Face::Minimal,
            "the draft survives"
        );
    }

    #[test]
    fn a_local_commit_is_published_to_the_client() {
        let link = start_link();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        let mut stream = connect(&link);
        handshake(&mut stream);
        state.settings.appearance.face = Face::Minimal;
        let revision = link.publish(&customization_of(&state.settings));
        assert_eq!(revision, 2);
        let push = read_client_message(&mut stream).expect("push");
        assert_eq!(push.kind, LinkKind::Message);
        assert_eq!(push.revision, Some(2));
        let envelope = push.envelope.expect("published state");
        assert_eq!(
            envelope.message,
            Message::Customization(customization_of(&state.settings))
        );
    }

    #[test]
    fn a_cancelled_draft_leaves_the_revision_alone() {
        let link = start_link();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        state.dispatch(Event::BeginCustomization);
        state.settings.appearance.face = Face::Minimal;
        state.dispatch(Event::CancelCustomization);
        let revision = link.publish(&customization_of(&state.settings));
        assert_eq!(revision, 1, "nothing was committed");
        assert_eq!(state.settings.appearance.face, Face::Hybrid);
    }

    #[test]
    fn a_reconnect_reports_the_committed_state() {
        let link = start_link();
        let storage = MemoryStorage::default();
        let mut state = state_with(Face::Hybrid);
        link.seed(&customization_of(&state.settings));
        {
            let mut stream = connect(&link);
            let hello = handshake(&mut stream);
            let base = hello.revision.expect("revision");
            let request = LinkMessage::message(
                2,
                Envelope::new(2, Message::Customization(customization(Face::Classic))),
            )
            .with_revision(base);
            write_message(&mut stream, &request).expect("write");
            assert!(pump_until_answered(&link, &mut state, &storage));
            read_client_message(&mut stream).expect("apply reply");
        }
        let mut stream = connect(&link);
        let hello = handshake(&mut stream);
        assert_eq!(hello.revision, Some(2));
        let envelope = hello.envelope.expect("authoritative state");
        assert_eq!(
            envelope.message,
            Message::Customization(customization_of(&state.settings))
        );
    }

    #[test]
    fn an_oversized_frame_is_rejected_without_an_answer() {
        let link = start_link();
        let mut stream = connect(&link);
        let mut prefix = ((super::super::MAX_PAYLOAD_BYTES + 1) as u32)
            .to_be_bytes()
            .to_vec();
        prefix.extend_from_slice(b"ignored");
        stream.write_all(&prefix).expect("write");
        assert!(read_client_message(&mut stream).is_none());
    }

    #[test]
    fn a_malformed_payload_is_rejected_without_an_answer() {
        let link = start_link();
        let mut stream = connect(&link);
        let payload = b"{not json";
        let mut framed = (payload.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(payload);
        stream.write_all(&framed).expect("write");
        assert!(read_client_message(&mut stream).is_none());
    }

    #[test]
    fn the_link_stops_when_it_is_shut_down() {
        let link = start_link();
        assert!(!is_shutting_down(&link));
        link.shutdown();
        assert!(is_shutting_down(&link));
    }

    #[test]
    fn a_weak_token_is_refused() {
        assert!(check_token("short").is_err());
        assert!(check_token("changeme-changeme-changeme").is_err());
        assert!(check_token("  padded-token-value  ").is_err());
        assert!(check_token("").is_err());
    }

    #[test]
    fn a_strong_token_is_accepted() {
        assert_eq!(check_token(TOKEN).ok(), Some(TOKEN.to_string()));
    }

    #[test]
    fn the_link_never_binds_outside_the_loopback_interface() {
        let link = start_link();
        let stream = connect(&link);
        let local = stream.local_addr().expect("local address");
        assert!(local.ip().is_loopback());
        let peer = stream.peer_addr().expect("peer address");
        assert!(
            peer.ip().is_loopback(),
            "the listener is bound to the loopback interface"
        );
    }
}
