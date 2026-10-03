mod app;
mod svc;
mod tray;
mod ug;

fn desk() -> Option<()> { // TryExec hides it after cargo uninstall
	let b = std::env::current_exe().ok().filter(|b| !b.components().any(|c| c.as_os_str() == "target"))?; // skip cargo run
	let p = std::path::PathBuf::from(std::env::var_os("XDG_DATA_HOME").or_else(|| std::env::var_os("HOME").map(|h| format!("{}/.local/share", h.to_string_lossy()).into()))?).join("applications");
	let s = include_str!("../guardio.desktop").replace("@BIN@", &b.to_string_lossy());
	if std::fs::read_to_string(p.join("guardio.desktop")).ok() != Some(s.clone()) {
		std::fs::create_dir_all(&p).ok()?;
		std::fs::write(p.join("guardio.desktop"), s).ok()?;
	}
	Some(())
}

fn main() -> eframe::Result {
	desk();
	match std::env::args().nth(1).as_deref() {
		Some("-d") => return tray::run(),
		Some(a) => {
			let r = match a {
				"-i" => svc::install(),
				"-u" => svc::uninstall(),
				_ => Err("usage: guardio [-d daemon | -i install svc | -u uninstall svc]".into()),
			};
			match r {
				Ok(s) => println!("{s}"),
				Err(e) => {
					eprintln!("{e}");
					std::process::exit(1)
				}
			}
			return Ok(());
		}
		None => {}
	}
	let o = eframe::NativeOptions { viewport: eframe::egui::ViewportBuilder::default().with_inner_size([820., 480.]).with_app_id("guardio"), ..Default::default() };
	eframe::run_native("guardio", o, Box::new(|cc| Ok(Box::new(app::App::new(&cc.egui_ctx)))))
}
