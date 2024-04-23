// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::sync::OnceLock;

use handlebars::Handlebars;
use handlebars::RenderError;
use postmark::reqwest::PostmarkClient;
use postmark::Query;
use serde::Serialize;

use crate::ids::ScopeName;

const BASE_TXT: &str = "base.txt";
const BASE_HTML: &str = "base.html";
const SCOPE_INVITE_TXT: &str = "scope_invite.txt";
const SCOPE_INVITE_HTML: &str = "scope_invite.html";
const PERSONAL_ACCESS_TOKEN_TXT: &str = "personal_access_token.txt";
const PERSONAL_ACCESS_TOKEN_HTML: &str = "personal_access_token.html";

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum EmailArgs<'a> {
  ScopeInvite {
    name: Cow<'a, str>,
    inviter_name: Cow<'a, str>,
    scope: Cow<'a, ScopeName>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
  PersonalAccessToken {
    name: Cow<'a, str>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
}

impl EmailArgs<'_> {
  pub fn subject(&self) -> String {
    match self {
      EmailArgs::ScopeInvite {
        scope,
        registry_name,
        ..
      } => {
        format!("You've been invited to @{scope} on {registry_name}")
      }
      EmailArgs::PersonalAccessToken { registry_name, .. } => {
        format!("A new personal access token was created on {registry_name}")
      }
    }
  }

  pub fn text_template_filename(&self) -> &'static str {
    match self {
      EmailArgs::ScopeInvite { .. } => SCOPE_INVITE_TXT,
      EmailArgs::PersonalAccessToken { .. } => PERSONAL_ACCESS_TOKEN_TXT,
    }
  }

  pub fn html_template_filename(&self) -> &'static str {
    match self {
      EmailArgs::ScopeInvite { .. } => SCOPE_INVITE_HTML,
      EmailArgs::PersonalAccessToken { .. } => PERSONAL_ACCESS_TOKEN_HTML,
    }
  }
}

fn init_handlebars(
) -> Result<Handlebars<'static>, Box<handlebars::TemplateError>> {
  let mut t: Handlebars<'static> = Handlebars::new();

  t.register_template_string(
    BASE_TXT,
    include_str!("./templates/base.txt.hbs"),
  )?;
  t.register_template_string(
    BASE_HTML,
    include_str!("./templates/base.html.hbs"),
  )?;
  t.register_template_string(
    SCOPE_INVITE_TXT,
    include_str!("./templates/scope_invite.txt.hbs"),
  )?;
  t.register_template_string(
    SCOPE_INVITE_HTML,
    include_str!("./templates/scope_invite.html.hbs"),
  )?;
  t.register_template_string(
    PERSONAL_ACCESS_TOKEN_TXT,
    include_str!("./templates/personal_access_token.txt.hbs"),
  )?;
  t.register_template_string(
    PERSONAL_ACCESS_TOKEN_HTML,
    include_str!("./templates/personal_access_token.html.hbs"),
  )?;

  t.set_strict_mode(true);

  Ok(t)
}

#[derive(Debug)]
pub struct EmailContent {
  pub text: String,
  pub html: String,
}

pub fn email_content(args: EmailArgs) -> Result<EmailContent, RenderError> {
  static TEMPLATE_ENGINE: OnceLock<Handlebars<'static>> = OnceLock::new();
  let hbs = TEMPLATE_ENGINE.get_or_init(|| init_handlebars().unwrap());

  let text_filename = args.text_template_filename();
  let html_filename = args.html_template_filename();

  let text = hbs.render(text_filename, &args)?;
  let html = hbs.render(html_filename, &args)?;

  Ok(EmailContent { text, html })
}

#[derive(Debug)]
pub struct EmailSender {
  postmark: PostmarkClient,
  pub from: String,
  pub from_name: String,
}

impl EmailSender {
  pub fn new(
    postmark: PostmarkClient,
    from: String,
    from_name: String,
  ) -> Self {
    Self {
      postmark,
      from,
      from_name,
    }
  }

  pub async fn send(
    &self,
    to: String,
    args: EmailArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    let subject = args.subject();
    let content = email_content(args)?;
    let req = postmark::api::email::SendEmailRequest::builder()
      .from(format!("{} <{}>", self.from_name, self.from))
      .to(to)
      .subject(subject)
      .body(postmark::api::Body::HtmlAndText {
        html: content.html,
        text: content.text,
      })
      .build();
    let resp = req.execute(&self.postmark).await?;
    if resp.error_code != 0 {
      Err(anyhow::anyhow!(
        "Postmark error {}: {}",
        resp.error_code,
        resp.message
      ))
    } else {
      Ok(())
    }
  }
}
