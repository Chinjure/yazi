use std::os::fd::AsRawFd;

use anyhow::{Context, Result};
use hashbrown::HashSet;
use tokio::io::AsyncWriteExt;
use yazi_dds::{ClientReader, Payload, Stream, ember::EmberHi};
use yazi_macro::try_format;

use crate::dds::Dds;

const TIOCSTI: u64 = 0x5412;

impl Dds {
	/// Subscribe to @cwd and @exec events and inject cd commands into the calling
	/// terminal via TIOCSTI, achieving real-time CWD sync and remote exec.
	///
	/// The process daemonizes: it injects the initial state, then forks to
	/// background so the shell regains control while monitoring continues.
	pub(crate) async fn follow() -> Result<()> {
		async fn connect(kinds: &HashSet<&str>) -> Result<ClientReader> {
			let (lines, mut writer) = Stream::connect().await?;
			let hi = Payload::new(EmberHi::borrowed(kinds.iter().copied()));
			writer.write_all(try_format!("{hi}\n")?.as_bytes()).await?;
			writer.flush().await?;
			Ok(lines)
		}

		// Open /dev/tty NOW, before forking (child inherits fd but loses
		// controlling terminal if we call setsid)
		let tty = std::fs::File::open("/dev/tty").context("Cannot open /dev/tty")?;
		let tty_fd = tty.as_raw_fd();

		// Ignore SIGTTOU — background processes can't write to terminal
		unsafe { libc::signal(libc::SIGTTOU, libc::SIG_IGN) };

		let kinds = HashSet::from_iter(["@cwd", "@exec"]);

		let mut lines =
			connect(&kinds).await.context("No running Yazi instance found. Start yazi first.")?;

		// Read initial state (replayed @cwd / @exec) and inject immediately
		while let Ok(Ok(Some(line))) =
			tokio::time::timeout(std::time::Duration::from_millis(500), lines.next_line()).await
		{
			if let Some(cmd) = parse_cmd(&line) {
				tiocsti_inject(tty_fd, &cmd);
			}
		}

		// Fork to background: parent exits so shell regains terminal control
		unsafe {
			let pid = libc::fork();
			if pid < 0 {
				return Err(anyhow::anyhow!("fork failed"));
			}
			if pid != 0 {
				// Parent: exit immediately, shell is now in control
				std::process::exit(0);
			}
			// Child: continue monitoring
			libc::setsid();
		}

		// Drop the tty File but keep the raw fd (child process)
		// tty is dropped here but we already have tty_fd as raw i32

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
	let kind = line.split(',').next()?;
	match kind {
		"@cwd" => extract_field(line, "url").map(|url| format!("cd {url}")),
		"@exec" => {
			let cwd = extract_field(line, "cwd")?;
			let cmd = extract_field(line, "cmd")?;
			Some(format!("cd {cwd} && {cmd}"))
		}
		_ => None,
	}
}

/// Inject a shell command into the terminal as keystrokes via TIOCSTI.
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
