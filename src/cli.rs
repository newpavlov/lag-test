use vulkano::swapchain::PresentMode;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    name = "cam-vis",
    about = "Simple camera visualization tool")]
pub(crate) struct Cli {
    #[structopt(long = "mode", short = "m",
        parse(try_from_str = "parse_mode"),
        default_value="fifo")]
    /// Vulkan present mode: immediate, mailbox, fifo or relaxed
    pub mode: PresentMode,
}

fn parse_mode(s: &str) -> Result<PresentMode, &'static str> {
    use self::PresentMode::*;

    Ok(match s {
        "immediate" => Immediate,
        "mailbox" => Mailbox,
        "fifo" => Fifo,
        "relaxed" => Relaxed,
        _ => Err("unknown present mode")?,
    })
}
