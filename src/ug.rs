use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

#[derive(Clone, Default)]
pub struct Dev {
	pub id: u32,
	pub tgt: String,
	pub vp: String,
	pub name: String,
	pub serial: String,
	pub port: String,
	pub ifs: String,
	pub hash: String,
	pub raw: String,
}

pub enum Ev {
	Chg,
	New(Dev),
	Err(String),
}

pub fn run(a: &[&str]) -> Result<String, String> {
	let o = Command::new("usbguard").args(a).output().map_err(|e| e.to_string())?;
	let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
	if o.status.success() {
		Ok(String::from_utf8_lossy(&o.stdout).into())
	} else if e.is_empty() {
		Err(format!("usbguard {} failed", a[0]))
	} else {
		Err(e)
	}
}

fn toks(s: &str) -> Vec<String> {
	let (mut v, mut it) = (vec![], s.chars().peekable());
	while let Some(&c) = it.peek() {
		let mut t = String::new();
		if c.is_whitespace() {
			it.next();
			continue;
		} else if c == '"' {
			it.next();
			while let Some(c) = it.next() {
				match c {
					'\\' => t.extend(it.next()),
					'"' => break,
					_ => t.push(c),
				}
			}
		} else if c == '{' {
			it.next();
			while let Some(c) = it.next().filter(|&c| c != '}') {
				t.push(c);
			}
			t = t.trim().into();
		} else {
			while let Some(c) = it.next_if(|c| !c.is_whitespace()) {
				t.push(c);
			}
		}
		v.push(t);
	}
	v
}

pub fn rule(r: &str) -> Dev {
	let t = toks(r);
	let mut d = Dev { tgt: t.first().cloned().unwrap_or_default(), raw: r.into(), ..Default::default() };
	let mut i = 1;
	while i + 1 < t.len() {
		let v = t[i + 1].clone();
		match t[i].as_str() {
			"id" => d.vp = v,
			"name" => d.name = v,
			"serial" => d.serial = v,
			"via-port" => d.port = v,
			"with-interface" => d.ifs = v,
			"hash" => d.hash = v,
			_ => {}
		}
		i += 2;
	}
	d
}

fn rows(a: &[&str]) -> Result<Vec<(u32, String)>, String> {
	Ok(run(a)?.lines().filter_map(|l| l.split_once(": ")).filter_map(|(i, r)| Some((i.trim().parse().ok()?, r.to_string()))).collect())
}

pub fn devs() -> Result<Vec<Dev>, String> {
	Ok(rows(&["list-devices"])?.into_iter().map(|(i, r)| Dev { id: i, ..rule(&r) }).collect())
}

pub fn rules() -> Result<Vec<Dev>, String> {
	Ok(rows(&["list-rules"])?.into_iter().map(|(i, r)| Dev { id: i, ..rule(&r) }).collect())
}

impl Dev {
	pub fn hub(&self) -> bool {
		self.ifs == "09:00:00"
	}

	pub fn hits(&self, d: &Dev) -> bool {
		// self = rule
		let eq = |r: &str, v: &str| r.is_empty() || r == v || r.strip_suffix('*').is_some_and(|p| v.starts_with(p));
		if self.hash.is_empty() { !self.vp.is_empty() && eq(&self.vp, &d.vp) && eq(&self.serial, &d.serial) && eq(&self.name, &d.name) } else { self.hash == d.hash }
	}
}

pub fn act(id: u32, tgt: &str, perm: bool) -> Result<String, String> {
	let (c, i) = (format!("{tgt}-device"), id.to_string());
	if perm { run(&[&c, "-p", &i]) } else { run(&[&c, &i]) }
}

pub fn param(k: &str) -> String {
	run(&["get-parameter", k]).map(|s| s.trim().to_string()).unwrap_or_default()
}

pub fn watch(ping: impl Fn() + Send + 'static) -> Receiver<Ev> {
	let (tx, rx) = channel();
	std::thread::spawn(move || {
		loop {
			let Ok(mut c) = Command::new("usbguard").arg("watch").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() else { return };
			let (mut pres, mut ins, mut id) = (false, false, 0);
			let mut ln = BufReader::new(c.stdout.take().unwrap()).lines();
			while let Some(Ok(l)) = ln.next() {
				if let Some(h) = l.strip_prefix("[device] ") {
					(pres, ins) = (h.starts_with("PresenceChanged"), false);
					id = h.rsplit("id=").next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
					let _ = tx.send(Ev::Chg);
				} else if l.trim() == "event=Insert" {
					ins = true;
				} else if let Some(r) = l.trim().strip_prefix("device_rule=").filter(|_| pres && ins) {
					let d = Dev { id, ..rule(r) };
					if d.tgt == "block" {
						let _ = tx.send(Ev::New(d));
					}
				} else {
					continue;
				}
				ping();
			}
			let mut e = String::new();
			c.stderr.take().map(|mut s| s.read_to_string(&mut e));
			let _ = c.wait();
			if tx.send(Ev::Err(e.trim().into())).is_err() {
				return;
			}
			ping();
			std::thread::sleep(std::time::Duration::from_secs(3)); // daemon down/no perms, retry
		}
	});
	rx
}
