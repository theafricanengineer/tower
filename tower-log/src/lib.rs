//! Tower middleware that logs errors returned by a wrapped service.
//!
//! This is useful if those errors would otherwise be ignored or
//! transformed into another error type that might provide less
//! information, such as by `tower-buffer`.

extern crate futures;
extern crate tower;
extern crate log;

use futures::{Async, Future, Poll};
use tower::{Service, NewService};

use std::error::Error;
use std::fmt;

/// Wrap a `Service` or `NewService` with `LogErrors` middleware.
///
/// Unlike using `LogErrors::new`, this macro will configure the returned
/// middleware to log messages with the module path and file/line location of
/// the _call site_, as using the `log!` macro in that file would.
///
/// For example:
/// ```rust,ignore
/// mod my_module {
///     fn my_function<S>(service: S) -> LogErrors<S>
///     where S: Service,
///           S::Error: Error,
///     {
///         let timeout = Timeout::new(
///             service, timer, Duration::from_secs(1)
///         );
///         log_errors!(timeout)
///     }
/// }
/// ```
/// will produce log messages like
/// ```notrust,ignore
/// ERROR 2018-03-05T18:36:36Z: my_crate::my_module: Future::poll: operation timed out after Duration { secs: 1, nanos: 0 }
/// ```
/// while
/// ```rust,ignore
/// mod my_module {
///     fn my_function<S>(service: S) -> LogErrors<S>
///     where S: Service,
///           S::Error: Error,
///     {
///         let timeout = Timeout::new(
///             service, timer, Duration::from_secs(1)
///         );
///         LogErrors::new(timeout)
///     }
/// }
/// ```
/// will produce log messages like
/// ```notrust,ignore
/// ERROR 2018-03-05T18:36:36Z: tower_log_errors: Future::poll: operation timed out after Duration { secs: 1, nanos: 0 }
/// ```
#[macro_export]
macro_rules! log_errors {
    ($inner:expr) => {
        log_errors!(level: ::log::Level::Error, $inner)
    };
    (target: $target:expr, $($rest:tt)+) => {
        log_errors!($($rest),+).target($target)
    };
    (level: $level:expr, $inner:expr) => {
        $crate::LogErrors::new($inner)
            .in_module(module_path!())
            .at_location(file!(), line!())
            .at_level($level)
    };
}

/// Wrap a `Service` or `NewService` with `LogResponses` middleware.
///
/// Unlike using `LogResponses::new`, this macro will configure the returned
/// middleware to log messages with the module path and file/line location of
/// the _call site_, as using the `log!` macro in that file would.
#[macro_export]
macro_rules! log_responses {
    ($inner:expr) => {
        log_responses!(level: ::log::Level::Debug, $inner)
    };
    (target: $target:expr, $($rest:tt)+) => {
        log_responses!($($rest),+).target($target)
    };
    (level: $level:expr, $inner:expr) => {
        $crate::LogResponses::new($inner)
            .in_module(module_path!())
            .at_location(file!(), line!())
            .at_level($level)
    };
}
/// Logs error responses.
#[derive(Clone, Debug)]
pub struct LogErrors<T> {
    inner: T,
    level: log::Level,
    target: Option<&'static str>,
    module_path: Option<&'static str>,
    file: Option<&'static str>,
    line: Option<u32>,
}

/// Logs successful responses.
#[derive(Clone, Debug)]
pub struct LogResponses<T> {
    inner: T,
    level: log::Level,
    not_ready: bool,
    target: Option<&'static str>,
    module_path: Option<&'static str>,
    file: Option<&'static str>,
    line: Option<u32>,
}


// ===== impl LogErrors =====

impl<T> LogErrors<T> {

    /// Construct a new `LogErrors` middleware that wraps the given `Service`
    /// or `NewService`.
    ///
    /// The log level will default to `Level::Error` but may be changed with
    /// the [`at_level`] function.
    ///
    /// # Note
    ///
    /// Unless the module path of the `LogErrors` middleware is changed with
    /// the [`in_module`]  methods, log records produced by the returned
    /// middleware will always have the module path `tower-log-errors`. It may
    /// be preferred to use the [`log_errors!`] macro instead, as it will
    /// produce log records which appear to have been produced at the call site.
    ///
    /// [`at_level`]: struct.LogErrors.html#method.at_level
    /// [`in_module`]: struct.LogErrors.html#method.in_module
    /// [`log_errors!`]: macro.log_errors.html
    pub fn new(inner: T) -> Self {
        LogErrors {
            inner,
            level: log::Level::Error,
            target: None,
            module_path: None,
            file: None,
            line: None,
        }
    }

    /// Set the log level of the produced log records.
    ///
    /// Log records will be logged at the `Error` level by default.
    pub fn at_level(mut self, level: log::Level) -> Self {
        self.level = level;
        self
    }

    /// Set the target of the produced log records.
    ///
    /// The target will default to the module path of the `LogErrors` middleware
    /// by default.
    pub fn with_target(mut self, target: &'static str) -> Self {
        self.target = Some(target);
        self
    }

    /// Set the module path of the produced log records to the given string.
    pub fn in_module(mut self, module_path: &'static str) -> Self {
        self.module_path = Some(module_path);
        self
    }

    /// Set the file and line number of the produced log records.
    pub fn at_location(mut self, file: &'static str, line: u32) -> Self {
        self.file = Some(file);
        self.line = Some(line);
        self
    }

    fn child<U>(&self, inner: U) -> LogErrors<U> {
        LogErrors {
            inner,
            level: self.level,
            target: self.target,
            module_path: self.module_path,
            file: self.file,
            line: self.line,
        }
    }

    fn log_line<E: Error>(&self, error: &E, context: &'static str) {
        log::Log::log(
            log::logger(),
            &log::RecordBuilder::new()
                .level(self.level)
                .file(self.file.or_else(|| Some(file!())))
                .line(self.line.or_else(|| Some(line!())))
                .target(
                    self.target
                        .or(self.module_path)
                        .unwrap_or_else(|| module_path!())
                )
                .module_path(
                    self.module_path
                        .or(self.target)
                        .or_else(|| Some(module_path!())))
                .args(format_args!("{}: {}", context, error))
                .build()
        )

    }

}

impl<T> Future for LogErrors<T>
where
    T: Future,
    T::Error: Error,
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll().map_err(|e| {
            self.log_line(&e, "Future::poll");
            e
        })
    }
}

impl<T> Service for LogErrors<T>
where
    T: Service,
    T::Error: Error,
{
    type Request = T::Request;
    type Response = T::Response;
    type Error = T::Error;
    type Future = LogErrors<T::Future>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.inner.poll_ready().map_err(|e| {
            self.log_line(&e, "Service::poll_ready");
            e
        })
    }

    fn call(&mut self, req: Self::Request) -> Self::Future {
        let inner = self.inner.call(req);
        self.child(inner)
    }
}

impl<T> NewService for LogErrors<T>
where
    T: NewService,
    T::Error: Error,
    T::InitError: Error,
{

    type Request = T::Request;
    type Response = T::Response;
    type Error = T::Error;
    type Service = T::Service;
    type InitError = T::InitError;
    type Future = LogErrors<T::Future>;

    fn new_service(&self) -> Self::Future {
        self.child(self.inner.new_service())
    }
}

// ===== impl LogResponses =====

impl<T> LogResponses<T> {

    /// Construct a new `LogResponses` middleware that wraps the given `Service`
    /// or `NewService`.
    ///
    /// The log level will default to `Level::Debug` but may be changed with
    /// the [`at_level`] function. `Async::NotReady` responses will not be
    /// logged by default, but may be enabled with the [`log_not_ready`]
    /// method.
    ///
    /// # Note
    ///
    /// Unless the module path of the `LogResponses` middleware is changed with
    /// the [`in_module`]  methods, log records produced by the returned
    /// middleware will always have the module path `tower-log`. It may
    /// be preferred to use the [`log_responses!`] macro instead, as it will
    /// produce log records which appear to have been produced at the call site.
    ///
    /// [`at_level`]: struct.LogResponses.html#method.at_level
    /// [`not_ready`]: struct.LogResponses.html#method.log_not_ready
    /// [`in_module`]: struct.LogResponses.html#method.in_module
    /// [`log_responses!`]: macro.log_responses.html
    pub fn new(inner: T) -> Self {
        LogResponses {
            inner,
            level: log::Level::Debug,
            not_ready: false,
            target: None,
            module_path: None,
            file: None,
            line: None,
        }
    }

    /// Set the log level of the produced log records.
    ///
    /// Log records will be logged at the `Debug` level by default.
    pub fn at_level(mut self, level: log::Level) -> Self {
        self.level = level;
        self
    }

    /// Set the target of the produced log records.
    ///
    /// The target will default to the module path of the `LogResponses`
    /// middleware by default.
    pub fn with_target(mut self, target: &'static str) -> Self {
        self.target = Some(target);
        self
    }

    /// Set the module path of the produced log records to the given string.
    pub fn in_module(mut self, module_path: &'static str) -> Self {
        self.module_path = Some(module_path);
        self
    }

    /// Set the file and line number of the produced log records.
    pub fn at_location(mut self, file: &'static str, line: u32) -> Self {
        self.file = Some(file);
        self.line = Some(line);
        self
    }

    /// Set whether or not this middleware should log `Async::NotReady`
    /// responses.
    pub fn log_not_ready(mut self, not_ready: bool) -> Self {
        self.not_ready = not_ready;
        self
    }

    fn child<U>(&self, inner: U) -> LogResponses<U> {
        LogResponses {
            inner,
            not_ready: self.not_ready,
            level: self.level,
            target: self.target,
            module_path: self.module_path,
            file: self.file,
            line: self.line,
        }
    }

    fn log_line<R: fmt::Debug>(&self, resp: &R, context: &'static str) {
        log::Log::log(
            log::logger(),
            &log::RecordBuilder::new()
                .level(self.level)
                .file(self.file.or_else(|| Some(file!())))
                .line(self.line.or_else(|| Some(line!())))
                .target(
                    self.target
                        .or(self.module_path)
                        .unwrap_or_else(|| module_path!())
                )
                .module_path(
                    self.module_path
                        .or(self.target)
                        .or_else(|| Some(module_path!())))
                .args(format_args!("{}: {:?}", context, resp))
                .build()
        )
    }

    fn log_poll<R: fmt::Debug>(&self,
                                poll: Async<R>,
                                context: &'static str)
                                -> Async<R>
    {
        match poll {
            ref not_ready @ Async::NotReady if self.not_ready => {
                self.log_line(not_ready, context);
                Async::NotReady
            },
            Async::Ready(rsp) => {
                self.log_line(&rsp, context);
                Async::Ready(rsp)
            },
            rsp => rsp,
        }
    }

}

impl<T> Future for LogResponses<T>
where
    T: Future,
    T::Item: fmt::Debug,
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let poll = self.inner.poll();
        poll.map(|poll| self.log_poll(poll, "Future::poll"))
    }
}

impl<T> Service for LogResponses<T>
where
    T: Service,
    T::Response: fmt::Debug,
{
    type Request = T::Request;
    type Response = T::Response;
    type Error = T::Error;
    type Future = LogResponses<T::Future>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        let poll = self.inner.poll_ready();
        poll.map(|poll| self.log_poll(poll, "Service::poll_ready"))
    }

    fn call(&mut self, req: Self::Request) -> Self::Future {
        let inner = self.inner.call(req);
        self.child(inner)
    }
}

impl<T> NewService for LogResponses<T>
where
    T: NewService,
    T::Service: fmt::Debug,
    T::Response: fmt::Debug,
{

    type Request = T::Request;
    type Response = T::Response;
    type Error = T::Error;
    type Service = T::Service;
    type InitError = T::InitError;
    type Future = LogResponses<T::Future>;

    fn new_service(&self) -> Self::Future {
        self.child(self.inner.new_service())
    }
}