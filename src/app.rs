use crate::info;
use crate::ug::{self, Dev, Ev};
use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Sense, Stroke, StrokeKind, ViewportCommand};
use std::collections::HashMap;
use std::sync::mpsc::Receiver;

const HINT: &str = "no IPC perms, run: doas usbguard add-user $USER -d modify,list,listen -p modify,list -P modify,list,listen";
const BG: Color32 = Color32::from_rgb(0x16, 0x16, 0x1b);
const CARD: Color32 = Color32::from_rgb(0x20, 0x20, 0x27);
const LINE: Color32 = Color32::from_rgb(0x30, 0x30, 0x3a);
const TXT: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xea);
const DIM: Color32 = Color32::from_rgb(0x9a, 0x9c, 0xa8);
const ACC: Color32 = Color32::from_rgb(0x7a, 0xa2, 0xf7);

enum Act {
	Dev(u32, &'static str, bool),
	Revoke(u32, Option<u32>),
	Label(String, String),
}

pub struct App {
	devs: Vec<Dev>,
	rules: Vec<Dev>,
	pol: [String; 2],
	asks: Vec<Dev>,
	rx: Receiver<Ev>,
	tab: u8,
	hubs: bool,
	lbl: HashMap<String, String>,
	ren: Option<(String, String)>, // key, buf
	pick: (&'static str, u32),     // rules form: tgt, dev id
	add: String,
	msg: String,
}

fn theme(ctx: &egui::Context) {
	let mut v = egui::Visuals::dark();
	(v.panel_fill, v.window_fill, v.faint_bg_color, v.extreme_bg_color) = (BG, CARD, CARD, Color32::from_rgb(0x12, 0x12, 0x16));
	v.weak_text_color = Some(DIM);
	v.window_corner_radius = CornerRadius::same(14);
	v.window_stroke = Stroke::new(1., LINE);
	v.selection.bg_fill = ACC.gamma_multiply(0.35);
	v.selection.stroke = Stroke::new(1., ACC);
	v.widgets.noninteractive.fg_stroke.color = TXT;
	v.widgets.noninteractive.bg_stroke.color = LINE;
	for (w, f) in [(&mut v.widgets.inactive, 0x2a), (&mut v.widgets.hovered, 0x36), (&mut v.widgets.active, 0x40), (&mut v.widgets.open, 0x36)] {
		w.corner_radius = CornerRadius::same(8);
		(w.bg_fill, w.weak_bg_fill) = (Color32::from_rgb(f, f, f + 9), Color32::from_rgb(f, f, f + 9));
		w.bg_stroke = Stroke::NONE;
	}
	ctx.set_theme(egui::ThemePreference::Dark);
	ctx.set_visuals_of(egui::Theme::Dark, v);
	ctx.all_styles_mut(|s| {
		s.spacing.button_padding = egui::vec2(12., 5.);
		s.spacing.item_spacing = egui::vec2(8., 8.);
	});
}

fn btn(ui: &mut egui::Ui, t: &str, c: Color32) -> egui::Response {
	ui.add(egui::Button::new(RichText::new(t).color(c)).fill(c.gamma_multiply(0.16)))
}

fn chip(ui: &mut egui::Ui, t: &str, c: Color32) {
	egui::Frame::new().fill(c.gamma_multiply(0.18)).corner_radius(8).inner_margin(Margin::symmetric(7, 1)).show(ui, |ui| ui.label(RichText::new(t).small().color(c)));
}

fn tab(ui: &mut egui::Ui, t: &str, n: Option<usize>, hot: bool, on: bool) -> bool {
	let r = ui.horizontal(|ui| {
		ui.label(RichText::new(t).size(15.).color(if on { TXT } else { DIM }));
		if let Some(n) = n {
			chip(ui, &n.to_string(), if hot && n > 0 { col("reject") } else { DIM });
		}
	});
	let r = r.response.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
	if on || r.hovered() {
		ui.painter().hline(r.rect.x_range(), r.rect.bottom() + 7., Stroke::new(2., if on { ACC } else { LINE }));
	}
	ui.add_space(10.);
	r.clicked()
}

fn kinds(ui: &mut egui::Ui, d: &Dev) {
	let ks = info::kinds(d);
	if !ks.is_empty() {
		ui.label(RichText::new(ks.iter().map(|(e, n)| format!("{e} {n}")).collect::<Vec<_>>().join("   ")).small().color(TXT));
	}
	if let Some(w) = info::sus(d) {
		ui.label(RichText::new(format!("⚠ {w}")).color(col("reject")).small().strong());
	}
}

impl App {
	pub fn new(ctx: &egui::Context) -> Self {
		theme(ctx);
		let c = ctx.clone();
		let mut a = App { devs: vec![], rules: vec![], pol: Default::default(), asks: vec![], rx: ug::watch(move || c.request_repaint()), tab: 0, hubs: false, lbl: HashMap::new(), ren: None, pick: ("allow", 0), add: String::new(), msg: String::new() };
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
		self.lbl = info::labels();
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

	fn cards(&mut self, ui: &mut egui::Ui) -> Option<Act> {
		let l = &lists(&self.devs, &self.rules, self.hubs)[self.tab as usize];
		let mut act = None;
		if l.is_empty() {
			ui.add_space(60.);
			ui.vertical_centered(|ui| ui.label(RichText::new(["no saved devices", "nothing allowed just for now", "nothing blocked, all good"][self.tab as usize]).color(DIM)));
		}
		ui.spacing_mut().item_spacing = egui::vec2(12., 12.);
		let n = ((ui.available_width() + 12.) / 262.).max(1.) as usize; // min card 250 + gap
		let w = (ui.available_width() + 12.) / n as f32 - 12. - 32.; // - gap - margin/stroke
		for row in l.chunks(n) {
			ui.horizontal_top(|ui| {
				for &(d, live) in row {
					let c = col(live.map_or("", |x| &x.tgt));
					let fr = egui::Frame::new().corner_radius(14).inner_margin(15).fill(CARD).stroke(Stroke::new(1., LINE)).show(ui, |ui| {
						ui.set_width(w);
						ui.spacing_mut().item_spacing.y = 6.;
						ui.vertical(|ui| {
							ui.horizontal(|ui| {
								let (r, _) = ui.allocate_exact_size(egui::vec2(10., 10.), Sense::hover());
								ui.painter().circle_filled(r.center(), 4.5, c);
								let k = info::key(d);
								match &mut self.ren {
									Some((rk, b)) if *rk == k => {
										let r = ui.add(egui::TextEdit::singleline(b).id(egui::Id::new(("ren", &k))).desired_width(w - 20.).hint_text("name, empty = reset"));
										if r.lost_focus() {
											act = Some(if ui.input(|i| i.key_pressed(egui::Key::Escape)) { Act::Label(String::new(), String::new()) } else { Act::Label(k, b.clone()) }); // empty key = cancel
										}
									}
									_ => {
										let t = info::title(d, &self.lbl);
										if ui
											.add(egui::Label::new(RichText::new(&t).strong().size(15.).color(TXT)).truncate().sense(Sense::click()))
											.on_hover_text(format!("{}\ndouble-click to rename\n\n{}", d.name, d.raw))
											.double_clicked()
										{
											ui.memory_mut(|m| m.request_focus(egui::Id::new(("ren", &k))));
											self.ren = Some((k, self.lbl.get(&info::key(d)).cloned().unwrap_or(t)));
										}
									}
								}
							});
							if !info::vendor(d).is_empty() {
								ui.add(egui::Label::new(RichText::new(info::vendor(d)).small().color(DIM)).truncate());
							}
							kinds(ui, d);
							ui.label(RichText::new(format!("{}   {}", d.vp, live.map_or("not plugged in", |x| &x.port))).monospace().small().color(DIM));
							ui.add_space(4.);
							ui.horizontal(|ui| match (self.tab, live) {
								(0, x) => {
									if btn(ui, "revoke", col("reject")).on_hover_text("drop the rule, block if plugged in").clicked() {
										act = Some(Act::Revoke(d.id, x.map(|x| x.id)));
									}
								}
								(1, _) => {
									if btn(ui, "always", col("allow")).clicked() {
										act = Some(Act::Dev(d.id, "allow", true));
									}
									if btn(ui, "block", col("block")).clicked() {
										act = Some(Act::Dev(d.id, "block", false));
									}
								}
								_ => {
									if btn(ui, "allow", col("allow")).clicked() {
										act = Some(Act::Dev(d.id, "allow", false));
									}
									if btn(ui, "always", col("allow")).clicked() {
										act = Some(Act::Dev(d.id, "allow", true));
									}
									if d.tgt == "block" && btn(ui, "reject", col("reject")).clicked() {
										act = Some(Act::Dev(d.id, "reject", false));
									}
								}
							});
						});
					});
					if fr.response.contains_pointer() {
						ui.painter().rect_stroke(fr.response.rect, 14, Stroke::new(1.5, c.gamma_multiply(0.8)), StrokeKind::Inside);
					}
				}
			});
		}
		act
	}

	fn rules_ui(&mut self, ui: &mut egui::Ui) {
		let sec = |ui: &mut egui::Ui, t: &str| {
			ui.add_space(8.);
			ui.label(RichText::new(t).strong().size(15.).color(TXT));
		};
		sec(ui, "policy");
		let mut i = 0;
		while i < 2 {
			let (k, s, o): (&str, &str, &[(&str, &str)]) = if i == 0 {
				("ImplicitPolicyTarget", "devices without a rule are", &[("allow", "allowed"), ("block", "blocked"), ("reject", "rejected")])
			} else {
				("InsertedDevicePolicy", "newly plugged devices are", &[("apply-policy", "checked against the rules"), ("block", "blocked"), ("reject", "rejected")])
			};
			let old = self.pol[i].clone();
			ui.horizontal(|ui| {
				ui.label(RichText::new(s).color(DIM));
				let cur = o.iter().find(|x| x.0 == old).map_or(old.as_str(), |x| x.1);
				egui::ComboBox::from_id_salt(k).selected_text(cur).show_ui(ui, |ui| o.iter().for_each(|&(v, t)| _ = ui.selectable_value(&mut self.pol[i], v.into(), t)));
			});
			if self.pol[i] != old {
				let r = ug::run(&["set-parameter", k, &self.pol[i]]);
				self.res(r);
			}
			i += 1;
		}
		sec(ui, "add a rule");
		ui.horizontal(|ui| {
			egui::ComboBox::from_id_salt("tgt")
				.selected_text(format!("always {}", self.pick.0))
				.show_ui(ui, |ui| ["allow", "block", "reject"].iter().for_each(|&t| _ = ui.selectable_value(&mut self.pick.0, t, format!("always {t}"))));
			let cur = self.devs.iter().find(|d| d.id == self.pick.1).map_or("pick a plugged in device".into(), |d| info::title(d, &self.lbl));
			egui::ComboBox::from_id_salt("dev").width(260.).selected_text(cur).show_ui(ui, |ui| {
				for d in self.devs.iter().filter(|d| self.hubs || !d.hub()) {
					ui.selectable_value(&mut self.pick.1, d.id, format!("{}  ({})", info::title(d, &self.lbl), d.vp));
				}
			});
			if ui.add_enabled(self.devs.iter().any(|d| d.id == self.pick.1), egui::Button::new("add")).clicked() {
				let r = ug::act(self.pick.1, self.pick.0, true);
				self.res(r);
			}
		});
		egui::CollapsingHeader::new(RichText::new("raw rule").color(DIM)).show(ui, |ui| {
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
		});
		sec(ui, "rules");
		let mut rm = None;
		for r in self.rules.iter().filter(|r| self.hubs || !r.hub()) {
			egui::Frame::new().fill(CARD).corner_radius(10).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
				ui.set_width(ui.available_width());
				ui.horizontal(|ui| {
					chip(ui, &format!("always {}", r.tgt), col(&r.tgt));
					let what = if r.vp.is_empty() && r.hash.is_empty() { "anything matching".into() } else { info::title(r, &self.lbl) };
					ui.label(RichText::new(what).color(TXT)).on_hover_text(&r.raw);
					ui.label(RichText::new(&r.vp).monospace().small().color(DIM));
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
						if btn(ui, "remove", col("reject")).clicked() {
							rm = Some(r.id);
						}
					});
				});
			});
		}
		if let Some(i) = rm {
			let r = ug::run(&["remove-rule", &i.to_string()]);
			self.res(r);
		}
	}

	fn asks_ui(&mut self, ctx: &egui::Context) {
		let mut done = vec![];
		for d in &self.asks {
			egui::Window::new("new usb device").id(egui::Id::new(("ask", d.id))).collapsible(false).resizable(false).show(ctx, |ui| {
				ui.label(RichText::new(info::title(d, &self.lbl)).strong().size(17.).color(TXT));
				ui.label(RichText::new(info::vendor(d)).small().color(DIM));
				kinds(ui, d);
				ui.label(RichText::new(format!("{}   {}", d.vp, d.port)).monospace().small().color(DIM));
				ui.add_space(4.);
				ui.horizontal(|ui| {
					for (l, t, p, c) in [("allow", "allow", false, col("allow")), ("always", "allow", true, col("allow")), ("reject", "reject", false, col("reject")), ("ignore", "", false, DIM)] {
						if btn(ui, l, c).clicked() {
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

fn lists<'a>(devs: &'a [Dev], rules: &'a [Dev], hubs: bool) -> [Vec<(&'a Dev, Option<&'a Dev>)>; 3] { // (shown, live dev)
	let h = |d: &&Dev| hubs || !d.hub();
	let always = |d: &Dev| rules.iter().any(|r| r.tgt == "allow" && r.hits(d));
	let a = rules.iter().filter(|r| r.tgt == "allow").filter(h).map(|r| (r, devs.iter().find(|d| r.hits(d)))).collect();
	let b = devs.iter().filter(|d| d.tgt == "allow" && !always(d)).filter(h).map(|d| (d, Some(d))).collect();
	let c = devs.iter().filter(|d| d.tgt != "allow").filter(h).map(|d| (d, Some(d))).collect();
	[a, b, c]
}

fn col(t: &str) -> Color32 {
	match t {
		"allow" => Color32::from_rgb(0x7d, 0xc9, 0x8a),
		"block" => Color32::from_rgb(0xe8, 0xb8, 0x5a),
		"reject" => Color32::from_rgb(0xf0, 0x7a, 0x78),
		_ => Color32::from_rgb(0x6c, 0x6e, 0x7a),
	}
}

impl eframe::App for App {
	fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
		BG.to_normalized_gamma_f32() // def is see-through
	}

	fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
		let mut dirty = false;
		while let Ok(e) = self.rx.try_recv() {
			match e {
				Ev::Chg => dirty = true,
				Ev::New(d) if !ug::up(true) => { // guardio -d asks instead
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
		let n = lists(&self.devs, &self.rules, self.hubs).map(|l| l.len());
		egui::Panel::top("top").frame(egui::Frame::new().fill(BG).inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 10 })).show(ui, |ui| {
			ui.horizontal(|ui| {
				for (i, t) in ["always allowed", "allowed", "not allowed"].iter().enumerate() {
					if tab(ui, t, Some(n[i]), i == 2, self.tab == i as u8) {
						self.tab = i as u8;
					}
				}
				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					if tab(ui, "rules", None, false, self.tab == 3) {
						self.tab = 3;
					}
					if ui.button("⟳").on_hover_text("refresh").clicked() {
						self.load();
					}
					ui.checkbox(&mut self.hubs, RichText::new("hubs").color(DIM));
				});
			});
		});
		if !self.msg.is_empty() {
			egui::Panel::bottom("msg").frame(egui::Frame::new().fill(col("reject").gamma_multiply(0.15)).inner_margin(Margin::symmetric(16, 8))).show(ui, |ui| {
				ui.horizontal(|ui| {
					ui.label(RichText::new(&self.msg).color(col("reject")));
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
						if ui.small_button("✕").clicked() {
							self.msg.clear();
						}
					});
				});
			});
		}
		egui::CentralPanel::default_margins().frame(egui::Frame::new().fill(BG).inner_margin(Margin::symmetric(16, 12))).show(ui, |ui| {
			egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
				if self.tab == 3 {
					return self.rules_ui(ui);
				}
				let r = match self.cards(ui) {
					Some(Act::Dev(i, t, p)) => ug::act(i, t, p),
					Some(Act::Label(k, v)) => {
						self.ren = None;
						if k.is_empty() {
							return;
						}
						info::label(&k, &v).map(|_| String::new())
					}
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
