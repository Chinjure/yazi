use anyhow::{Context, Result};
use hashbrown::HashSet;
use tokio::io::AsyncWriteExt;
use yazi_dds::{ClientReader, Payload, Stream, ember::EmberHi};
use yazi_macro::try_format;

use crate::dds::Dds;

impl Dds {
	/// Subscribe to @cwd events and output clean directory paths for shell integration.
	/// One path per line. Reconnects automatically if the Yazi instance restarts.
	pub(crate) async fn follow() -> Result<()> {
		async fn connect(kinds: &HashSet<&str>) -> Result<ClientReader> {
			let (lines, mut writer) = Stream::connect().await?;
			let hi = Payload::new(EmberHi::borrowed(kinds.iter().copied()));
			writer.write_all(try_format!("{hi}\n")?.as_bytes()).await?;
			writer.flush().await?;
			Ok(lines)
		}

		let kinds = HashSet::from_iter(["@cwd"]);

		let mut lines =
			connect(&kinds).await.context("No running Yazi instance found. Start yazi first.")?;

		loop {
			match lines.next_line().await? {
				Some(line) => {
					// Format: "@cwd,0,<sender>,{\"url\":\"/path/to/dir\"}"
					if let Some(url) = extract_url(&line) {
						println!("{url}");
					}
				}
				None => {
					// Connection lost -- reconnect forever
					loop {
						tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
						match connect(&kinds).await {
							Ok(new_lines) => {
								lines = new_lines;
								break;
							}
							Err(_) => continue,
						}
					}
				}
			}
		}
	}
}

/// Extract the directory path from a DDS @cwd event line.
/// Input format: "@cwd,0,1234,{"url":"/home/user/projects"}"
fn extract_url(line: &str) -> Option<String> {
	let body = line.splitn(4, ',').nth(3)?;
	let json: serde_json::Value = serde_json::from_str(body).ok()?;
	let url = json.get("url")?.as_str()?;
	Some(url_decode(url))
}

/// Decode percent-encoded strand URLs back to a filesystem path.
/// If decoding fails, returns the original string unchanged.
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
