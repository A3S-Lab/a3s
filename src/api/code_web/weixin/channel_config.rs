use std::path::Path;

use a3s_acl::{Block, Value};

use super::dto::SafeBlocker;

pub(super) enum WeixinChannelLoad {
    Enabled,
    Unavailable(SafeBlocker),
}

pub(super) struct WeixinChannelConfig;

impl WeixinChannelConfig {
    pub(super) fn load(config_path: &Path) -> WeixinChannelLoad {
        let source = match std::fs::read_to_string(config_path) {
            Ok(source) => Some(source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => {
                return WeixinChannelLoad::Unavailable(blocker(
                    "ilink_configuration_unreadable",
                    "The local Weixin channel configuration could not be read.",
                ))
            }
        };
        load_from_source(source.as_deref())
    }
}

fn load_from_source(source: Option<&str>) -> WeixinChannelLoad {
    let block = match source.map(parse_weixin_block).transpose() {
        Ok(block) => block.flatten(),
        Err(issue) => return WeixinChannelLoad::Unavailable(issue),
    };

    let Some(block) = block else {
        return WeixinChannelLoad::Enabled;
    };
    if !block.blocks.is_empty()
        || block
            .attributes
            .keys()
            .any(|attribute| attribute != "enabled")
    {
        return WeixinChannelLoad::Unavailable(invalid_configuration());
    }
    match block.attributes.get("enabled") {
        None | Some(Value::Bool(true)) => WeixinChannelLoad::Enabled,
        Some(Value::Bool(false)) => WeixinChannelLoad::Unavailable(blocker(
            "ilink_channel_disabled",
            "The Weixin channel is disabled in the local A3S configuration.",
        )),
        Some(_) => WeixinChannelLoad::Unavailable(invalid_configuration()),
    }
}

fn parse_weixin_block(source: &str) -> Result<Option<Block>, SafeBlocker> {
    let document = a3s_acl::parse_acl(source).map_err(|_| invalid_configuration())?;
    let channels = document
        .blocks
        .iter()
        .filter(|block| block.name == "channels")
        .collect::<Vec<_>>();
    if channels.len() > 1 {
        return Err(invalid_configuration());
    }
    let Some(channels) = channels.first() else {
        return Ok(None);
    };
    let weixin = channels
        .blocks
        .iter()
        .filter(|block| block.name == "weixin")
        .collect::<Vec<_>>();
    if weixin.len() > 1 {
        return Err(invalid_configuration());
    }
    Ok(weixin.first().map(|block| (*block).clone()))
}

fn invalid_configuration() -> SafeBlocker {
    blocker(
        "ilink_configuration_invalid",
        "The local Weixin channel configuration is invalid.",
    )
}

fn blocker(code: &str, message: &str) -> SafeBlocker {
    SafeBlocker {
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_channel_configuration_enables_builtin_ilink() {
        assert!(matches!(load_from_source(None), WeixinChannelLoad::Enabled));
    }

    #[test]
    fn explicit_enable_does_not_require_protocol_identity_fields() {
        assert!(matches!(
            load_from_source(Some("channels { weixin { enabled = true } }")),
            WeixinChannelLoad::Enabled
        ));
    }

    #[test]
    fn removed_protocol_identity_fields_are_rejected() {
        let WeixinChannelLoad::Unavailable(blocker) = load_from_source(Some(
            r#"
                channels {
                  weixin {
                    app_id = "obsolete"
                    bot_type = "obsolete"
                  }
                }
            "#,
        )) else {
            panic!("removed protocol identity fields must not be accepted");
        };
        assert_eq!(blocker.code, "ilink_configuration_invalid");
    }

    #[test]
    fn explicit_disable_keeps_the_channel_unavailable() {
        let WeixinChannelLoad::Unavailable(blocker) =
            load_from_source(Some("channels { weixin { enabled = false } }"))
        else {
            panic!("explicit disable must win");
        };
        assert_eq!(blocker.code, "ilink_channel_disabled");
    }
}
