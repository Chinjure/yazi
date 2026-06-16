use std::borrow::Cow;

use mlua::{IntoLua, Lua, Value};
use serde::{Deserialize, Serialize};
use yazi_shared::url::UrlBuf;

use super::Ember;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EmberCwd<'a> {
	pub url: Cow<'a, UrlBuf>,
}

impl<'a> EmberCwd<'a> {
	pub fn borrowed(url: &'a UrlBuf) -> Ember<'a> {
		Self { url: url.into() }.into()
	}
}

impl EmberCwd<'static> {
	pub fn owned(url: &UrlBuf) -> Ember<'static> {
		Self { url: url.clone().into() }.into()
	}
}

impl<'a> From<EmberCwd<'a>> for Ember<'a> {
	fn from(value: EmberCwd<'a>) -> Self { Self::Cwd(value) }
}

impl IntoLua for EmberCwd<'_> {
	fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
		lua
			.create_table_from([(
				"url",
				yazi_binding::Url::new(self.url.into_owned()).into_lua(lua)?,
			)])?
			.into_lua(lua)
	}
}
