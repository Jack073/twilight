use crate::request::prelude::*;
use dawn_model::id::{GuildId, IntegrationId};

#[derive(Default, Serialize)]
struct UpdateGuildIntegrationFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_emoticons: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expire_behavior: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expire_grace_period: Option<u64>,
}

pub struct UpdateGuildIntegration<'a> {
    fields: UpdateGuildIntegrationFields,
    fut: Option<Pending<'a, ()>>,
    guild_id: GuildId,
    http: &'a Client,
    integration_id: IntegrationId,
    reason: Option<String>,
}

impl<'a> UpdateGuildIntegration<'a> {
    pub(crate) fn new(http: &'a Client, guild_id: GuildId, integration_id: IntegrationId) -> Self {
        Self {
            fields: UpdateGuildIntegrationFields::default(),
            fut: None,
            guild_id,
            http,
            integration_id,
            reason: None,
        }
    }

    pub fn enable_emoticons(mut self, enable_emoticons: bool) -> Self {
        self.fields.enable_emoticons.replace(enable_emoticons);

        self
    }

    pub fn expire_behavior(mut self, expire_behavior: u64) -> Self {
        self.fields.expire_behavior.replace(expire_behavior);

        self
    }

    pub fn expire_grace_period(mut self, expire_grace_period: u64) -> Self {
        self.fields.expire_grace_period.replace(expire_grace_period);

        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason.replace(reason.into());

        self
    }

    fn start(&mut self) -> Result<()> {
        let request = if let Some(reason) = &self.reason {
            let headers = audit_header(&reason)?;
            Request::from((
                serde_json::to_vec(&self.fields)?,
                headers,
                Route::UpdateGuildIntegration {
                    guild_id: self.guild_id.0,
                    integration_id: self.integration_id.0,
                },
            ))
        } else {
            Request::from((
                serde_json::to_vec(&self.fields)?,
                Route::UpdateGuildIntegration {
                    guild_id: self.guild_id.0,
                    integration_id: self.integration_id.0,
                },
            ))
        };

        self.fut.replace(Box::pin(self.http.request(request)));

        Ok(())
    }
}

poll_req!(UpdateGuildIntegration<'_>, ());
