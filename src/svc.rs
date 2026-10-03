use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Init {
	name: &'static str,
	on: bool,
	file: PathBuf,
	src: &'static str,
	mode: u32,
	up: &'static [&'static [&'static str]],
	down: &'static [&'static [&'static str]],
}

fn pick() -> Option<Init> {
	let cfg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
	let run = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_default();
	let has = |p: &str| Path::new(p).exists();
	[
		Init { name: "systemd", on: has("/run/systemd/system"), file: cfg.join("systemd/user/guardiod.service"), src: include_str!("../init/systemd"), mode: 0o644, up: &[&["systemctl", "--user", "daemon-reload"], &["systemctl", "--user", "enable", "--now", "guardiod"]], down: &[&["systemctl", "--user", "disable", "--now", "guardiod"]] },
		Init { name: "openrc", on: has("/run/openrc"), file: cfg.join("rc/init.d/guardiod"), src: include_str!("../init/openrc"), mode: 0o755, up: &[&["rc-update", "--user", "add", "guardiod", "default"], &["rc-service", "--user", "guardiod", "restart"]], down: &[&["rc-service", "--user", "guardiod", "stop"], &["rc-update", "--user", "del", "guardiod", "default"]] },
		Init { name: "dinit", on: run.join("dinitctl").exists(), file: cfg.join("dinit.d/guardiod"), src: include_str!("../init/dinit"), mode: 0o644, up: &[&["dinitctl", "enable", "guardiod"]], down: &[&["dinitctl", "disable", "guardiod"], &["dinitctl", "stop", "guardiod"]] },
		Init { name: "runit", on: has("/run/runit"), file: cfg.join("service/guardiod/run"), src: include_str!("../init/runit"), mode: 0o755, up: &[], down: &[] }, // user runsvdir on ~/.config/service picks it up
		Init { name: "xdg autostart", on: true, file: cfg.join("autostart/guardiod.desktop"), src: include_str!("../init/autostart"), mode: 0o644, up: &[], down: &[] },
	]
	.into_iter()
	.find(|i| i.on)
}

fn cmds(c: &[&[&str]]) -> String {
	let mut e = String::new();
	for a in c {
		if !Command::new(a[0]).args(&a[1..]).status().is_ok_and(|s| s.success()) {
			e += &format!("\nfailed: {}", a.join(" "));
		}
	}
	e
}

pub fn install() -> Result<String, String> {
	let i = pick().ok_or("no $HOME")?;
	let b = std::env::current_exe().map_err(|e| e.to_string())?;
	std::fs::create_dir_all(i.file.parent().unwrap()).map_err(|e| e.to_string())?;
	std::fs::write(&i.file, i.src.replace("@BIN@", &b.to_string_lossy())).map_err(|e| e.to_string())?;
	std::fs::set_permissions(&i.file, std::fs::Permissions::from_mode(i.mode)).map_err(|e| e.to_string())?;
	Ok(format!("{}: wrote {}{}", i.name, i.file.display(), cmds(i.up)))
}

pub fn uninstall() -> Result<String, String> {
	let i = pick().ok_or("no $HOME")?;
	let e = cmds(i.down);
	if i.name == "runit" {
		let _ = std::fs::remove_dir_all(i.file.parent().unwrap());
	}
	std::fs::remove_file(&i.file).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e.to_string()) })?;
	Ok(format!("{}: removed {}{e}", i.name, i.file.display()))
}
