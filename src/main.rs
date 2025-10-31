mod chess;
mod evaluation;
pub mod search;
mod uci;

pub fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(m) = info.payload().downcast_ref::<&str>() {
            m
        } else if let Some(m) = info.payload().downcast_ref::<String>() {
            m.as_str()
        } else {
            "unknown"
        };

        let location = if let Some(l) = info.location() {
            format!("{}:{}:{}", l.file(), l.line(), l.column())
        } else {
            "unknown".to_string()
        };

        send!("info string panic {msg} {location}");

        if let Ok(bt) = std::env::var("RUST_BACKTRACE") {
            if bt == "1" || bt == "full" {
                let bt = std::backtrace::Backtrace::force_capture().to_string();
                for line in bt.lines() {
                    send!("info string {line}");
                }
            }
        }
    }));

    let mut uci = uci::Uci::new();
    uci.uci_loop();
}
