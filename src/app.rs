use crate::ug::{self, Dev, Ev};
use eframe::egui::{self, Color32, RichText, ViewportCommand};
use std::sync::mpsc::Receiver;

const HINT: &str = "no IPC perms, run: doas usbguard add-user $USER -d modify,list,listen -p modify,list -P modify,list,listen";

enum Act {
	Dev(u32, &'static str, bool),
	Revoke(u32, Option<u32>),
}

pub struct App {
	devs: Vec<Dev>,
	rules: Vec<Dev>,
	pol: [String; 2],
	asks: Vec<Dev>,
	rx: Receiver<Ev>,
	tab: u8,
	hubs: bool,
	add: String,
	msg: String,
}

impl App {
	pub fn new(ctx: &egui::Context) -> Self {
		let c = ctx.clone();
		let mut a = App { devs: vec![], rules: vec![], pol: Default::default(), asks: vec![], rx: ug::watch(move || c.request_repaint()), tab: 0, hubs: false, add: String::new(), msg: String::new() };
		a.load();
		a
	}

	fn load(&mut self) {
		self.msg.clear();
		match ug::devs() {
			Ok(d) => self.devs = d,
			Err(e) => self.msg = if e.contains("not permitted") { HINT.into() } else { e },
		}
		self.rules = ug::rules().unwrap_or_default();
		self.pol = ["ImplicitPolicyTarget", "InsertedDevicePolicy"].map(ug::param);
		let d = &self.devs;
		self.asks.retain(|a| d.iter().any(|x| x.id == a.id && x.tgt == "block"));
	}

	fn res(&mut self, r: Result<String, String>) {
		match r {
			Ok(_) => self.load(),
			Err(e) => self.msg = e,
		}
	}

	fn always(&self, d: &Dev) -> bool {
		self.rules.iter().any(|r| r.tgt == "allow" && r.hits(d))
	}

	fn lists(&self) -> [Vec<(&Dev, Option<&Dev>)>; 3] {
		// (shown, live dev)
		let h = |d: &&Dev| self.hubs || !d.hub();
		let a = self.rules.iter().filter(|r| r.tgt == "allow").filter(h).map(|r| (r, self.devs.iter().find(|d| r.hits(d)))).collect();
		let b = self.devs.iter().filter(|d| d.tgt == "allow" && !self.always(d)).filter(h).map(|d| (d, Some(d))).collect();
		let c = self.devs.iter().filter(|d| d.tgt != "allow").filter(h).map(|d| (d, Some(d))).collect();
		[a, b, c]
	}

	fn cards(&self, ui: &mut egui::Ui, l: &[(&Dev, Option<&Dev>)]) -> Option<Act> {
		let mut act = None;
		if l.is_empty() {
			ui.add_space(30.);
			ui.vertical_centered(|ui| ui.weak("nothing here"));
		}
		ui.spacing_mut().item_spacing = egui::vec2(12., 12.);
		let n = ((ui.available_width() + 12.) / 262.).max(1.) as usize; // min card 250 + gap
		let w = (ui.available_width() + 12.) / n as f32 - 12. - 31.; // - gap - margin/stroke
		for row in l.chunks(n) {
			ui.horizontal(|ui| {
				for &(d, live) in row {
					let c = col(live.map_or("", |x| &x.tgt));
					egui::Frame::new().corner_radius(14.).inner_margin(14.).fill(ui.visuals().faint_bg_color).stroke(egui::Stroke::new(1.5, c.gamma_multiply(0.6))).show(ui, |ui| {
						ui.set_width(w);
						ui.vertical(|ui| {
							ui.horizontal(|ui| {
								let (r, _) = ui.allocate_exact_size(egui::vec2(10., 10.), egui::Sense::hover());
								ui.painter().circle_filled(r.center(), 4.5, c);
								ui.add(egui::Label::new(RichText::new(if d.name.is_empty() { "unknown device" } else { &d.name }).strong().size(15.)).truncate()).on_hover_text(&d.raw);
							});
							ui.label(RichText::new(format!("{}  {}", d.vp, live.map_or("not plugged in", |x| &x.port))).monospace().weak().small());
							ui.add_space(6.);
							ui.horizontal(|ui| match (self.tab, live) {
								(0, x) => {
									if ui.button("revoke").on_hover_text("drop the rule, block if plugged in").clicked() {
										act = Some(Act::Revoke(d.id, x.map(|x| x.id)));
									}
								}
								(1, _) => {
									if ui.button("always").clicked() {
										act = Some(Act::Dev(d.id, "allow", true));
									}
									if ui.button("block").clicked() {
										act = Some(Act::Dev(d.id, "block", false));
									}
								}
								_ => {
									if ui.button("allow").clicked() {
										act = Some(Act::Dev(d.id, "allow", false));
									}
									if ui.button("always").clicked() {
										act = Some(Act::Dev(d.id, "allow", true));
									}
									if d.tgt == "block" && ui.button("reject").clicked() {
										act = Some(Act::Dev(d.id, "reject", false));
									}
								}
							});
						});
					});
				}
			});
		}
		act
	}

	fn rules_ui(&mut self, ui: &mut egui::Ui) {
		ui.horizontal(|ui| {
			let mut i = 0;
			while i < 2 {
				let (k, o): (&str, &[&str]) = if i == 0 { ("ImplicitPolicyTarget", &["allow", "block", "reject"]) } else { ("InsertedDevicePolicy", &["apply-policy", "block", "reject"]) };
				let old = self.pol[i].clone();
				egui::ComboBox::from_label(k).selected_text(&self.pol[i]).show_ui(ui, |ui| o.iter().for_each(|&v| _ = ui.selectable_value(&mut self.pol[i], v.into(), v)));
				if self.pol[i] != old {
					let r = ug::run(&["set-parameter", k, &self.pol[i]]);
					self.res(r);
				}
				i += 1;
			}
		});
		ui.separator();
		ui.horizontal(|ui| {
			let r = ui.add(egui::TextEdit::singleline(&mut self.add).hint_text("allow id 046d:c52b").desired_width(ui.available_width() - 60.).font(egui::TextStyle::Monospace));
			if (ui.button("add").clicked() || r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) && !self.add.trim().is_empty() {
				let r = ug::run(&["append-rule", self.add.trim()]);
				if r.is_ok() {
					self.add.clear();
				}
				self.res(r);
			}
		});
		let mut rm = None;
		egui::Grid::new("rules").striped(true).spacing([12., 6.]).show(ui, |ui| {
			for r in &self.rules {
				ui.label(r.id.to_string());
				ui.label(RichText::new(&r.raw).monospace().color(col(&r.tgt)));
				if ui.small_button("x").clicked() {
					rm = Some(r.id);
				}
				ui.end_row();
			}
		});
		if let Some(i) = rm {
			let r = ug::run(&["remove-rule", &i.to_string()]);
			self.res(r);
		}
	}

	fn asks_ui(&mut self, ctx: &egui::Context) {
		let mut done = vec![];
		for d in &self.asks {
			egui::Window::new("new usb device").id(egui::Id::new(("ask", d.id))).collapsible(false).resizable(false).show(ctx, |ui| {
				ui.heading(if d.name.is_empty() { "unknown device" } else { &d.name });
				ui.monospace(format!("{}  @ {}", d.vp, d.port));
				ui.label(format!("ifs: {}", d.ifs));
				ui.horizontal(|ui| {
					for (l, t, p) in [("allow", "allow", false), ("always", "allow", true), ("reject", "reject", false), ("ignore", "", false)] {
						if ui.button(l).clicked() {
							done.push((d.id, t, p));
						}
					}
				});
			});
		}
		for (i, t, p) in done {
			self.asks.retain(|a| a.id != i);
			if !t.is_empty() {
				let r = ug::act(i, t, p);
				self.res(r);
			}
		}
	}
}

fn col(t: &str) -> Color32 {
	match t {
		"allow" => Color32::from_rgb(80, 180, 90),
		"block" => Color32::from_rgb(220, 160, 40),
		"reject" => Color32::from_rgb(220, 70, 60),
		_ => Color32::GRAY,
	}
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
		let mut dirty = false;
		while let Ok(e) = self.rx.try_recv() {
			match e {
				Ev::Chg => dirty = true,
				Ev::New(d) => {
					ui.ctx().send_viewport_cmd(ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
					self.asks.retain(|a| a.id != d.id);
					self.asks.push(d);
				}
				Ev::Err(e) if self.msg.is_empty() => self.msg = if e.contains("not permitted") { HINT.into() } else { format!("watch: {e}") },
				_ => {}
			}
		}
		if dirty {
			self.load();
		}
		let n = self.lists().map(|l| l.len());
		egui::Panel::top("top").show(ui, |ui| {
			ui.add_space(6.);
			ui.horizontal(|ui| {
				for (i, t) in ["always allowed", "allowed", "not allowed"].iter().enumerate() {
					ui.selectable_value(&mut self.tab, i as u8, RichText::new(format!("{t}  {}", n[i])).size(15.));
				}
				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					ui.selectable_value(&mut self.tab, 3, "rules");
					if ui.button("refresh").clicked() {
						self.load();
					}
					ui.checkbox(&mut self.hubs, "hubs");
				});
			});
			ui.add_space(4.);
		});
		if !self.msg.is_empty() {
			egui::Panel::bottom("msg").show(ui, |ui| {
				ui.horizontal(|ui| {
					ui.colored_label(Color32::from_rgb(220, 70, 60), &self.msg);
					if ui.small_button("x").clicked() {
						self.msg.clear();
					}
				});
			});
		}
		egui::CentralPanel::default_margins().show(ui, |ui| {
			egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
				if self.tab == 3 {
					return self.rules_ui(ui);
				}
				let l = self.lists();
				let a = self.cards(ui, &l[self.tab as usize]);
				drop(l);
				let r = match a {
					Some(Act::Dev(i, t, p)) => ug::act(i, t, p),
					Some(Act::Revoke(r, d)) => ug::run(&["remove-rule", &r.to_string()]).and_then(|s| d.map_or(Ok(s), |d| ug::act(d, "block", false))),
					None => return,
				};
				self.res(r);
			});
		});
		let c = ui.ctx().clone();
		self.asks_ui(&c);
	}
}
