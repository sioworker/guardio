use crate::ug::Dev;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

fn ids() -> &'static HashMap<String, String> {
	static M: OnceLock<HashMap<String, String>> = OnceLock::new();
	M.get_or_init(|| {
		let s = std::fs::read("/usr/share/hwdata/usb.ids").map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
		let (mut m, mut v) = (HashMap::new(), String::new());
		for l in s.lines().take_while(|l| !l.starts_with("C ")) { // vendors end where classes start
			if let Some(p) = l.strip_prefix('\t').filter(|p| !p.starts_with('\t')) {
				if let Some((id, n)) = p.split_once("  ") {
					m.insert(format!("{v}:{id}"), n.to_string());
				}
			} else if let Some((id, n)) = l.split_once("  ").filter(|(id, _)| id.len() == 4 && !l.starts_with(['#', '\t'])) {
				v = id.into();
				m.insert(v.clone(), n.to_string());
			}
		}
		m
	})
}

pub fn vendor(d: &Dev) -> &'static str {
	d.vp.get(..4).and_then(|v| ids().get(v)).map_or("", |s| s)
}

pub fn title(d: &Dev, l: &HashMap<String, String>) -> String {
	l.get(&key(d)).cloned().or_else(|| Some(d.name.clone()).filter(|n| !n.is_empty())).or_else(|| ids().get(&d.vp).cloned()).unwrap_or_else(|| "unknown device".into())
}

pub fn kinds(d: &Dev) -> Vec<(&'static str, &'static str)> {
	let mut v = vec![];
	for i in d.ifs.split_whitespace() {
		let p: Vec<&str> = i.split(':').collect();
		let k = match (p[0], p.get(2).copied().unwrap_or("")) {
			("03", "01") => ("⌨", "keyboard"),
			("03", "02") => ("🖱", "mouse"),
			("03", _) => ("🎮", "hid"),
			("01", _) => ("🎧", "audio"),
			("02" | "0a", _) => ("🔌", "network"),
			("06", _) => ("📷", "camera"),
			("0e", _) => ("📷", "webcam"),
			("07", _) => ("🖨", "printer"),
			("08", _) => ("💾", "storage"),
			("09", _) => ("🔀", "hub"),
			("0b", _) => ("💳", "smartcard"),
			("e0", _) => ("📶", "wireless"),
			("ff", _) => ("❓", "vendor"),
			_ => continue,
		};
		if !v.contains(&k) {
			v.push(k);
		}
	}
	v
}

pub fn sus(d: &Dev) -> Option<&'static str> { // classic badusb combos
	let k: Vec<&str> = kinds(d).iter().map(|k| k.1).collect();
	if !k.contains(&"keyboard") {
		None
	} else if k.contains(&"storage") {
		Some("keyboard + storage, could be BadUSB")
	} else if k.contains(&"network") {
		Some("keyboard + network, could be BadUSB")
	} else {
		None
	}
}

pub fn key(d: &Dev) -> String {
	if d.hash.is_empty() { format!("{}/{}", d.vp, d.serial) } else { d.hash.clone() }
}

fn path() -> Option<PathBuf> {
	Some(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?.join("guardio/names"))
}

pub fn labels() -> HashMap<String, String> {
	path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default().lines().filter_map(|l| l.split_once('\t')).map(|(k, v)| (k.into(), v.into())).collect()
}

pub fn label(k: &str, v: &str) -> Result<(), String> {
	let (p, mut m) = (path().ok_or("no $HOME")?, labels());
	if v.trim().is_empty() {
		m.remove(k);
	} else {
		m.insert(k.into(), v.trim().replace(['\t', '\n'], " "));
	}
	let mut s: Vec<String> = m.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
	s.sort();
	std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
	std::fs::write(p, s.concat()).map_err(|e| e.to_string())
}
