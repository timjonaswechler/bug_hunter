use crate::server::protocol::Create;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    launch: crate::session::launch::Config,
    tick: crate::command::tick::Config,
    report: crate::report::Config,
}
pub(super) fn parse(source: &str) -> Result<Create, Box<dyn std::error::Error>> {
    let file: File = toml::from_str(source)?;
    if file.version != 1 {
        return Err("unsupported configuration version".into());
    }
    Ok(Create {
        launch: file.launch,
        tick: file.tick,
        report: file.report,
    })
}
