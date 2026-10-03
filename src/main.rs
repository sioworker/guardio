mod app;
mod ug;

fn main() -> eframe::Result {
	let o = eframe::NativeOptions { viewport: eframe::egui::ViewportBuilder::default().with_inner_size([820., 480.]).with_app_id("guardio"), ..Default::default() };
	eframe::run_native("guardio", o, Box::new(|cc| Ok(Box::new(app::App::new(&cc.egui_ctx)))))
}
