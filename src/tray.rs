use crate::ug::{self, Dev, Ev};
use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use std::process::Command;

struct Tray {
	bad: usize,
}

impl ksni::Tray for Tray {
	fn id(&self) -> String {
		"guardio".into()
	}

	fn title(&self) -> String {
		"guardio".into()
	}

	fn icon_name(&self) -> String {
		if self.bad > 0 { "security-low" } else { "security-high" }.into()
	}

	fn tool_tip(&self) -> ksni::ToolTip {
		ksni::ToolTip { title: "guardio".into(), description: format!("{} not allowed", self.bad), ..Default::default() }
	}

	fn activate(&mut self, _: i32, _: i32) {
		gui();
	}

	fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
		vec![StandardItem { label: "open".into(), activate: Box::new(|_| gui()), ..Default::default() }.into()]
	}
}

fn gui() {
	if !ug::up(false) {
		std::env::current_exe().map(|b| Command::new(b).spawn()).ok();
	}
}

fn bad() -> usize {
	ug::devs().map(|d| d.iter().filter(|d| d.tgt != "allow").count()).unwrap_or(0)
}

fn ask(d: Dev) {
	let b = format!("{}\n{}  @ {}", if d.name.is_empty() { "unknown device" } else { &d.name }, d.vp, d.port);
	let Ok(o) = Command::new("notify-send").args(["-a", "guardio", "-i", "security-low", "-u", "critical", "-A", "default=open", "-A", "allow=Allow", "-A", "always=Always", "-A", "reject=Reject", "new usb device", &b]).output() else { return };
	let r = match String::from_utf8_lossy(&o.stdout).trim() {
		"allow" => ug::act(d.id, "allow", false),
		"always" => ug::act(d.id, "allow", true),
		"reject" => ug::act(d.id, "reject", false),
		"default" => return gui(),
		_ => return,
	};
	if let Err(e) = r {
		let _ = Command::new("notify-send").args(["-a", "guardio", "-u", "critical", "guardio", &e]).status();
	}
}

pub fn run() -> eframe::Result {
	let h = Tray { bad: bad() }.assume_sni_available(true).spawn().ok(); // waits for waybar etc
	let rx = ug::watch(|| {});
	while let Ok(e) = rx.recv() {
		if let Ev::New(d) = e {
			std::thread::spawn(move || ask(d));
		}
		let n = bad();
		h.iter().for_each(|h| _ = h.update(|t| t.bad = n));
	}
	Ok(())
}
