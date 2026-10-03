mod app;
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
	let o = eframe::NativeOptions { viewport: eframe::egui::ViewportBuilder::default().with_inner_size([820., 480.]).with_app_id("guardio"), ..Default::default() };
	eframe::run_native("guardio", o, Box::new(|cc| Ok(Box::new(app::App::new(&cc.egui_ctx)))))
}
