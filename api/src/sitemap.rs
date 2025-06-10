// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::Body;
use hyper::Request;
use hyper::Response;
use routerify::ext::RequestExt;
use sitemap_rs::sitemap::Sitemap;
use sitemap_rs::sitemap_index::SitemapIndex;
use sitemap_rs::url::ChangeFrequency;
use sitemap_rs::url_builder::UrlBuilder;
use sitemap_rs::url_set::UrlSet;
use tracing::error;

use crate::RegistryUrl;
use crate::api::ApiError;
use crate::db::Database;

#[allow(deprecated)] // the replacement can not be used in const positions in stable
const TWO_DAYS: chrono::Duration = chrono::Duration::days(2);
#[allow(deprecated)] // the replacement can not be used in const positions in stable
const FOUR_WEEKS: chrono::Duration = chrono::Duration::weeks(3);

pub async fn sitemap_index_handler(
  req: Request<Body>,
) -> Result<Response<Body>, ApiError> {
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let sitemaps = vec![
    Sitemap::new(format!("{registry_url}sitemap-scopes.xml"), None),
    Sitemap::new(format!("{registry_url}sitemap-packages.xml"), None),
  ];
  let sitemap_index = SitemapIndex::new(sitemaps).map_err(|err| {
    error!("Failed to build sitemap: {}", err);
    ApiError::InternalServerError
  })?;

  let mut bytes = Vec::new();
  sitemap_index.write(&mut bytes).map_err(|err| {
    error!("Failed to write sitemap: {}", err);
    ApiError::InternalServerError
  })?;

  let response = Response::builder()
    .header("Content-Type", "application/xml")
    .body(Body::from(bytes))
    .unwrap();

  Ok(response)
}

pub async fn scopes_sitemap_handler(
  req: Request<Body>,
) -> Result<Response<Body>, ApiError> {
  let db = req.data::<Database>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let scopes = db.list_all_scopes_for_sitemap().await?;

  let mut urls = vec![];

  for (scope, updated_at, latest_package_updated_at) in scopes {
    let updated_at = latest_package_updated_at
      .map(|latest_package_updated_at| {
        latest_package_updated_at.max(updated_at)
      })
      .unwrap_or(updated_at);

    let last_updated = updated_at - chrono::Utc::now();
    let change_frequency = if last_updated < TWO_DAYS {
      ChangeFrequency::Daily
    } else if last_updated < FOUR_WEEKS {
      ChangeFrequency::Weekly
    } else {
      ChangeFrequency::Monthly
    };

    let Ok(url) = UrlBuilder::new(format!("{registry_url}@{scope}"))
      .last_modified(updated_at.fixed_offset())
      .change_frequency(change_frequency)
      .build()
    else {
      continue;
    };
    urls.push(url);
  }

  let url_set = UrlSet::new(urls).map_err(|err| {
    error!("Failed to build sitemap: {}", err);
    ApiError::InternalServerError
  })?;

  let mut bytes = Vec::new();
  url_set.write(&mut bytes).map_err(|err| {
    error!("Failed to write sitemap: {}", err);
    ApiError::InternalServerError
  })?;

  let response = Response::builder()
    .header("Content-Type", "application/xml")
    .body(Body::from(bytes))
    .unwrap();

  Ok(response)
}

pub async fn packages_sitemap_handler(
  req: Request<Body>,
) -> Result<Response<Body>, ApiError> {
  let db = req.data::<Database>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let packages = db.list_all_packages_for_sitemap().await?;

  let mut urls = vec![];

  for (scope, package, updated_at, latest_version_updated_at) in packages {
    let updated_at = latest_version_updated_at.max(updated_at);

    let last_updated = updated_at - chrono::Utc::now();
    let change_frequency = if last_updated < TWO_DAYS {
      ChangeFrequency::Daily
    } else if last_updated < FOUR_WEEKS {
      ChangeFrequency::Weekly
    } else {
      ChangeFrequency::Monthly
    };

    let Ok(url) = UrlBuilder::new(format!("{registry_url}@{scope}/{package}"))
      .last_modified(updated_at.fixed_offset())
      .change_frequency(change_frequency)
      .build()
    else {
      continue;
    };
    urls.push(url);
  }

  let url_set = UrlSet::new(urls).map_err(|err| {
    error!("Failed to build sitemap: {}", err);
    ApiError::InternalServerError
  })?;

  let mut bytes = Vec::new();
  url_set.write(&mut bytes).map_err(|err| {
    error!("Failed to write sitemap: {}", err);
    ApiError::InternalServerError
  })?;

  let response = Response::builder()
    .header("Content-Type", "application/xml")
    .body(Body::from(bytes))
    .unwrap();

  Ok(response)
}
