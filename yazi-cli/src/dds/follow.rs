use std::os::fd::AsRawFd;

use anyhow::Result;
use hashbrown::HashSet;
use tokio::io::AsyncWriteExt;
use yazi_dds::{ClientReader, Payload, Stream, ember::EmberHi};
use yazi_macro::try_format;

use crate::dds::Dds;

const TIOCSTI: u64 = 0x5412;

impl Dds {
	/// Connect, inject initial @cwd state via TIOCSTI, then return.
	/// The caller spawns a follow-watch daemon for live updates.
	pub(crate) async fn follow() -> Result<()> {
		async fn connect(kinds: &HashSet<&str>) -> Result<ClientReader> {
			let (lines, mut writer) = Stream::connect().await?;
			let hi = Payload::new(EmberHi::borrowed(kinds.iter().copied()));
			writer.write_all(try_format!("{hi}\n")?.as_bytes()).await?;
			writer.flush().await?;
			Ok(lines)
		}

		let tty = std::fs::File::open("/dev/tty")?;
		let tty_fd = tty.as_raw_fd();
		unsafe { libc::signal(libc::SIGTTOU, libc::SIG_IGN) };

		let kinds = HashSet::from_iter(["@cwd", "@exec"]);
		let mut lines = connect(&kinds)
			.await
			.map_err(|_| anyhow::anyhow!("No running Yazi instance found. Start yazi first."))?;

		// Drain initial replayed state — inject @cwd, skip stale @exec
		while let Ok(Ok(Some(line))) =
			tokio::time::timeout(std::time::Duration::from_millis(200), lines.next_line()).await
		{
			if line.split(',').next() == Some("@cwd") {
				if let Some(url) = extract_field(&line, "url") {
					tiocsti_inject(tty_fd, &format!("cd {url}"));
				}
			}
		}

		Ok(())
	}

	/// Background daemon: subscribe to DDS events and inject commands via
	/// TIOCSTI for real-time CWD sync and remote exec.
	pub(crate) async fn follow_watch() -> Result<()> {
		async fn connect(kinds: &HashSet<&str>) -> Result<ClientReader> {
			let (lines, mut writer) = Stream::connect().await?;
			let hi = Payload::new(EmberHi::borrowed(kinds.iter().copied()));
			writer.write_all(try_format!("{hi}\n")?.as_bytes()).await?;
			writer.flush().await?;
			Ok(lines)
		}

		let tty = std::fs::File::open("/dev/tty")?;
		let tty_fd = tty.as_raw_fd();
		unsafe { libc::signal(libc::SIGTTOU, libc::SIG_IGN) };

		let kinds = HashSet::from_iter(["@cwd", "@exec"]);
		let mut lines = connect(&kinds)
			.await
			.map_err(|_| anyhow::anyhow!("No running Yazi instance found. Start yazi first."))?;

		// Skip initial replayed state (already handled by `follow`)
		while let Ok(Ok(Some(_))) =
			tokio::time::timeout(std::time::Duration::from_millis(200), lines.next_line()).await
		{}

		loop {
			match lines.next_line().await? {
				Some(line) => {
					if let Some(cmd) = parse_cmd(&line) {
						tiocsti_inject(tty_fd, &cmd);
					}
				}
				None => loop {
					tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
					match connect(&kinds).await {
						Ok(new_lines) => {
							lines = new_lines;
							break;
						}
						Err(_) => continue,
					}
				},
			}
		}
	}
}

fn parse_cmd(line: &str) -> Option<String> {
	match line.split(',').next()? {
		"@cwd" => extract_field(line, "url").map(|url| format!("cd {url}")),
		"@exec" => {
			let cwd = extract_field(line, "cwd")?;
			let cmd = extract_field(line, "cmd")?;
			Some(format!("cd {cwd} && {cmd}"))
		}
		_ => None,
	}
}

fn tiocsti_inject(fd: i32, cmd: &str) {
	for &b in cmd.as_bytes().iter().chain(b"\n") {
		unsafe {
			libc::ioctl(fd, TIOCSTI, &b);
		}
	}
}

fn extract_field(line: &str, key: &str) -> Option<String> {
	let body = line.splitn(4, ',').nth(3)?;
	let json: serde_json::Value = serde_json::from_str(body).ok()?;
	let val = json.get(key)?.as_str()?;
	Some(url_decode(val))
}

fn url_decode(s: &str) -> String {
	let mut result = String::with_capacity(s.len());
	let mut chars = s.bytes();
	while let Some(b) = chars.next() {
		if b == b'%' {
			let hi = chars.next().and_then(hex_val);
			let lo = chars.next().and_then(hex_val);
			match (hi, lo) {
				(Some(h), Some(l)) => result.push((h << 4 | l) as char),
				_ => result.push('%'),
			}
		} else {
			result.push(b as char);
		}
	}
	result
}

fn hex_val(b: u8) -> Option<u8> {
	match b {
		b'0'..=b'9' => Some(b - b'0'),
		b'a'..=b'f' => Some(b - b'0' + 10),
		b'A'..=b'F' => Some(b - b'A' + 10),
		_ => None,
	}
}
