use super::{
    BacktraceStatus, Failure, Location,
    marker::{self, Payload},
};
use std::{
    backtrace::{Backtrace, BacktraceStatus as Status},
    sync::Once,
};
use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context};

pub(crate) fn install_hook() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned());
            let location = info.location().map(|l| Location {
                file: l.file().into(),
                line: l.line(),
                column: Some(l.column()),
            });
            let backtrace = Backtrace::force_capture();
            let status = match backtrace.status() {
                Status::Captured => BacktraceStatus::Captured,
                Status::Disabled => BacktraceStatus::Disabled,
                _ => BacktraceStatus::Unsupported,
            };
            let text = Some(backtrace.to_string());
            marker::emit(Payload::Failure {
                failure: Failure::captured_panic(message, location, text, status),
            });
            previous(info);
        }));
    });
}

/// Register through `LogPlugin::custom_layer`. Compose explicitly with other custom layers.
/// This observes dispatched errors only and never changes the application's filters.
pub fn tracing_error_layer(_app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer> {
    (std::env::var("WOODPECKER_TRACING_ERRORS").as_deref() == Ok("1"))
        .then(|| Box::new(ErrorLayer) as bevy::log::BoxedLayer)
}

pub(crate) fn layer_status() {
    let installed = tracing::dispatcher::get_default(|dispatcher| {
        dispatcher.downcast_ref::<ErrorLayer>().is_some()
    });
    marker::emit(Payload::LayerStatus { installed });
}

struct ErrorLayer;
impl<S: Subscriber> Layer<S> for ErrorLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        if *event.metadata().level() != Level::ERROR {
            return;
        }
        marker::emit(Payload::Failure {
            failure: from_event(event),
        });
    }
}

fn from_event(event: &Event<'_>) -> Failure {
    let mut fields = Fields::default();
    event.record(&mut fields);
    let metadata = event.metadata();
    let message = fields.message.unwrap_or_else(|| {
        if fields.values.is_empty() {
            metadata.name().into()
        } else {
            fields.values.join(", ")
        }
    });
    let location = metadata
        .file()
        .zip(metadata.line())
        .map(|(file, line)| Location {
            file: file.into(),
            line,
            column: None,
        });
    Failure::tracing_error(message, Some(metadata.target().into()), location)
}

#[derive(Default)]
struct Fields {
    message: Option<String>,
    values: Vec<String>,
}
impl Fields {
    fn add(&mut self, field: &Field, value: String, message: String) {
        if field.name() == "message" {
            self.message = Some(message);
        } else {
            self.values.push(format!("{}={value}", field.name()));
        }
    }
}
impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value = format!("{value:?}");
        self.add(field, value.clone(), value);
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.add(field, serde_json::to_string(value).unwrap(), value.into());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.add(field, value.to_string(), value.to_string());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.add(field, value.to_string(), value.to_string());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.add(field, value.to_string(), value.to_string());
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.add(field, value.to_string(), value.to_string());
    }
}
