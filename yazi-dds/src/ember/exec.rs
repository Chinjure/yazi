use std::borrow::Cow;

use mlua::{IntoLua, Lua, Value};
use serde::{Deserialize, Serialize};
use yazi_shared::url::UrlBuf;

use super::Ember;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EmberExec<'a> {
	pub cwd: Cow<'a, UrlBuf>,
	pub cmd: Cow<'a, str>,
}

impl<'a> EmberExec<'a> {
	pub fn borrowed(cwd: &'a UrlBuf, cmd: &'a str) -> Ember<'a> {
		Self { cwd: cwd.into(), cmd: cmd.into() }.into()
	}
}

impl EmberExec<'static> {
	pub fn owned(cwd: &UrlBuf, cmd: &str) -> Ember<'static> {
		Self { cwd: cwd.clone().into(), cmd: cmd.to_owned().into() }.into()
	}
}

impl<'a> From<EmberExec<'a>> for Ember<'a> {
	fn from(value: EmberExec<'a>) -> Self { Self::Exec(value) }
}

impl IntoLua for EmberExec<'_> {
	fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
		lua
			.create_table_from([
				("cwd", yazi_binding::Url::new(self.cwd.into_owned()).into_lua(lua)?),
				("cmd", self.cmd.into_owned().into_lua(lua)?),
			])?
			.into_lua(lua)
	}
}
