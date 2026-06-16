use anyhow::{Context, Result};
use hashbrown::HashSet;
use tokio::io::AsyncWriteExt;
use yazi_dds::{ClientReader, Payload, Stream, ember::EmberHi};
use yazi_macro::try_format;

use crate::dds::Dds;

impl Dds {
	/// Subscribe to @cwd and @exec events and output clean directory paths and
	/// shell commands for integration. Reconnects automatically.
	pub(crate) async fn follow() -> Result<()> {
		async fn connect(kinds: &HashSet<&str>) -> Result<ClientReader> {
			let (lines, mut writer) = Stream::connect().await?;
			let hi = Payload::new(EmberHi::borrowed(kinds.iter().copied()));
			writer.write_all(try_format!("{hi}\n")?.as_bytes()).await?;
			writer.flush().await?;
			Ok(lines)
		}

		let kinds = HashSet::from_iter(["@cwd", "@exec"]);

		let mut lines =
			connect(&kinds).await.context("No running Yazi instance found. Start yazi first.")?;

		loop {
			match lines.next_line().await? {
				Some(line) => {
					if let Some(kind) = line.split(',').next() {
						match kind {
							"@cwd" => {
								if let Some(url) = extract_field(&line, "url") {
									println!("{url}");
								}
							}
							"@exec" => {
								if let (Some(cwd), Some(cmd)) =
									(extract_field(&line, "cwd"), extract_field(&line, "cmd"))
								{
									println!("EXEC\t{cwd}\t{cmd}");
								}
							}
							_ => {}
						}
					}
				}
				None => {
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

/// Extract a JSON field value from a DDS event line.
/// Format: "kind,receiver,sender,{...json...}"
fn extract_field(line: &str, key: &str) -> Option<String> {
	let body = line.splitn(4, ',').nth(3)?;
	let json: serde_json::Value = serde_json::from_str(body).ok()?;
	let val = json.get(key)?.as_str()?;
	Some(url_decode(val))
}

/// Decode percent-encoded strand URLs back to a filesystem path.
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
